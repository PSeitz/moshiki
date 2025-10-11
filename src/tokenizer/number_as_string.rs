use std::ops::Range;

/// Represents a substring of the input, identified by its byte range.
/// Stores only the raw text slice; no numeric parsing is performed.
#[derive(Debug, Clone)]
pub struct Number {
    /// The byte-range of the original substring in the input.
    str_range: Range<usize>,
}

impl Number {
    /// Constructs a `Number` by capturing the range of characters to treat as a string.
    #[inline]
    pub fn new(_input: &str, range: Range<usize>) -> Self {
        Number { str_range: range }
    }

    /// Returns the original substring slice from the input.
    #[inline]
    pub fn as_bytes<'a>(&self, input: &'a str) -> &'a [u8] {
        &input.as_bytes()[self.str_range.start..self.str_range.end]
    }

    /// Converts the stored substring into a standalone `String`.
    #[inline]
    pub fn to_string(&self, input: &str) -> String {
        input[self.str_range.start..self.str_range.end].to_string()
    }
}
