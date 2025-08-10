use std::hash::Hasher;

use fxhash::FxHasher;

use crate::{Token, tokenizer::TokenTypeTrait};

pub(crate) fn fingerprint_tokens(tokens: &[Token]) -> u64 {
    let mut hasher = FxHasher::default();
    let mut block = [0u8; 8];
    let mut chunk_iter = tokens.chunks_exact(8);
    for token in chunk_iter.by_ref() {
        for (i, t) in token.iter().enumerate() {
            block[i] = t.token_type() as u8;
        }
        hasher.write(&block);
        // TODO: feature "whitespace" support (or remove the feature)
    }
    // Handle the remaining tokens if the length is not a multiple of 8
    for token in chunk_iter.remainder() {
        hasher.write_u8(token.token_type() as u8);
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
