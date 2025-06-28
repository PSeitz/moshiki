use std::ops::Range;

pub fn tokenize_into(input: &str, tokens: &mut Vec<Token>) {
    let tokenizer = Tokenizer::new(input);
    for token in tokenizer {
        tokens.push(token);
    }
}

pub fn tokenize(input: &str) -> Vec<Token> {
    Tokenizer::new(input).collect()
}

pub fn reconstruct_from_tokens(input: &str, tokens: impl Iterator<Item = Token>) -> String {
    tokens
        .map(|t| match t {
            Token::IPv4(r)
            | Token::Number(r)
            | Token::Uuid(r)
            | Token::Word(r)
            | Token::Punctuation(r) => input[r.start as usize..r.end as usize].to_string(),
            Token::Whitespace(s) => " ".repeat(s as usize),
        })
        .collect()
}

/// Typed token kinds with zero allocations
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Token {
    IPv4(Range<u32>),
    Number(Range<u32>),
    Uuid(Range<u32>),
    Word(Range<u32>),
    Punctuation(Range<u32>),
    Whitespace(u32),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum TokenType {
    Word = 1,
    Number = 2,
    IPv4 = 3,
    Uuid = 4,
    Punctuation = 5,
    Whitespace = 6,
}

impl TokenType {
    pub fn is_whitespace(&self) -> bool {
        *self == TokenType::Whitespace
    }
}

impl From<u8> for TokenType {
    #[inline]
    fn from(val: u8) -> Self {
        match val {
            1 => TokenType::Word,
            2 => TokenType::Number,
            3 => TokenType::IPv4,
            4 => TokenType::Uuid,
            5 => TokenType::Punctuation,
            6 => TokenType::Whitespace,
            _ => panic!("Invalid token type"),
        }
    }
}

/// Retrun an ID for each token type
impl Token {
    #[inline]
    /// They start from 1, so we can use them for the fingerprint and differentiate from
    /// doesn't exist token type (0).
    pub fn token_type(&self) -> TokenType {
        match self {
            Token::Word(_) => TokenType::Word,
            Token::Number(_) => TokenType::Number,
            Token::IPv4(_) => TokenType::IPv4,
            Token::Uuid(_) => TokenType::Uuid,
            Token::Punctuation(_) => TokenType::Punctuation,
            Token::Whitespace(_) => TokenType::Whitespace,
        }
    }

    #[inline]
    pub const fn type_id_num_bits() -> u8 {
        3 // 6 token types fit in 3 bits (2^3 = 8)
    }
    #[inline]
    pub fn as_str<'a>(&self, input: &'a str) -> Option<&'a str> {
        match self {
            Token::Word(r)
            | Token::Number(r)
            | Token::IPv4(r)
            | Token::Uuid(r)
            | Token::Punctuation(r) => Some(&input[r.start as usize..r.end as usize]),
            Token::Whitespace(_) => None,
        }
    }
}

/// Quick IPv4 check: four octets 0–255
#[inline]
fn is_ipv4(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    let mut octet_count = 0;
    let mut current_octet_val = 0;
    let mut current_octet_len = 0;
    let mut prev_char_was_digit = false;

    for &b in bytes {
        if b == b'.' {
            if octet_count == 3 || current_octet_len == 0 || !prev_char_was_digit {
                return false;
            }
            octet_count += 1;
            current_octet_val = 0;
            current_octet_len = 0;
            prev_char_was_digit = false;
        } else if b.is_ascii_digit() {
            current_octet_val = current_octet_val * 10 + (b - b'0') as u16;
            current_octet_len += 1;
            if current_octet_val > 255
                || current_octet_len > 3
                || (current_octet_len > 1
                    && current_octet_val < 10
                    && bytes[bytes.len() - current_octet_len] == b'0')
            {
                return false;
            }
            prev_char_was_digit = true;
        } else {
            return false;
        }
    }

    octet_count == 3 && current_octet_len > 0 && prev_char_was_digit
}

/// All digits (treat any numeric token as Number)
#[inline]
fn is_number(s: &str) -> bool {
    !s.is_empty() && s.as_bytes().iter().all(|&c| c.is_ascii_digit())
}

/// Simple UUID v4-ish check (36 chars + hyphens)
#[inline]
fn is_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }

    // Check for hyphens at correct positions and hex digits elsewhere
    (bytes[8] == b'-' &&
     bytes[13] == b'-' &&
     bytes[18] == b'-' &&
     bytes[23] == b'-') &&

    // Check all other characters are hex digits
    bytes.iter().enumerate().all(|(i, &c)| {
        match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        }
    })
}

/// Zero-allocation tokenizer: splits on whitespace and ASCII punctuation
/// (excluding '.', '-', and '_' so tokens like IPs, hyphenated IDs, and snake_case stay intact)
pub struct Tokenizer<'a> {
    input: &'a str,
    pos: u32,
}

impl<'a> Tokenizer<'a> {
    #[inline]
    pub fn new(input: &'a str) -> Self {
        Tokenizer { input, pos: 0 }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos as usize >= self.input.len() {
            return None;
        }

        let bytes = &self.input.as_bytes()[self.pos as usize..];

        // 1) Whitespace (contiguous)
        if bytes[0].is_ascii_whitespace() {
            let len = bytes
                .iter()
                .take_while(|&&b| b.is_ascii_whitespace())
                .count();
            self.pos += len as u32;
            return Some(Token::Whitespace(len as u32));
        }

        let start = self.pos;

        // 2) Punctuation (contiguous), exclude '.', '-', '_'
        if bytes[0].is_ascii_punctuation()
            && bytes[0] != b'.'
            && bytes[0] != b'-'
            && bytes[0] != b'_'
        {
            let len = bytes
                .iter()
                .take_while(|&&b| b.is_ascii_punctuation() && b != b'.' && b != b'-' && b != b'_')
                .count();
            self.pos += len as u32;
            return Some(Token::Punctuation(start..self.pos));
        }

        // 3) Word-like token: scan until next whitespace or punctuation (excluding '.', '-', '_')
        let len = bytes
            .iter()
            .take_while(|&&b| {
                !(b.is_ascii_whitespace()
                    || (b.is_ascii_punctuation() && b != b'.' && b != b'-' && b != b'_'))
            })
            .count();

        let tok_str = &self.input[start as usize..(start as usize + len)];
        self.pos += len as u32;

        // 4) Classify
        let token = if is_ipv4(tok_str) {
            Token::IPv4(start..self.pos)
        } else if is_number(tok_str) {
            Token::Number(start..self.pos)
        } else if is_uuid(tok_str) {
            Token::Uuid(start..self.pos)
        } else {
            Token::Word(start..self.pos)
        };
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_is_whitespace() {
        assert!(TokenType::Whitespace.is_whitespace());
    }

    #[test]
    fn test_tokenizer_simple() {
        let line = "src: /10.10.34.30:33078, dest: /10.10.34.11:50010";
        let toks: Vec<_> = tokenize(line);
        let expected_strs = vec![
            "src",
            ":",
            " ",
            "/",
            "10.10.34.30",
            ":",
            "33078",
            ",",
            " ",
            "dest",
            ":",
            " ",
            "/",
            "10.10.34.11",
            ":",
            "50010",
        ];
        let expected_types = vec![
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Punctuation,
            TokenType::IPv4,
            TokenType::Punctuation,
            TokenType::Number,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Punctuation,
            TokenType::IPv4,
            TokenType::Punctuation,
            TokenType::Number,
        ];

        for (i, (tok, expected_str)) in toks.iter().zip(expected_strs.iter()).enumerate() {
            assert_eq!(tok.token_type(), expected_types[i]);
            match tok {
                Token::Whitespace(len) => assert_eq!(*len as usize, expected_str.len()),
                _ => assert_eq!(tok.as_str(line).unwrap(), *expected_str),
            }
        }

        let reconstructed = reconstruct_from_tokens(line, toks.into_iter());
        assert_eq!(reconstructed, line);
    }

    #[test]
    fn test_packet_expected_and_reconstruction() {
        let line = "PacketResponder: BP-108841162-10.10.34.11-1440074360971:blk_1074072698_331874, type=HAS_DOWNSTREAM_IN_PIPELINE terminating";
        let toks: Vec<_> = tokenize(line);
        let expected_strs = vec![
            "PacketResponder",
            ":",
            " ",
            "BP-108841162-10.10.34.11-1440074360971",
            ":",
            "blk_1074072698_331874",
            ",",
            " ",
            "type",
            "=",
            "HAS_DOWNSTREAM_IN_PIPELINE",
            " ",
            "terminating",
        ];
        let expected_types = vec![
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Word,
            TokenType::Whitespace,
            TokenType::Word,
        ];

        for (i, (tok, expected_str)) in toks.iter().zip(expected_strs.iter()).enumerate() {
            assert_eq!(tok.token_type(), expected_types[i]);
            match tok {
                Token::Whitespace(len) => assert_eq!(*len as usize, expected_str.len()),
                _ => assert_eq!(tok.as_str(line).unwrap(), *expected_str),
            }
        }

        let reconstructed = reconstruct_from_tokens(line, toks.into_iter());
        assert_eq!(reconstructed, line);
    }

    #[test]
    fn test_tokenizer_log_line() {
        let line = "src: /10.10.34.11:52611, dest: /10.10.34.42:50010, bytes: 162571, op: HDFS_WRITE, cliID: DFSClient_NONMAPREDUCE_-941064892_1, offset: 0, srvID: ac6cb715-a2bc-4644-aaa4-10fcbd1c390e, blockid: BP-108841162-10.10.34.11-1440074360971:blk_1073854279_113455, duration: 3374681";
        let toks: Vec<_> = tokenize(line);

        let expected_strs = vec![
            "src",
            ":",
            " ",
            "/",
            "10.10.34.11",
            ":",
            "52611",
            ",",
            " ",
            "dest",
            ":",
            " ",
            "/",
            "10.10.34.42",
            ":",
            "50010",
            ",",
            " ",
            "bytes",
            ":",
            " ",
            "162571",
            ",",
            " ",
            "op",
            ":",
            " ",
            "HDFS_WRITE",
            ",",
            " ",
            "cliID",
            ":",
            " ",
            "DFSClient_NONMAPREDUCE_-941064892_1",
            ",",
            " ",
            "offset",
            ":",
            " ",
            "0",
            ",",
            " ",
            "srvID",
            ":",
            " ",
            "ac6cb715-a2bc-4644-aaa4-10fcbd1c390e",
            ",",
            " ",
            "blockid",
            ":",
            " ",
            "BP-108841162-10.10.34.11-1440074360971",
            ":",
            "blk_1073854279_113455",
            ",",
            " ",
            "duration",
            ":",
            " ",
            "3374681",
        ];

        let expected_types = vec![
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Punctuation,
            TokenType::IPv4,
            TokenType::Punctuation,
            TokenType::Number,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Punctuation,
            TokenType::IPv4,
            TokenType::Punctuation,
            TokenType::Number,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Number,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Number,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Uuid,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Word,
            TokenType::Punctuation,
            TokenType::Whitespace,
            TokenType::Number,
        ];

        for (i, (tok, expected_str)) in toks.iter().zip(expected_strs.iter()).enumerate() {
            assert_eq!(tok.token_type(), expected_types[i]);
            match tok {
                Token::Whitespace(len) => assert_eq!(*len as usize, expected_str.len()),
                _ => assert_eq!(tok.as_str(line).unwrap(), *expected_str),
            }
        }
    }
}
