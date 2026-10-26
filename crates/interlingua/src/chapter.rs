//! Translating a chapter, one paragraph at a time: where the job is and
//! which paragraph comes next.
//!
//! The text filter sees only the blocks near the page being laid out, so a
//! whole chapter has to be read from the book itself: the job opens the
//! document, asks for the block at its offset ([`Document::block_at`]),
//! and moves on to the block's end. It does not keep the document between
//! two paragraphs (a second copy of a book is a lot of an X4's heap): the
//! caller opens it each time.
//!
//! A "chapter" is a *section* of the book (an EPUB's file of the spine); a
//! TXT book is one section, so the job stops after [`MOST_PER_RUN`]
//! paragraphs and a second run goes on, skipping what is kept.

use alloc::string::String;

use paperpaw_sdk::Storage;
use paperpaw_sdk::document::{BlockKind, Document, DocumentError, Locator};

/// Paragraphs sent in one run: the next run goes on from there, as the
/// ones kept are skipped.
pub const MOST_PER_RUN: u32 = 1000;

/// A chapter being translated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// The book.
    pub path: String,
    pub section: u32,
    /// Where to look for the next block.
    pub offset: u32,
    /// Where the job stops: the first block that starts at or after it
    /// is not translated.
    pub until: u32,
    /// Paragraphs sent so far, and the one waiting for its answer.
    pub sent: u32,
    pub waiting: Option<u32>,
}

impl Job {
    /// A job for `section` of the book at `path`, from its start to its
    /// end.
    pub fn new(path: &str, section: u32) -> Self {
        Self::range(path, section, 0, u32::MAX)
    }

    /// A job for `section` of the book at `path`, from offset `from` up to
    /// offset `until`.
    pub fn range(path: &str, section: u32, from: u32, until: u32) -> Self {
        Self {
            path: String::from(path),
            section,
            offset: from,
            until,
            sent: 0,
            waiting: None,
        }
    }
}

/// What the job does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Translate this paragraph, which starts at `offset`.
    Paragraph { offset: u32, text: String },
    /// The section is done, or the run's limit is reached
    /// (`more`: there is more to translate in a next run).
    Finished { more: bool },
}

/// The next paragraph of the job's section that `kept` does not have
/// (it is given the paragraph's offset), moving the job on.
pub fn next(
    document: &dyn Document,
    storage: &dyn Storage,
    job: &mut Job,
    kept: &dyn Fn(u32) -> bool,
) -> Result<Next, DocumentError> {
    let end = document.section_len(job.section);
    loop {
        if job.offset >= end || job.offset >= job.until {
            return Ok(Next::Finished { more: false });
        }
        let at = Locator::new(job.section, job.offset);
        let Some(block) = document.block_at(storage, at)? else {
            return Ok(Next::Finished { more: false });
        };
        // A block that does not move on would keep the job here for ever.
        if block.end <= job.offset {
            return Err(DocumentError::Malformed("a block does not advance".into()));
        }
        job.offset = block.end;
        let wanted = block.kind == BlockKind::Paragraph
            && !block.text.trim().is_empty()
            && !kept(block.start);
        if !wanted {
            continue;
        }
        if job.sent >= MOST_PER_RUN {
            return Ok(Next::Finished { more: true });
        }
        job.sent += 1;
        job.waiting = Some(block.start);
        return Ok(Next::Paragraph {
            offset: block.start,
            text: block.text,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use paperpaw_sdk::MemoryStorage;
    use paperpaw_sdk::document::{Block, Metadata};

    /// A section of blocks laid one after the other.
    struct Fake {
        meta: Metadata,
        blocks: Vec<(BlockKind, &'static str)>,
    }

    impl Fake {
        fn start_of(&self, index: usize) -> u32 {
            self.blocks[..index].iter().map(|(_, t)| t.len() as u32 + 1).sum()
        }
    }

    impl Document for Fake {
        fn metadata(&self) -> &Metadata {
            &self.meta
        }
        fn sections(&self) -> u32 {
            1
        }
        fn section_len(&self, _: u32) -> u32 {
            self.start_of(self.blocks.len())
        }
        fn block_at(
            &self,
            _: &dyn Storage,
            at: Locator,
        ) -> Result<Option<Block>, DocumentError> {
            Ok((0..self.blocks.len())
                .find(|&i| self.start_of(i + 1) > at.offset)
                .map(|i| {
                    let (kind, text) = self.blocks[i];
                    let start = self.start_of(i);
                    Block::new(kind, start, start + text.len() as u32 + 1, String::from(text))
                }))
        }
    }

    fn book() -> Fake {
        Fake {
            meta: Metadata::default(),
            blocks: alloc::vec![
                (BlockKind::Heading(1), "Chapter one"),
                (BlockKind::Paragraph, "Alice was tired."),
                (BlockKind::Paragraph, "   "),
                (BlockKind::Paragraph, "She sat down."),
            ],
        }
    }

    fn texts(book: &Fake, kept: &dyn Fn(u32) -> bool) -> Vec<String> {
        let storage = MemoryStorage::new();
        let mut job = Job::new("b.epub", 0);
        let mut out = Vec::new();
        while let Ok(Next::Paragraph { text, .. }) = next(book, &storage, &mut job, kept) {
            out.push(text);
        }
        out
    }

    #[test]
    fn only_the_paragraphs_with_text_come() {
        let out = texts(&book(), &|_| false);
        assert_eq!(out, ["Alice was tired.", "She sat down."]);
    }

    #[test]
    fn what_is_kept_is_skipped() {
        let b = book();
        let first = b.start_of(1);
        let out = texts(&b, &|offset| offset == first);
        assert_eq!(out, ["She sat down."]);
    }

    #[test]
    fn the_job_remembers_the_paragraph_it_waits_for() {
        let b = book();
        let storage = MemoryStorage::new();
        let mut job = Job::new("b.epub", 0);
        let got = next(&b, &storage, &mut job, &|_| false).unwrap();
        assert_eq!(
            got,
            Next::Paragraph {
                offset: b.start_of(1),
                text: String::from("Alice was tired.")
            }
        );
        assert_eq!(job.waiting, Some(b.start_of(1)));
        assert_eq!(job.sent, 1);
    }

    #[test]
    fn a_run_stops_at_its_limit_and_says_there_is_more() {
        let b = book();
        let storage = MemoryStorage::new();
        let mut job = Job::new("b.epub", 0);
        job.sent = MOST_PER_RUN;
        let got = next(&b, &storage, &mut job, &|_| false).unwrap();
        assert_eq!(got, Next::Finished { more: true });
    }

    #[test]
    fn the_end_of_the_section_finishes() {
        let b = book();
        let storage = MemoryStorage::new();
        let mut job = Job::new("b.epub", 0);
        job.offset = b.section_len(0);
        let got = next(&b, &storage, &mut job, &|_| false).unwrap();
        assert_eq!(got, Next::Finished { more: false });
    }

    #[test]
    fn a_job_starts_where_it_is_told_and_stops_at_its_limit() {
        let b = book();
        let storage = MemoryStorage::new();
        // From the second paragraph on, up to the middle of the third.
        let from = b.start_of(1);
        let mut job = Job::range("b.epub", 0, from, b.start_of(3));
        let got = next(&b, &storage, &mut job, &|_| false).unwrap();
        assert_eq!(
            got,
            Next::Paragraph {
                offset: from,
                text: String::from("Alice was tired.")
            }
        );
        // The blank block, then the limit: the last paragraph is left out.
        let got = next(&b, &storage, &mut job, &|_| false).unwrap();
        assert_eq!(got, Next::Finished { more: false });
    }
}
