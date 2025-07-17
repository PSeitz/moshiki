use std::hash::Hasher;

use fxhash::FxHasher;

use crate::{Token, tokenizer::TokenTypeTrait};

pub fn fingerprint(tokens: &[Token]) -> u64 {
    let mut hasher = FxHasher::default();
    for token in tokens {
        hasher.write_u8(token.token_type() as u8);
        #[cfg(feature = "whitespace")]
        if let Token::Whitespace(num) = token {
            hasher.write_u32(*num);
        }
    }

    hasher.finish()
}

pub fn fingerprint_types<T: TokenTypeTrait>(tokens: impl Iterator<Item = T>) -> u64 {
    let mut hasher = FxHasher::default();
    for token in tokens {
        hasher.write_u8(token.token_type() as u8);
    }

    hasher.finish()
}
