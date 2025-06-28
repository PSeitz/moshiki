pub fn tokenize_into<'a>(input: &'a str, tokens: &mut Vec<Token<'a>>) {
    let tokenizer = Tokenizer::new(input);
    for token in tokenizer {
        tokens.push(token);
    }
}

pub fn tokenize(input: &str) -> Vec<Token> {
    Tokenizer::new(input).collect()
}

pub fn reconstruct_from_tokens<'a>(tokens: impl Iterator<Item = Token<'a>>) -> String {
    tokens
        .map(|t| match t {
            Token::IPv4(s)
            | Token::Number(s)
            | Token::Uuid(s)
            | Token::Word(s)
            | Token::Punctuation(s) => s.to_string(),
            Token::Whitespace(s) => " ".repeat(s),
        })
        .collect()
}

/// Typed token kinds with zero allocations
#[derive(Debug, PartialEq, Eq)]
pub enum Token<'a> {
    IPv4(&'a str),
    Number(&'a str),
    Uuid(&'a str),
    Word(&'a str),
    Punctuation(&'a str),
    Whitespace(usize),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TokenType(pub u8);
impl TokenType {
    pub fn is_whitespace(&self) -> bool {
        self.0 == 6 // Whitespace token type ID
    }
}

impl From<u8> for TokenType {
    #[inline]
    fn from(val: u8) -> Self {
        TokenType(val)
    }
}

/// Retrun an ID for each token type
impl<'a> Token<'a> {
    #[inline]
    /// They start from 1, so we can use them for the fingerprint and differentiate from
    /// doesn't exist token type (0).
    pub fn token_type(&self) -> TokenType {
        let val = match self {
            Token::Word(_) => 1u8,
            Token::Number(_) => 2,
            Token::IPv4(_) => 3,
            Token::Uuid(_) => 4,
            Token::Punctuation(_) => 5,
            Token::Whitespace(_) => 6,
        };
        val.into()
    }

    #[inline]
    pub const fn type_id_num_bits() -> u8 {
        3 // 6 token types fit in 3 bits (2^3 = 8)
    }
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Token::Word(s)
            | Token::Number(s)
            | Token::IPv4(s)
            | Token::Uuid(s)
            | Token::Punctuation(s) => Some(s),
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
    pos: usize,
}

impl<'a> Tokenizer<'a> {
    #[inline]
    pub fn new(input: &'a str) -> Self {
        Tokenizer { input, pos: 0 }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let remaining = &self.input[self.pos..];
        if remaining.is_empty() {
            return None;
        }

        let bytes = remaining.as_bytes();

        // 1) Whitespace (contiguous)
        if bytes[0].is_ascii_whitespace() {
            let len = bytes
                .iter()
                .take_while(|&&b| b.is_ascii_whitespace())
                .count();
            self.pos += len;
            return Some(Token::Whitespace(len));
        }

        // 2) Punctuation (contiguous), exclude '.', '-', '_'
        if bytes[0].is_ascii_punctuation()
            && bytes[0] != b'.'
            && bytes[0] != b'-'
            && bytes[0] != b'_'
        {
            let mut len = 0;
            while self.pos + len < self.input.len()
                && bytes[len].is_ascii_punctuation()
                && bytes[len] != b'.'
                && bytes[len] != b'-'
                && bytes[len] != b'_'
            {
                len += 1;
            }
            let tok = &remaining[0..len];
            self.pos += len;
            return Some(Token::Punctuation(tok));
        }

        // 3) Word-like token: scan until next whitespace or punctuation (excluding '.', '-', '_')
        let len = bytes
            .iter()
            .take_while(|&&b| {
                !(b.is_ascii_whitespace() || (b.is_ascii_punctuation() && b != b'.' && b != b'-' && b != b'_'))
            })
            .count();
        let tok = &remaining[0..len];
        self.pos += len;

        // 4) Classify
        let token = if is_ipv4(tok) {
            Token::IPv4(tok)
        } else if is_number(tok) {
            Token::Number(tok)
        } else if is_uuid(tok) {
            Token::Uuid(tok)
        } else {
            Token::Word(tok)
        };
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_is_whitespace() {
        assert!(TokenType(6).is_whitespace());
    }

    #[test]
    fn test_tokenizer_simple() {
        let line = "src: /10.10.34.30:33078, dest: /10.10.34.11:50010";
        let toks: Vec<_> = tokenize(line);
        assert_eq!(
            toks,
            vec![
                Token::Word("src"),
                Token::Punctuation(":"),
                Token::Whitespace(1),
                Token::Punctuation("/"),
                Token::IPv4("10.10.34.30"),
                Token::Punctuation(":"),
                Token::Number("33078"),
                Token::Punctuation(","),
                Token::Whitespace(1),
                Token::Word("dest"),
                Token::Punctuation(":"),
                Token::Whitespace(1),
                Token::Punctuation("/"),
                Token::IPv4("10.10.34.11"),
                Token::Punctuation(":"),
                Token::Number("50010"),
            ]
        );
    }

    #[test]
    fn test_packet_expected_and_reconstruction() {
        let line = "PacketResponder: BP-108841162-10.10.34.11-1440074360971:blk_1074072698_331874, type=HAS_DOWNSTREAM_IN_PIPELINE terminating";
        let toks: Vec<_> = tokenize(line);
        let expected = vec![
            Token::Word("PacketResponder"),
            Token::Punctuation(":"),
            Token::Whitespace(1),
            Token::Word("BP-108841162-10.10.34.11-1440074360971"),
            Token::Punctuation(":"),
            Token::Word("blk_1074072698_331874"),
            Token::Punctuation(","),
            Token::Whitespace(1),
            Token::Word("type"),
            Token::Punctuation("="),
            Token::Word("HAS_DOWNSTREAM_IN_PIPELINE"),
            Token::Whitespace(1),
            Token::Word("terminating"),
        ];
        assert_eq!(toks, expected);

        let reconstructed: String = toks
            .iter()
            .map(|t| match t {
                Token::IPv4(s)
                | Token::Number(s)
                | Token::Uuid(s)
                | Token::Word(s)
                | Token::Punctuation(s) => s.to_string(),
                Token::Whitespace(s) => " ".repeat(*s),
            })
            .collect();
        assert_eq!(reconstructed, line);
    }

    #[test]
    fn test_tokenizer_log_line() {
        let line = "src: /10.10.34.11:52611, dest: /10.10.34.42:50010, bytes: 162571, op: HDFS_WRITE, cliID: DFSClient_NONMAPREDUCE_-941064892_1, offset: 0, srvID: ac6cb715-a2bc-4644-aaa4-10fcbd1c390e, blockid: BP-108841162-10.10.34.11-1440074360971:blk_1073854279_113455, duration: 3374681";
        let toks: Vec<_> = tokenize(line);

        use Token::*;
        let expected = vec![
            Word("src"),
            Punctuation(":"),
            Whitespace(1),
            Punctuation("/"),
            IPv4("10.10.34.11"),
            Punctuation(":"),
            Number("52611"),
            Punctuation(","),
            Whitespace(1),
            Word("dest"),
            Punctuation(":"),
            Whitespace(1),
            Punctuation("/"),
            IPv4("10.10.34.42"),
            Punctuation(":"),
            Number("50010"),
            Punctuation(","),
            Whitespace(1),
            Word("bytes"),
            Punctuation(":"),
            Whitespace(1),
            Number("162571"),
            Punctuation(","),
            Whitespace(1),
            Word("op"),
            Punctuation(":"),
            Whitespace(1),
            Word("HDFS_WRITE"),
            Punctuation(","),
            Whitespace(1),
            Word("cliID"),
            Punctuation(":"),
            Whitespace(1),
            Word("DFSClient_NONMAPREDUCE_-941064892_1"),
            Punctuation(","),
            Whitespace(1),
            Word("offset"),
            Punctuation(":"),
            Whitespace(1),
            Number("0"),
            Punctuation(","),
            Whitespace(1),
            Word("srvID"),
            Punctuation(":"),
            Whitespace(1),
            Uuid("ac6cb715-a2bc-4644-aaa4-10fcbd1c390e"),
            Punctuation(","),
            Whitespace(1),
            Word("blockid"),
            Punctuation(":"),
            Whitespace(1),
            Word("BP-108841162-10.10.34.11-1440074360971"),
            Punctuation(":"),
            Word("blk_1073854279_113455"),
            Punctuation(","),
            Whitespace(1),
            Word("duration"),
            Punctuation(":"),
            Whitespace(1),
            Number("3374681"),
        ];
        assert_eq!(toks, expected);
    }
}
