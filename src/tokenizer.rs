pub fn tokenize_into<'a>(input: &'a str, tokens: &mut Vec<Token<'a>>) {
    let tokenizer = Tokenizer::new(input);
    for token in tokenizer {
        tokens.push(token);
    }
}

pub fn tokenize(input: &str) -> Vec<Token> {
    Tokenizer::new(input).collect()
}

/// Typed token kinds with zero allocations
#[derive(Debug, PartialEq, Eq)]
pub enum Token<'a> {
    IPv4(&'a str),
    Number(&'a str),
    Uuid(&'a str),
    Word(&'a str),
    Punctuation(&'a str),
    Whitespace(&'a str),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TokenType(pub u8);
impl From<u8> for TokenType {
    fn from(val: u8) -> Self {
        TokenType(val)
    }
}

/// Retrun an ID for each token type
impl<'a> Token<'a> {
    pub fn type_id(&self) -> TokenType {
        let val = match self {
            Token::Word(_) => 0u8,
            Token::Number(_) => 1,
            Token::IPv4(_) => 2,
            Token::Uuid(_) => 3,
            Token::Punctuation(_) => 4,
            Token::Whitespace(_) => 5,
        };
        val.into()
    }
    pub const fn type_id_num_bits() -> u8 {
        3 // 6 types, fits in 3 bits
    }
    pub fn as_str(&self) -> &str {
        match self {
            Token::Word(s)
            | Token::Number(s)
            | Token::IPv4(s)
            | Token::Uuid(s)
            | Token::Punctuation(s)
            | Token::Whitespace(s) => s,
        }
    }
}

/// Quick IPv4 check: four octets 0–255
fn is_ipv4(s: &str) -> bool {
    if s.is_empty()
        || !s.chars().next().unwrap().is_ascii_digit()
        || s.chars().filter(|&c| c == '.').count() != 3
    {
        return false;
    }
    let mut count = 0;
    for part in s.split('.') {
        count += 1;
        if part.is_empty() || part.len() > 3 {
            return false;
        }
        if let Ok(n) = part.parse::<u8>() {
            if n.to_string().len() != part.len() {
                return false;
            }
        } else {
            return false;
        }
    }
    count == 4
}

/// All digits (treat any numeric token as Number)
fn is_number(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Simple UUID v4-ish check (36 chars + hyphens)
fn is_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    s.chars()
        .zip("xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx".chars())
        .all(|(c, p)| {
            if p == '-' {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
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
    pub fn new(input: &'a str) -> Self {
        Tokenizer { input, pos: 0 }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.input.as_bytes();
        let len = bytes.len();
        if self.pos >= len {
            return None;
        }

        // 1) Whitespace (contiguous)
        if bytes[self.pos].is_ascii_whitespace() {
            let start = self.pos;
            while self.pos < len && bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            return Some(Token::Whitespace(&self.input[start..self.pos]));
        }

        // 2) Punctuation (single char), exclude '.', '-', '_'
        let b = bytes[self.pos];
        if b.is_ascii_punctuation() && b != b'.' && b != b'-' && b != b'_' {
            let tok = &self.input[self.pos..self.pos + 1];
            self.pos += 1;
            return Some(Token::Punctuation(tok));
        }

        // 3) Word-like token: scan until next whitespace or punctuation (excluding '.', '-', '_')
        let start = self.pos;
        while self.pos < len {
            let c = bytes[self.pos];
            if c.is_ascii_whitespace()
                || (c.is_ascii_punctuation() && c != b'.' && c != b'-' && c != b'_')
            {
                break;
            }
            self.pos += 1;
        }
        let tok = &self.input[start..self.pos];

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
    fn test_tokenizer_simple() {
        let line = "src: /10.10.34.30:33078, dest: /10.10.34.11:50010";
        let toks: Vec<_> = tokenize(line);
        assert_eq!(
            toks,
            vec![
                Token::Word("src"),
                Token::Punctuation(":"),
                Token::Whitespace(" "),
                Token::Punctuation("/"),
                Token::IPv4("10.10.34.30"),
                Token::Punctuation(":"),
                Token::Number("33078"),
                Token::Punctuation(","),
                Token::Whitespace(" "),
                Token::Word("dest"),
                Token::Punctuation(":"),
                Token::Whitespace(" "),
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
            Token::Whitespace(" "),
            Token::Word("BP-108841162-10.10.34.11-1440074360971"),
            Token::Punctuation(":"),
            Token::Word("blk_1074072698_331874"),
            Token::Punctuation(","),
            Token::Whitespace(" "),
            Token::Word("type"),
            Token::Punctuation("="),
            Token::Word("HAS_DOWNSTREAM_IN_PIPELINE"),
            Token::Whitespace(" "),
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
                | Token::Punctuation(s)
                | Token::Whitespace(s) => *s,
            })
            .collect();
        assert_eq!(reconstructed, line);
    }
}
