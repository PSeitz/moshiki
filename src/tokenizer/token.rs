use serde::{Deserialize, Serialize};
use std::ops::Range;

use super::Number;

/// Typed token kinds with zero allocations
#[derive(Debug, Clone)]
pub enum Token {
    IPv4(Range<u32>),
    Number(Number), // u64 little endian representation
    Uuid(Range<u32>),
    Word(Range<u32>),
    Punctuation(Range<u32>),
    #[cfg(feature = "whitespace")]
    Whitespace(u32),
}

impl Token {
    /// Compares with another token to see if they are the same type, but NOT range.
    /// Whitespace tokens are only considered equal if they have the same number of spaces.
    #[inline]
    pub fn matches(&self, other: &Token) -> bool {
        match (self, other) {
            (Token::Word(_), Token::Word(_)) => true,
            (Token::Number(_), Token::Number(_)) => true,
            (Token::IPv4(_), Token::IPv4(_)) => true,
            (Token::Uuid(_), Token::Uuid(_)) => true,
            (Token::Punctuation(_), Token::Punctuation(_)) => true,
            #[cfg(feature = "whitespace")]
            (Token::Whitespace(num1), Token::Whitespace(num2)) => num1 == num2,
            _ => false,
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
    #[cfg(feature = "whitespace")]
    Whitespace = 6,
}

impl TokenType {
    pub fn get_color_code(&self) -> &'static str {
        match self {
            TokenType::Word => "W",
            TokenType::Number => concat!("\x1b[33m", "N", "\x1b[0m"),
            TokenType::IPv4 => concat!("\x1b[34m", "I", "\x1b[0m"),
            TokenType::Uuid => concat!("\x1b[35m", "U", "\x1b[0m"),
            TokenType::Punctuation => "P",
            #[cfg(feature = "whitespace")]
            TokenType::Whitespace => concat!("\x1b[36m", "S", "\x1b[0m"),
        }
    }

    #[cfg(feature = "whitespace")]
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
            #[cfg(feature = "whitespace")]
            6 => TokenType::Whitespace,
            _ => panic!("Invalid token type"),
        }
    }
}

pub trait TokenTypeTrait {
    fn token_type(&self) -> TokenType;
}
impl TokenTypeTrait for TokenType {
    #[inline]
    fn token_type(&self) -> TokenType {
        *self
    }
}
impl TokenTypeTrait for Token {
    /// They start from 1, so we can use them for the fingerprint and differentiate from
    /// doesn't exist token type (0).
    #[inline]
    fn token_type(&self) -> TokenType {
        match self {
            Token::Word(_) => TokenType::Word,
            Token::Number(_) => TokenType::Number,
            Token::IPv4(_) => TokenType::IPv4,
            Token::Uuid(_) => TokenType::Uuid,
            Token::Punctuation(_) => TokenType::Punctuation,
            #[cfg(feature = "whitespace")]
            Token::Whitespace(_) => TokenType::Whitespace,
        }
    }
}

/// Retrun an ID for each token type
impl Token {
    #[inline]
    pub const fn type_id_num_bits() -> u8 {
        3 // 7 token types fit in 3 bits (2^3 = 8)
    }
    #[inline]
    pub fn to_string(&self, input: &str) -> String {
        match self {
            Token::Word(r) | Token::IPv4(r) | Token::Uuid(r) | Token::Punctuation(r) => {
                input[r.start as usize..r.end as usize].to_string()
            }
            #[cfg(feature = "whitespace")]
            Token::Whitespace(num) => " ".repeat(*num as usize),
            Token::Number(num) => num.to_string(input),
        }
    }

    #[inline]
    pub fn as_bytes<'a>(&'a self, input: &'a str) -> Option<&'a [u8]> {
        match self {
            Token::Word(r) | Token::IPv4(r) | Token::Uuid(r) | Token::Punctuation(r) => {
                Some(&input.as_bytes()[r.start as usize..r.end as usize])
            }
            Token::Number(n) => Some(n.as_bytes(input)),
            // White is ignored for now
            #[cfg(feature = "whitespace")]
            Token::Whitespace(_) => None,
        }
    }

    #[allow(dead_code)]
    #[cfg(feature = "whitespace")]
    pub(crate) fn is_whitespace(&self) -> bool {
        matches!(self, Token::Whitespace(_))
    }
}
