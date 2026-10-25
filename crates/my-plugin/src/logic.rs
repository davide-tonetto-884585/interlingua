//! The plugin's logic without any screen or device in it.
//!
//! Keeping it apart from the drawing code has a reason you will meet often
//! in embedded Rust: this file is plain Rust, so `cargo test` runs it on
//! your computer in a moment, with no emulator and no reader.

/// Books opened since PaperPaw started.
///
/// `Copy` on purpose: it is a small value, and the plugin keeps it in a
/// `Cell`, which hands out copies rather than references.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counter(u32);

impl Counter {
    /// A counter at zero.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// One more book. It stops at the largest number instead of wrapping
    /// around to zero (`saturating_add`); a device that runs for years
    /// should never show a wrong count.
    pub const fn increment(&mut self) {
        self.0 = self.0.saturating_add(1);
    }

    /// The count.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Counter;

    #[test]
    fn starts_at_zero() {
        assert_eq!(Counter::new().get(), 0);
        assert_eq!(Counter::default(), Counter::new());
    }

    #[test]
    fn counts_up() {
        let mut counter = Counter::new();
        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn stops_at_the_largest_number() {
        let mut counter = Counter(u32::MAX);
        counter.increment();
        assert_eq!(counter.get(), u32::MAX);
    }
}
