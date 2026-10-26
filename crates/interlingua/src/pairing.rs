//! Which part of a paragraph's translation belongs to each of its
//! sentences: the alignment an interlinear reading needs, so that each
//! sentence's translation starts above the sentence.
//!
//! **An exercise.** [`pair`] is to be written: its tests below say what it
//! must do and are `#[ignore]`d until it does (`cargo test -p interlingua
//! -- --ignored` runs them). The rule is the one CrossLingua Reader uses
//! (MIT, `src/modules/lingua/utils/SentencePairing.h`), from its idea
//! only: no code of it is here.
//!
//! The rule, in characters (bytes are fine for a first version):
//!
//! 1. Split both texts into sentences ([`split_sentences`]).
//! 2. Each **translation** sentence has a midpoint: the middle of its
//!    range, as a fraction of the translation's length (0.0 to 1.0).
//! 3. Each **source** sentence covers a part of its paragraph, as a
//!    fraction of the source's length: from where it starts to where the
//!    next one starts (the last one to 1.0).
//! 4. A source sentence takes every translation sentence whose midpoint
//!    falls in its part. Those are always next to each other, so one
//!    range of the translation holds them.
//! 5. A source sentence that takes none takes the translation sentence
//!    whose midpoint is closest to its own middle. (So two source
//!    sentences that a translator merged into one share that one.)
//!
//! Things to learn on the way: `Range<usize>` and slicing, iterators
//! (`iter`, `filter`, `min_by`), `f32` and why `partial_cmp` returns an
//! `Option`, and empty ranges as "nothing".

use alloc::vec::Vec;
use core::ops::Range;

pub use crate::text::split_sentences;

/// For each sentence of `source`, in the order [`split_sentences`] gives
/// them, the range of `translation` that translates it. An empty range
/// (`0..0`) means no translation, e.g. when `translation` is empty.
///
/// Two source sentences may get the same range: their translation is one
/// sentence. The layer then draws it once, above the first of them.
pub fn pair(source: &str, translation: &str) -> Vec<Range<usize>> {
    let src = split_sentences(source);
    let tr = split_sentences(translation);
    if tr.is_empty() {
        return src.iter().map(|_| 0..0).collect();
    }

    // Each translation sentence's midpoint, as a fraction of the text.
    let tr_len = translation.len() as f32;
    let mids: Vec<f32> = tr
        .iter()
        .map(|r| (r.start + r.end) as f32 / 2.0 / tr_len)
        .collect();

    let src_len = source.len() as f32;
    let mut out = Vec::with_capacity(src.len());
    for (i, sentence) in src.iter().enumerate() {
        // The part of the paragraph this sentence covers.
        let from = sentence.start as f32 / src_len;
        let to = src.get(i + 1).map_or(1.0, |next| next.start as f32 / src_len);
        let last = i + 1 == src.len();
        let inside = |m: f32| m >= from && (m < to || (last && m <= to));

        let first = mids.iter().position(|&m| inside(m));
        let range = match first {
            Some(first) => {
                // Midpoints grow with the index, so the taken ones are
                // next to each other.
                let end = first + mids[first..].iter().take_while(|&&m| inside(m)).count();
                tr[first].start..tr[end - 1].end
            }
            None => {
                let middle = (from + to) / 2.0;
                let (closest, _) = mids
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        (*a - middle).abs().total_cmp(&(*b - middle).abs())
                    })
                    .expect("tr is not empty");
                tr[closest].clone()
            }
        };
        out.push(range);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pairs as text, for readable assertions.
    fn pairs<'a>(source: &str, translation: &'a str) -> Vec<&'a str> {
        pair(source, translation)
            .into_iter()
            .map(|r| &translation[r])
            .collect()
    }

    #[test]
    fn one_sentence_each() {
        assert_eq!(
            pairs("Hi. How are you today?", "Ciao. Come stai oggi?"),
            ["Ciao.", "Come stai oggi?"]
        );
    }

    #[test]
    fn one_source_sentence_translated_as_two() {
        assert_eq!(
            pairs("A long sentence here.", "Una frase. Lunga qui."),
            ["Una frase. Lunga qui."]
        );
    }

    #[test]
    fn two_source_sentences_translated_as_one() {
        // The translation's one midpoint is in the first sentence's part;
        // the second takes the closest one, the same.
        assert_eq!(
            pairs("Aaaa bbbb. Cccc dddd.", "Xxxx yyyy zzzz."),
            ["Xxxx yyyy zzzz.", "Xxxx yyyy zzzz."]
        );
    }

    #[test]
    fn no_translation_is_empty_ranges() {
        assert_eq!(pair("One. Two.", ""), [0..0, 0..0]);
        assert!(pair("", "Uno.").is_empty());
    }
}
