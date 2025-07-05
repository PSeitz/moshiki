use std::hash::Hasher;

use fxhash::FxHasher;

use crate::Token;

pub fn fingerprint(tokens: &[Token]) -> u64 {
    let mut hasher = FxHasher::default();
    for token in tokens {
        hasher.write_u8(token.token_type() as u8);
        if let Token::Whitespace(num) = token {
            hasher.write_u32(*num);
        }
    }

    hasher.finish()
}
