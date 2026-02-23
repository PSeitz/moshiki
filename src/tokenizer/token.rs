use serde::{Deserialize, Serialize};
use std::ops::Range;

use super::Number;

/// Typed token kinds with zero allocations
#[derive(Debug, Clone)]
pub enum Token {
    /// IPv4 address
    IPv4(Range<usize>),
    /// Number
    Number(Number), // u64 little endian representation
    /// UUID
    Uuid(Range<usize>),
    /// The default token
    Word(Range<usize>),
    /// Punctuation token
    Punctuation(Range<usize>),
}

impl Token {
    /// Compares with another token to see if they are the same type, but NOT range.
    #[inline]
    pub fn matches(&self, other: &Token) -> bool {
        match (self, other) {
            (Token::Word(_), Token::Word(_)) => true,
            (Token::Number(_), Token::Number(_)) => true,
            (Token::IPv4(_), Token::IPv4(_)) => true,
            (Token::Uuid(_), Token::Uuid(_)) => true,
            (Token::Punctuation(_), Token::Punctuation(_)) => true,
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
#[repr(u8)]
/// The type of the token, used for fingerprinting and coloring
pub enum TokenType {
    /// Word
    Word = 1,
    /// Number
    Number = 2,
    /// IPv4
    IPv4 = 3,
    /// UUID
    Uuid = 4,
    /// Punctuation
    Punctuation = 5,
}

impl TokenType {
    /// Single colored char representation of the token type.
    /// Good the see patterns.
    pub fn get_color_code(&self) -> &'static str {
        match self {
            TokenType::Word => "W",
            TokenType::Number => concat!("\x1b[33m", "N", "\x1b[0m"),
            TokenType::IPv4 => concat!("\x1b[34m", "I", "\x1b[0m"),
            TokenType::Uuid => concat!("\x1b[35m", "U", "\x1b[0m"),
            TokenType::Punctuation => "P",
        }
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
            _ => panic!("Invalid token type"),
        }
    }
}

/// Trait to get the token type from a token
pub trait TokenTypeTrait {
    /// Returns the token type of the token
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
        }
    }
}

/// Retrun an ID for each token type
impl Token {
    #[inline]
    pub(crate) fn to_string(&self, input: &str) -> String {
        match self {
            Token::Word(r) | Token::IPv4(r) | Token::Uuid(r) | Token::Punctuation(r) => {
                input[r.start..r.end].to_string()
            }
            Token::Number(num) => num.to_string(input),
        }
    }

    #[inline]
    pub(crate) fn as_bytes<'a>(&'a self, input: &'a str) -> Option<&'a [u8]> {
        match self {
            Token::Word(r) | Token::IPv4(r) | Token::Uuid(r) | Token::Punctuation(r) => {
                Some(&input.as_bytes()[r.start..r.end])
            }
            Token::Number(n) => Some(n.as_bytes(input)),
        }
    }
}
