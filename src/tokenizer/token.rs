use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Typed token kinds with zero allocations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Token {
    IPv4(Range<u32>),
    Number(Number), // u64 little endian representation
    Uuid(Range<u32>),
    Word(Range<u32>),
    Punctuation(Range<u32>),
    Whitespace(u32),
    CatchAll(Range<u32>),
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Number {
    F64([u8; 8]),
    /// Represents u64 as little-endian bytes
    U64([u8; 8]),
}
impl From<u64> for Number {
    #[inline]
    fn from(num: u64) -> Self {
        Number::U64(num.to_le_bytes())
    }
}
impl From<f64> for Number {
    #[inline]
    fn from(num: f64) -> Self {
        Number::F64(num.to_le_bytes())
    }
}

impl Number {
    #[inline]
    pub fn new(input: &str, range: Range<usize>) -> Self {
        let num_str = &input[range];
        let number = num_str.parse::<u64>();
        match number {
            Ok(n) => n.into(),
            Err(_) => {
                let num = num_str
                    .parse::<f64>()
                    .expect("Failed to parse number as f64");
                num.into()
            }
        }
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Number::F64(bytes) => bytes,
            Number::U64(bytes) => bytes,
        }
    }
    pub fn to_string(&self, _input: &str) -> String {
        match self {
            Number::F64(bytes) => f64::from_le_bytes(*bytes).to_string(),
            Number::U64(bytes) => u64::from_le_bytes(*bytes).to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TokenType {
    Word = 1,
    Number = 2,
    IPv4 = 3,
    Uuid = 4,
    Punctuation = 5,
    Whitespace = 6,
    CatchAll = 7,
}

impl TokenType {
    pub fn is_catch_all(&self) -> bool {
        *self == TokenType::CatchAll
    }
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
            7 => TokenType::CatchAll,
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
            Token::CatchAll(_) => TokenType::CatchAll,
        }
    }

    #[inline]
    pub const fn type_id_num_bits() -> u8 {
        3 // 7 token types fit in 3 bits (2^3 = 8)
    }
    #[inline]
    pub fn to_string(&self, input: &str) -> String {
        match self {
            Token::Word(r)
            | Token::IPv4(r)
            | Token::Uuid(r)
            | Token::CatchAll(r)
            | Token::Punctuation(r) => input[r.start as usize..r.end as usize].to_string(),
            Token::Whitespace(num) => " ".repeat(*num as usize),
            Token::Number(num) => num.to_string(input),
        }
    }

    #[inline]
    pub fn as_bytes<'a>(&'a self, input: &'a str) -> Option<&'a [u8]> {
        match self {
            Token::Word(r)
            | Token::IPv4(r)
            | Token::Uuid(r)
            | Token::CatchAll(r)
            | Token::Punctuation(r) => Some(&input.as_bytes()[r.start as usize..r.end as usize]),
            Token::Number(n) => Some(n.as_bytes()),
            // White is ignored for now
            Token::Whitespace(_) => None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_whitespace(&self) -> bool {
        matches!(self, Token::Whitespace(_))
    }
}
