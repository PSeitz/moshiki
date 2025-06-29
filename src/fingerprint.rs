use std::hash::{Hash, Hasher};

use fxhash::FxHasher;

use crate::Token;

pub fn fingerprint(tokens: &[Token]) -> u64 {
    let mut hasher = FxHasher::default();
    for token in tokens {
        (token.token_type() as u64).hash(&mut hasher);
        if let Token::Whitespace(num) = token {
            num.hash(&mut hasher);
        }
    }

    hasher.finish()
}
