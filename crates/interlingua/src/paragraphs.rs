//! A paragraph's translation, split by sentence and placed in the book:
//! what the layer draws above the lines, kept in memory because a layer
//! must not read the card while it draws.
//!
//! The text filter makes a [`Paragraph`] of each paragraph that has a kept
//! translation: [`pair`] says which part of the translation belongs to each
//! source sentence, and the block's offset map turns the sentences' places
//! in the text into places in the book, which is what the words of a page
//! have. The layer then asks [`Paragraph::sentence_at`] for the sentence a
//! word is in, and [`Sentence::part`] for the part of its translation that
//! goes above the line the word is on.
//!
//! Pure as [`crate::text`]: strings and numbers, tested with `cargo test`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::pairing::{pair, split_sentences};

/// A source sentence, where it is in the book and its translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentence {
    /// Where it starts and ends in the book (source offsets).
    pub start: u32,
    pub end: u32,
    /// Its translation; empty for a sentence with none, and for one that
    /// shares its translation with the sentence before it (the translator
    /// merged the two): the layer draws it once, above the first.
    pub translation: String,
}

/// The sentences of a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraph {
    /// Where the paragraph starts and ends in the book.
    pub start: u32,
    pub end: u32,
    pub sentences: Vec<Sentence>,
}

impl Paragraph {
    /// The paragraph with `source` (its text, from `start` to `end` in the
    /// book) translated as `translation`. `offset` gives the place in the
    /// book of an index of `source` (`Block::source_offset`).
    pub fn new(
        source: &str,
        translation: &str,
        start: u32,
        end: u32,
        offset: impl Fn(usize) -> u32,
    ) -> Self {
        let sentences = split_sentences(source);
        let ranges = pair(source, translation);
        let mut out: Vec<Sentence> = Vec::with_capacity(sentences.len());
        for (i, sentence) in sentences.iter().enumerate() {
            let shared = i > 0 && ranges[i] == ranges[i - 1];
            let translation = match shared {
                true => String::new(),
                false => String::from(&translation[ranges[i].clone()]),
            };
            out.push(Sentence {
                start: offset(sentence.start),
                end: offset(sentence.end),
                translation,
            });
        }
        Self {
            start,
            end,
            sentences: out,
        }
    }

    /// The sentence the book's `offset` is in: the last one that starts at
    /// or before it.
    pub fn sentence_at(&self, offset: u32) -> Option<&Sentence> {
        let after = self.sentences.partition_point(|s| s.start <= offset);
        after.checked_sub(1).map(|i| &self.sentences[i])
    }
}

impl Sentence {
    /// The part of the translation for the part of the sentence from
    /// `from` to `to` (offsets in the book): the same fractions of the
    /// translation, moved to the nearest word boundaries, so that a
    /// sentence on several lines has its translation spread over them.
    pub fn part(&self, from: u32, to: u32) -> &str {
        let len = self.translation.len() as u64;
        let span = u64::from(self.end.saturating_sub(self.start));
        if len == 0 || span == 0 || to <= from {
            return "";
        }
        let at = |offset: u32| {
            let inside = u64::from(offset.clamp(self.start, self.end) - self.start);
            self.boundary(inside * len / span)
        };
        let (a, b) = (at(from), at(to));
        self.translation[a..b].trim()
    }

    /// The word boundary of the translation nearest to byte `target`: the
    /// start of the text, of a word, or its end.
    fn boundary(&self, target: u64) -> usize {
        let text = &self.translation;
        let mut best = 0;
        let mut previous_space = true;
        let starts = text.char_indices().filter_map(|(i, c)| {
            let start = previous_space && !c.is_whitespace();
            previous_space = c.is_whitespace();
            start.then_some(i)
        });
        for candidate in starts.chain(core::iter::once(text.len())) {
            if (candidate as u64).abs_diff(target) < (best as u64).abs_diff(target) {
                best = candidate;
            }
        }
        best
    }
}

/// `text` split over `rooms` (their widths, in reading order), each room
/// taking whole words up to its share: up to byte `ends[n]` of `text` (a
/// word whose middle is past it waits for the next room), and no more
/// than fit in it by `measure`. What does not fit flows on; the last room
/// takes all that is left (the layer cuts it there if it is too long). A
/// room too small for the next word stays empty. One piece for each room.
pub fn flow<'a>(
    text: &'a str,
    rooms: &[f32],
    ends: &[usize],
    measure: impl Fn(&str) -> f32,
) -> Vec<&'a str> {
    // Where each word starts and ends.
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                words.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        words.push((s, text.len()));
    }
    let mut pieces = Vec::with_capacity(rooms.len());
    let mut next = 0;
    for (n, &room) in rooms.iter().enumerate() {
        let Some(&(from, _)) = words.get(next) else {
            pieces.push("");
            continue;
        };
        if n + 1 == rooms.len() {
            pieces.push(&text[from..]);
            break;
        }
        let share = ends.get(n).copied().unwrap_or(usize::MAX);
        let mut end = next;
        while let Some(&(at, to)) = words.get(end) {
            if at + (to - at) / 2 > share || measure(&text[from..to]) > room {
                break;
            }
            end += 1;
        }
        match end > next {
            true => pieces.push(&text[from..words[end - 1].1]),
            false => pieces.push(""),
        }
        next = end;
    }
    pieces
}

/// Paragraphs without a translation remembered at most.
pub const MOST_MISSING: usize = 128;

/// The paragraphs the text filter met that have no translation yet, of one
/// book: what tells that the page turned to has some to ask about. Only a
/// small list of places, where the oldest go first.
#[derive(Debug, Default)]
pub struct Missing {
    path: String,
    /// Section, start and end in the book.
    blocks: Vec<(u32, u32, u32)>,
}

impl Missing {
    /// The paragraph from `start` to `end` of `section` of the book at
    /// `path` has no translation.
    pub fn note(&mut self, path: &str, section: u32, start: u32, end: u32) {
        if self.path != path {
            self.path = String::from(path);
            self.blocks.clear();
        }
        if self.blocks.iter().any(|&(s, a, _)| s == section && a == start) {
            return;
        }
        if self.blocks.len() >= MOST_MISSING {
            self.blocks.remove(0);
        }
        self.blocks.push((section, start, end));
    }

    /// The paragraph starting at `start` of `section` has one now.
    pub fn done(&mut self, path: &str, section: u32, start: u32) {
        if self.path == path {
            self.blocks.retain(|&(s, a, _)| !(s == section && a == start));
        }
    }

    /// Whether a paragraph without a translation is, wholly or in part,
    /// between `from` and `to` of `section` of the book at `path`.
    pub fn between(&self, path: &str, section: u32, from: u32, to: u32) -> bool {
        self.path == path
            && self
                .blocks
                .iter()
                .any(|&(s, a, b)| s == section && a < to && b > from)
    }
}

/// Bytes of translations kept for the layer at most: a whole section's
/// (its cache file holds at most [`crate::cache::MAX_BYTES`]) and some of
/// another.
pub const MOST_BYTES: usize = 36 * 1024;

impl Paragraph {
    /// About how much memory it takes.
    fn bytes(&self) -> usize {
        self.sentences
            .iter()
            .map(|s| s.translation.len() + core::mem::size_of::<Sentence>())
            .sum()
    }
}

/// The paragraphs with a translation that the text filter met, of one
/// book: what the layer draws from (the reader lays a page out once and
/// draws it many times, so they must stay). Within [`MOST_BYTES`], those
/// of other sections than the page shown go first (the reader lays them
/// out to count pages), then those farthest from it. All go when another
/// book is laid out.
#[derive(Debug, Default)]
pub struct Shown {
    path: String,
    /// The page shown: its section and offset.
    near: (u32, u32),
    /// Each paragraph with the section it is in.
    paragraphs: Vec<(u32, Paragraph)>,
    bytes: usize,
}

impl Shown {
    /// The page shown is at `offset` of `section`.
    pub fn near(&mut self, section: u32, offset: u32) {
        self.near = (section, offset);
    }

    /// How far `paragraph` of `section` is from the page shown: another
    /// section is farthest.
    fn distance(&self, section: u32, paragraph: &Paragraph) -> u32 {
        let (near_section, at) = self.near;
        if section != near_section {
            return u32::MAX;
        }
        match at < paragraph.start {
            true => paragraph.start - at,
            false => at.saturating_sub(paragraph.end),
        }
    }

    /// Keeps `paragraph` of `section` of the book at `path`, in place of
    /// one kept before for the same place; then the farthest go until
    /// the rest is within [`MOST_BYTES`] (this one too, if it is the
    /// farthest).
    pub fn keep(&mut self, path: &str, section: u32, paragraph: Paragraph) {
        if self.path != path {
            self.path = String::from(path);
            self.paragraphs.clear();
            self.bytes = 0;
        }
        if let Some(i) = self
            .paragraphs
            .iter()
            .position(|(s, p)| *s == section && p.start == paragraph.start)
        {
            let (_, old) = self.paragraphs.swap_remove(i);
            self.bytes -= old.bytes();
        }
        self.bytes += paragraph.bytes();
        self.paragraphs.push((section, paragraph));
        while self.bytes > MOST_BYTES {
            let Some((i, _)) = self
                .paragraphs
                .iter()
                .enumerate()
                .max_by_key(|(_, (s, p))| self.distance(*s, p))
            else {
                break;
            };
            let (_, gone) = self.paragraphs.swap_remove(i);
            self.bytes -= gone.bytes();
        }
    }

    /// The paragraph of the book at `path` that `offset` of `section` is
    /// in, if it is kept.
    pub fn at(&self, path: &str, section: u32, offset: u32) -> Option<&Paragraph> {
        if self.path != path {
            return None;
        }
        self.paragraphs
            .iter()
            .find(|(s, p)| *s == section && p.start <= offset && offset < p.end)
            .map(|(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A paragraph placed 100 bytes into the book, text and book alike.
    fn at_100(source: &str, translation: &str) -> Paragraph {
        Paragraph::new(source, translation, 100, 100 + source.len() as u32, |i| {
            100 + i as u32
        })
    }

    #[test]
    fn each_sentence_has_its_translation_and_its_place() {
        let p = at_100("Hi. How are you today?", "Ciao. Come stai oggi?");
        assert_eq!(p.sentences.len(), 2);
        assert_eq!((p.sentences[0].start, p.sentences[1].start), (100, 104));
        assert_eq!(p.sentences[0].translation, "Ciao.");
        assert_eq!(p.sentences[1].translation, "Come stai oggi?");
    }

    #[test]
    fn a_merged_translation_is_kept_once() {
        let p = at_100("Aaaa bbbb. Cccc dddd.", "Xxxx yyyy zzzz.");
        assert_eq!(p.sentences[0].translation, "Xxxx yyyy zzzz.");
        assert_eq!(p.sentences[1].translation, "");
    }

    #[test]
    fn no_translation_no_text() {
        let p = at_100("One. Two.", "");
        assert!(p.sentences.iter().all(|s| s.translation.is_empty()));
    }

    #[test]
    fn a_word_is_in_the_sentence_that_starts_before_it() {
        let p = at_100("Hi. How are you today?", "Ciao. Come stai oggi?");
        assert_eq!(p.sentence_at(99), None);
        assert_eq!(p.sentence_at(100).unwrap().translation, "Ciao.");
        assert_eq!(p.sentence_at(103).unwrap().translation, "Ciao.");
        assert_eq!(p.sentence_at(104).unwrap().translation, "Come stai oggi?");
        assert_eq!(p.sentence_at(500).unwrap().translation, "Come stai oggi?");
    }

    fn sentence(translation: &str) -> Sentence {
        // "one two three four": 18 bytes.
        Sentence {
            start: 0,
            end: 18,
            translation: String::from(translation),
        }
    }

    #[test]
    fn a_sentence_on_two_lines_is_split_at_a_word() {
        let s = sentence("uno due tre quattro");
        assert_eq!(s.part(0, 9), "uno due");
        assert_eq!(s.part(9, 18), "tre quattro");
    }

    #[test]
    fn the_whole_sentence_is_the_whole_translation() {
        let s = sentence("uno due tre quattro");
        assert_eq!(s.part(0, 18), "uno due tre quattro");
        // Outside the sentence counts as its ends.
        assert_eq!(s.part(0, 500), "uno due tre quattro");
    }

    #[test]
    fn nothing_for_an_empty_part_or_no_translation() {
        assert_eq!(sentence("uno due").part(5, 5), "");
        assert_eq!(sentence("uno due").part(9, 3), "");
        assert_eq!(sentence("").part(0, 18), "");
    }

    #[test]
    fn accents_do_not_break_a_word() {
        let s = sentence("già è così, perché");
        let parts = [s.part(0, 6), s.part(6, 12), s.part(12, 18)];
        assert_eq!(parts.join(" "), "già è così, perché");
    }

    #[test]
    fn shown_finds_a_paragraph_by_book_section_and_offset() {
        let mut shown = Shown::default();
        shown.keep("a.txt", 0, at_100("Hi. How are you?", "Ciao. Come stai?"));
        assert!(shown.at("a.txt", 0, 105).is_some());
        assert!(shown.at("a.txt", 1, 105).is_none());
        assert!(shown.at("b.txt", 0, 105).is_none());
        assert!(shown.at("a.txt", 0, 99).is_none());
    }

    /// A one-sentence paragraph at `n * 10` of the book.
    fn tenth(n: u32) -> Paragraph {
        Paragraph::new("One.", "Uno.", n * 10, n * 10 + 4, |i| n * 10 + i as u32)
    }

    /// A paragraph at `n * 1000` of the book with a translation of
    /// about 1 KiB.
    fn big(n: u32) -> Paragraph {
        let translation = "x".repeat(1000) + ".";
        Paragraph::new("One.", &translation, n * 1000, n * 1000 + 4, |i| n * 1000 + i as u32)
    }

    #[test]
    fn shown_keeps_a_whole_section_and_drops_other_sections_first() {
        let mut shown = Shown::default();
        // Section 0 is shown; the reader also lays out section 1.
        shown.near(0, 5000);
        for n in 0..30 {
            shown.keep("a.txt", 0, big(n));
            shown.keep("a.txt", 1, big(n));
        }
        assert!(shown.bytes <= MOST_BYTES);
        for n in 0..30 {
            assert!(shown.at("a.txt", 0, n * 1000).is_some(), "paragraph {n}");
        }
        // Of the other section, only what the budget leaves room for.
        let others = shown.paragraphs.iter().filter(|(s, _)| *s == 1).count();
        assert!(others < 30, "{others}");
    }

    #[test]
    fn shown_drops_the_farthest_of_the_section_shown_last() {
        let mut shown = Shown::default();
        shown.near(0, 0);
        for n in 0..60 {
            shown.keep("a.txt", 0, big(n));
        }
        assert!(shown.bytes <= MOST_BYTES);
        assert!(shown.at("a.txt", 0, 0).is_some(), "the page shown stays");
        assert!(shown.at("a.txt", 0, 59_000).is_none(), "the farthest went");
    }

    #[test]
    fn shown_keeps_one_book_and_one_paragraph_by_place() {
        let mut shown = Shown::default();
        shown.keep("a.txt", 0, tenth(1));
        shown.keep("a.txt", 0, tenth(1));
        assert_eq!(shown.paragraphs.len(), 1);
        assert_eq!(shown.bytes, tenth(1).bytes());
        shown.keep("b.txt", 0, tenth(2));
        assert_eq!(shown.paragraphs.len(), 1);
        assert!(shown.at("a.txt", 0, 10).is_none());
    }

    /// One unit a character.
    fn chars(text: &str) -> f32 {
        text.chars().count() as f32
    }

    #[test]
    fn a_sentence_flows_over_its_rooms() {
        let text = "strisciando a quattro zampe. Mi chiedevo";
        let pieces = flow(text, &[14.0, 14.0, 40.0], &[], chars);
        assert_eq!(pieces, ["strisciando a", "quattro zampe.", "Mi chiedevo"]);
    }

    #[test]
    fn the_last_room_takes_what_is_left() {
        let pieces = flow("uno due tre quattro", &[7.0, 3.0], &[], chars);
        assert_eq!(pieces, ["uno due", "tre quattro"]);
    }

    #[test]
    fn a_room_too_small_stays_empty_and_rooms_left_over_are_empty() {
        let pieces = flow("lunghissima parola", &[3.0, 11.0, 20.0, 5.0], &[], chars);
        assert_eq!(pieces, ["", "lunghissima", "parola", ""]);
    }

    #[test]
    fn nothing_flows_nowhere() {
        assert!(flow("uno", &[], &[], chars).is_empty());
        assert_eq!(flow("", &[5.0, 5.0], &[], chars), ["", ""]);
        assert_eq!(flow("  già  è ", &[20.0], &[], chars), ["già  è "]);
    }

    #[test]
    fn a_room_with_room_to_spare_takes_only_its_share() {
        // Three lines of the same width; the translation's halves belong
        // to the first two lines, nothing to the third until it is the last.
        let text = "uno due tre quattro";
        let pieces = flow(text, &[40.0, 40.0, 40.0], &[8, 19, 19], chars);
        assert_eq!(pieces, ["uno due", "tre quattro", ""]);
    }

    #[test]
    fn what_does_not_fit_in_its_share_flows_on() {
        let text = "uno due tre quattro";
        let pieces = flow(text, &[3.0, 40.0], &[8, 19], chars);
        assert_eq!(pieces, ["uno", "due tre quattro"]);
    }

    #[test]
    fn missing_tells_a_page_with_a_paragraph_not_translated() {
        let mut missing = Missing::default();
        missing.note("a.txt", 0, 100, 200);
        // A page from 150 to 400 holds the end of it; one from 200 does not.
        assert!(missing.between("a.txt", 0, 150, 400));
        assert!(!missing.between("a.txt", 0, 200, 400));
        assert!(!missing.between("a.txt", 1, 150, 400));
        assert!(!missing.between("b.txt", 0, 150, 400));
        missing.done("a.txt", 0, 100);
        assert!(!missing.between("a.txt", 0, 150, 400));
    }

    #[test]
    fn missing_keeps_one_book_and_a_bounded_list() {
        let mut missing = Missing::default();
        for n in 0..MOST_MISSING as u32 + 10 {
            missing.note("a.txt", 0, n * 10, n * 10 + 5);
            missing.note("a.txt", 0, n * 10, n * 10 + 5);
        }
        assert_eq!(missing.blocks.len(), MOST_MISSING);
        assert!(!missing.between("a.txt", 0, 0, 5), "the oldest went first");
        missing.note("b.txt", 0, 1, 2);
        assert_eq!(missing.blocks.len(), 1);
    }
}
