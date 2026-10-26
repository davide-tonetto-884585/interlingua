//! Words and sentences: plain Rust on `&str`, no screen and no device, so
//! `cargo test` checks it on a computer in a moment.

use alloc::vec::Vec;
use core::ops::Range;

/// Whether `word` ends a sentence: it ends in `.`, `!` or `?`, after any
/// closing quotes or brackets.
pub fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', '’', '”', ')'])
        .ends_with(['.', '!', '?'])
}

/// The sentences of `text`, as byte ranges into it, in order.
///
/// `text` is a block's text, whose spaces the book's format already
/// collapsed (one space between words). A sentence ends with a word that
/// [`ends_sentence`]; text after the last such word is a sentence too.
///
/// Byte ranges, not `String`s: slicing (`&text[range]`) costs nothing,
/// and on the X4 every allocation counts.
pub fn split_sentences(text: &str) -> Vec<Range<usize>> {
    let mut sentences = Vec::new();
    // Where the sentence being read began, once it has a word.
    let mut start: Option<usize> = None;
    let mut offset = 0;
    for word in text.split(' ') {
        let end = offset + word.len();
        if !word.is_empty() {
            let begin = *start.get_or_insert(offset);
            if ends_sentence(word) {
                sentences.push(begin..end);
                start = None;
            }
        }
        // Past the word and the space after it.
        offset = end + 1;
    }
    if let Some(begin) = start {
        sentences.push(begin..text.trim_end().len());
    }
    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sentences of `text` as text, for readable assertions.
    fn sentences(text: &str) -> Vec<&str> {
        split_sentences(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn sentence_ends() {
        assert!(ends_sentence("end."));
        assert!(ends_sentence("really?”"));
        assert!(ends_sentence("stop!)"));
        assert!(!ends_sentence("comma,"));
        assert!(!ends_sentence("word"));
    }

    #[test]
    fn splits_at_terminators() {
        assert_eq!(sentences("One. Two! Three?"), ["One.", "Two!", "Three?"]);
    }

    #[test]
    fn keeps_a_last_sentence_without_terminator() {
        assert_eq!(sentences("A heading"), ["A heading"]);
        assert_eq!(sentences("Done. And then"), ["Done.", "And then"]);
    }

    #[test]
    fn quotes_close_a_sentence() {
        assert_eq!(
            sentences("“Yes,” she said. “Fine.” Then."),
            ["“Yes,” she said.", "“Fine.”", "Then."]
        );
    }

    #[test]
    fn spaces_around_are_left_out() {
        assert_eq!(sentences("  Lead and trail.  "), ["Lead and trail."]);
        assert!(split_sentences("").is_empty());
        assert!(split_sentences("   ").is_empty());
    }
}
