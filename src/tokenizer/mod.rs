#[cfg(not(feature = "number_as_string"))]
pub mod number;
#[cfg(feature = "number_as_string")]
pub(crate) mod number_as_string;
/// Token types
pub mod token;
#[cfg(not(feature = "number_as_string"))]
pub use number::*;
#[cfg(feature = "number_as_string")]
pub use number_as_string::*;
pub use token::*;

const WORD_DELIMITER_LOOKUP_TABLE: [bool; 256] = {
    let mut lookup = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        if b.is_ascii_whitespace()
            || (b.is_ascii_punctuation() && b != b'.' && b != b'-' && b != b'_' && b != b'%')
        {
            lookup[i] = true;
        }
        i += 1;
    }
    lookup
};

const WHITESPACE_LOOKUP_TABLE: [bool; 256] = {
    let mut lookup = [false; 256];
    let mut i = 0;
    while i < 256 {
        if (i as u8).is_ascii_whitespace() {
            lookup[i] = true;
        }
        i += 1;
    }
    lookup
};

const PUNCTUATION_LOOKUP_TABLE: [bool; 256] = {
    let mut lookup = [false; 256];
    let mut i = 0;
    while i < 256 {
        if ((i as u8).is_ascii_punctuation() || (i as u8).is_ascii_whitespace())
            && (i as u8) != b'%'
        {
            lookup[i] = true;
        }
        i += 1;
    }
    lookup
};

const DIGIT_LOOKUP_TABLE: [bool; 256] = {
    let mut lookup = [false; 256];
    let mut i = 0;
    while i < 256 {
        if (i as u8).is_ascii_digit() {
            lookup[i] = true;
        }
        i += 1;
    }
    lookup
};

const HEX_DIGIT_LOOKUP_TABLE: [bool; 256] = {
    let mut lookup = [false; 256];
    let mut i = 0;
    while i < 256 {
        if (i as u8).is_ascii_hexdigit() {
            lookup[i] = true;
        }
        i += 1;
    }
    lookup
};

pub(crate) fn tokens_as_string(input: &str, tokens: impl Iterator<Item = Token>) -> Vec<String> {
    tokens.map(|t| t.to_string(input)).collect()
}

/// Zero-allocation tokenizer.
///
/// The Tokenizer implements `Iterator` and can be used to tokenize a string into `Token` objects.
pub struct Tokenizer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Tokenizer<'a> {
    #[inline]
    /// Create a new Tokenizer for the given input string.
    /// The tokenizer is an Iterator that yields `Token` objects.
    pub fn new(input: &'a str) -> Self {
        Tokenizer { input, pos: 0 }
    }
}

#[derive(Copy, Clone)]
enum Kind {
    IPv4,
    Number,
    Uuid,
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // end-of-input
        if self.pos >= self.input.len() {
            return None;
        }

        let bytes = &self.input.as_bytes()[self.pos..];

        // 1) Punctuation
        if PUNCTUATION_LOOKUP_TABLE[bytes[0] as usize] {
            let len = bytes
                .iter()
                .take_while(|&&b| PUNCTUATION_LOOKUP_TABLE[b as usize])
                .count();
            let start = self.pos;
            self.pos += len;
            return Some(Token::Punctuation(start..self.pos));
        }

        // 2) The “classify” table
        let start = self.pos;
        let mut choice: Option<(Kind, usize)> = None;

        // a small table of (matcher → variant).
        #[allow(clippy::type_complexity)]
        let matchers: &[(fn(&[u8]) -> Option<usize>, Kind)] = &[
            (is_ipv4, Kind::IPv4),
            (is_number, Kind::Number),
            (is_uuid_v4, Kind::Uuid),
        ];

        for &(matcher, kind) in matchers {
            if let Some(num_bytes) = matcher(bytes) {
                choice = Some((kind, num_bytes));
                break;
            }
        }

        // build the token (or fallback to a “word” of length word_len)
        let token = if let Some((kind, num_bytes)) = choice {
            self.pos += num_bytes;
            match kind {
                Kind::IPv4 => Token::IPv4(start..self.pos),
                Kind::Number => Token::Number(Number::new(self.input, start..self.pos)),
                Kind::Uuid => Token::Uuid(start..self.pos),
            }
        } else {
            let len = word_len(bytes);
            self.pos += len;
            Token::Word(start..self.pos)
        };

        Some(token)
    }
}

/// Quick IPv4 check: four octets 0–255
/// Returns the number of bytes consumed.
#[inline]
fn is_ipv4(bytes: &[u8]) -> Option<usize> {
    if !DIGIT_LOOKUP_TABLE[bytes[0] as usize] {
        return None;
    }
    let mut i = 0; // current index in `bytes`

    for octet_idx in 0..4 {
        // --- Parse one octet ------------------------------------------------
        let start = i;

        // At least one digit must be present
        if i >= bytes.len() || !DIGIT_LOOKUP_TABLE[bytes[i] as usize] {
            return None;
        }

        let mut val: u16 = 0;
        let mut digit_cnt = 0;

        while i < bytes.len() && DIGIT_LOOKUP_TABLE[bytes[i] as usize] {
            // Convert ASCII digit to numeric value
            val = val * 10 + (bytes[i] - b'0') as u16;
            digit_cnt += 1;
            i += 1;

            // Early bail-out conditions
            if digit_cnt > 3 || val > 255 {
                return None;
            }
        }

        // Reject leading zeros like "01", but allow "0"
        if digit_cnt > 1 && bytes[start] == b'0' {
            return None;
        }

        // --- Expect a dot after the first three octets ----------------------
        if octet_idx < 3 {
            if i >= bytes.len() || bytes[i] != b'.' {
                return None;
            }
            i += 1; // consume the '.'
        }
    }

    Some(i) // number of bytes consumed
}

/// All digits (treat any numeric token as Number)
/// Returns `Some(u32)` if the string is a valid number
/// The parameter is the number of bytes in the token
#[inline]
fn is_number(bytes: &[u8]) -> Option<usize> {
    if !DIGIT_LOOKUP_TABLE[bytes[0] as usize] {
        // Check if the first character is a digit
        return None;
    }
    Some(
        bytes
            .iter()
            .take_while(|&&c| DIGIT_LOOKUP_TABLE[c as usize])
            .count(),
    )
}

/// Simple UUID v4-ish check (8-4-4-4-12 pattern, 36 bytes total)
/// Returns the number of bytes consumed (36) on success.
#[inline]
fn is_uuid_v4(bytes: &[u8]) -> Option<usize> {
    // Quickcheck first character
    if bytes.len() < 36 || !HEX_DIGIT_LOOKUP_TABLE[bytes[0] as usize] {
        return None; // too short or first char is not a hex digit
    }
    // Quickcheck the - separators
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return None; // wrong separator positions
    }
    // Ranges without the separators
    let ranges_to_check = [0..8, 9..13, 14..18, 19..23, 24..36];
    // Check each range for hex digits
    for range in ranges_to_check.iter() {
        for &b in &bytes[range.clone()] {
            if !HEX_DIGIT_LOOKUP_TABLE[b as usize] {
                return None; // non-hex digit found
            }
        }
    }

    Some(36)
}

/// scheme://something   → until first whitespace
#[allow(dead_code)]
fn is_url_chunk(bytes: &[u8]) -> Option<usize> {
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i] == b':' && bytes[i + 1] == b'/' && bytes[i + 2] == b'/' {
            // found, now scan to whitespace
            let len = bytes
                .iter()
                .take_while(|&&b| !WHITESPACE_LOOKUP_TABLE[b as usize])
                .count();
            return Some(len);
        }
        if bytes[i].is_ascii_whitespace() {
            break;
        } // bail early
    }
    None
}

#[inline]
fn word_len(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|&&b| !WORD_DELIMITER_LOOKUP_TABLE[b as usize])
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_keeps_spaces_in_punctuation_tokens() {
        let line = "src: /10.10.34.30:33078, dest: /10.10.34.11:50010";
        let tokens: Vec<_> = Tokenizer::new(line).collect();
        let expected_strings = vec![
            "src",
            ": /",
            "10.10.34.30",
            ":",
            "33078",
            ", ",
            "dest",
            ": /",
            "10.10.34.11",
            ":",
            "50010",
        ];
        let expected_types = vec![
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::IPv4,
            TokenType::Punctuation,
            TokenType::Number,
            TokenType::Punctuation,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::IPv4,
            TokenType::Punctuation,
            TokenType::Number,
        ];

        assert_eq!(
            tokens_as_string(line, tokens.iter().cloned()),
            expected_strings
        );
        assert_eq!(
            tokens
                .iter()
                .map(|token| token.token_type())
                .collect::<Vec<_>>(),
            expected_types
        );
        let reconstructed = tokens
            .iter()
            .map(|token| token.to_string(line))
            .collect::<String>();
        assert_eq!(reconstructed, line);
    }

    #[test]
    fn test_tokenizer_merges_multiple_spaces_into_one_punctuation_token() {
        let line = "alpha   beta";
        let tokens: Vec<_> = Tokenizer::new(line).collect();

        assert_eq!(
            tokens_as_string(line, tokens.iter().cloned()),
            ["alpha", "   ", "beta"]
        );
        assert_eq!(
            tokens
                .iter()
                .map(|token| token.token_type())
                .collect::<Vec<_>>(),
            [TokenType::Word, TokenType::Punctuation, TokenType::Word]
        );
    }
}
