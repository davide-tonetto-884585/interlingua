//! The translations kept on the card: what the text filter reads, so that
//! no paragraph waits for the network when a page is laid out.
//!
//! One file for each stretch of [`BUCKET`] bytes of a section of a book, and
//! language, named as PaperPaw wants a book's cache named
//! (`tr-<book key>.<language>.<section>.<stretch>` in the plugin's folder,
//! see
//! `Registrar::add_book_cache`), so that it follows the book and goes away
//! with it. A line for each paragraph: its offset in the section, a tab
//! and its translation (`\`, line breaks and tabs escaped). Entries are
//! only appended; when an offset is there twice, the last one counts.
//!
//! Pure as [`crate::text`]: strings and a [`Storage`], nothing of the
//! device, so `cargo test` runs it with a [`MemoryStorage`].

use alloc::format;
use alloc::string::String;
use alloc::vec;

use paperpaw_sdk::{Storage, StorageError};

/// The start of a cache file's name, before the book's key.
pub const PREFIX: &str = "tr-";

/// How much of a section's source one cache file holds, in bytes of the
/// book: its translations (a quarter more, at most) stay below
/// [`MAX_BYTES`], so that a long chapter is not one file too big to read.
pub const BUCKET: u32 = 16 * 1024;

/// Largest cache file read into memory: the heap of an X4 is about
/// 256 KiB.
pub const MAX_BYTES: u64 = 32 * 1024;

/// The file with the translations into `language` (`it`, `zh-CN`) of the
/// paragraph at `offset` of `section` of the book with `key`
/// (`paperpaw_sdk::library::key`), and of those near it, in the plugin's
/// folder `dir`.
pub fn path(dir: &str, key: &str, language: &str, section: u32, offset: u32) -> String {
    format!("{dir}/{PREFIX}{key}.{language}.{section}.{}", offset / BUCKET)
}

/// The line that goes into the file for the paragraph at `offset`.
pub fn entry(offset: u32, translation: &str) -> String {
    let mut line = format!("{offset}\t");
    for c in translation.chars() {
        match c {
            '\\' => line.push_str("\\\\"),
            '\n' => line.push_str("\\n"),
            '\r' => line.push_str("\\r"),
            '\t' => line.push_str("\\t"),
            c => line.push(c),
        }
    }
    line.push('\n');
    line
}

/// The translation of the paragraph at `offset` in `data` (a cache file's
/// text), if it is there.
pub fn lookup(data: &str, offset: u32) -> Option<String> {
    let mut found = None;
    for line in data.lines() {
        let Some((at, text)) = line.split_once('\t') else {
            continue;
        };
        if at.parse::<u32>() == Ok(offset) {
            found = Some(text);
        }
    }
    found.map(unescape)
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// The text of the cache file at `path`: `None` if it is not there, too
/// big ([`MAX_BYTES`]) or not text.
pub fn load(storage: &dyn Storage, path: &str) -> Option<String> {
    let size = storage.size(path).ok()?;
    if size > MAX_BYTES {
        return None;
    }
    let mut data = vec![0u8; usize::try_from(size).ok()?];
    let mut read = 0;
    while read < data.len() {
        match storage.read_at(path, read as u64, &mut data[read..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => read += n,
        }
    }
    data.truncate(read);
    String::from_utf8(data).ok()
}

/// Keeps the translation of the paragraph at `offset` in the file at
/// `path`.
pub fn store(
    storage: &dyn Storage,
    path: &str,
    offset: u32,
    translation: &str,
) -> Result<(), StorageError> {
    storage.append(path, entry(offset, translation).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use paperpaw_sdk::MemoryStorage;

    #[test]
    fn names_follow_the_books_cache_rule() {
        assert_eq!(
            path(".paperpaw/plugins/interlingua", "0123456789abcdef", "it", 3, 40_000),
            ".paperpaw/plugins/interlingua/tr-0123456789abcdef.it.3.2"
        );
        // Paragraphs of one stretch share a file; the next stretch has its own.
        let (a, b) = (path("d", "k", "it", 1, 0), path("d", "k", "it", 1, BUCKET - 1));
        assert_eq!(a, b);
        assert_ne!(a, path("d", "k", "it", 1, BUCKET));
    }

    #[test]
    fn what_is_kept_is_found_again() {
        let data = entry(0, "Ciao") + &entry(120, "Alice era stanca.");
        assert_eq!(lookup(&data, 120).as_deref(), Some("Alice era stanca."));
        assert_eq!(lookup(&data, 0).as_deref(), Some("Ciao"));
        assert_eq!(lookup(&data, 7), None);
    }

    #[test]
    fn line_breaks_tabs_and_backslashes_survive() {
        let text = "uno\ndue\ttre \\ quattro\r";
        let data = entry(5, text);
        assert_eq!(data.lines().count(), 1);
        assert_eq!(lookup(&data, 5).as_deref(), Some(text));
    }

    #[test]
    fn the_last_entry_for_an_offset_counts() {
        let data = entry(9, "vecchia") + &entry(9, "nuova");
        assert_eq!(lookup(&data, 9).as_deref(), Some("nuova"));
    }

    #[test]
    fn damaged_lines_are_skipped() {
        let data = "senza tab\nxx\tno\n7\tsì\n";
        assert_eq!(lookup(data, 7).as_deref(), Some("sì"));
    }

    #[test]
    fn storage_round_trip_and_limits() {
        let storage = MemoryStorage::new();
        let file = path("d", "k", "it", 1, 0);
        assert_eq!(load(&storage, &file), None);
        store(&storage, &file, 3, "uno").unwrap();
        store(&storage, &file, 8, "due").unwrap();
        let data = load(&storage, &file).unwrap();
        assert_eq!(lookup(&data, 8).as_deref(), Some("due"));

        let big = MemoryStorage::new().with("big", vec![b'x'; (MAX_BYTES + 1) as usize]);
        assert_eq!(load(&big, "big"), None);
    }

    #[test]
    fn a_long_chapter_is_several_files_each_readable() {
        // 120 paragraphs of 700 bytes of source with 900 of translation:
        // 108 KB of translations, far past what one file may hold.
        let storage = MemoryStorage::new();
        let translation = "x".repeat(900);
        for n in 0..120u32 {
            let offset = n * 700;
            store(&storage, &path("d", "k", "it", 0, offset), offset, &translation).unwrap();
        }
        for n in 0..120u32 {
            let offset = n * 700;
            let file = path("d", "k", "it", 0, offset);
            let data = load(&storage, &file).expect("every file is readable");
            assert_eq!(lookup(&data, offset).as_deref(), Some(translation.as_str()), "{n}");
        }
    }
}
