use std::hash::{Hash, Hasher};

use fxhash::FxHasher;

use crate::{Token, tokenizer::TokenType};

/// This function generates a fingerprint for an iterator of token types.
pub fn fingerprint2(tokens: impl Iterator<Item = TokenType>) -> u64 {
    // Push each token type into the u64  fingerprint.
    let num_bits_per_type = Token::type_id_num_bits();
    let mut fingerprint = 0;
    let mut current_pos = 0;
    let max_tokens = 64 / num_bits_per_type as usize;
    for token_type in tokens.take(max_tokens) {
        if token_type.is_whitespace() {
            // Skip whitespace tokens, they don't contribute to the fingerprint.
            continue;
        }
        fingerprint |= (token_type.0 as u64) << current_pos;
        current_pos += num_bits_per_type;
    }
    fingerprint
}

pub fn fingerprint(tokens: impl Iterator<Item = TokenType>) -> u64 {
    let mut hasher = FxHasher::default();
    let mut num_tokens = 0;
    for token in tokens {
        if token.is_whitespace() {
            continue;
        }
        (token.0 as u64).hash(&mut hasher);
        num_tokens += 1;
    }
    // hash num tokens
    (num_tokens as u64).hash(&mut hasher);

    hasher.finish()
}
#[cfg(test)]
mod test {
    use super::Token;
    use super::fingerprint;

    #[test]
    fn multiple_tokens_pack_in_order() {
        let a = Token::Number("42"); // type_id() is 2
        let b = Token::Number("42"); // type_id() is 2
        let bits = Token::type_id_num_bits() as u64;
        let expected = 2 | (2 << bits);

        assert_eq!(
            fingerprint([a.token_type(), b.token_type()].into_iter()),
            expected
        );
    }

    #[test]
    fn single_token() {
        let a = Token::Number("42"); // type_id() is 2
        let expected = 2;

        assert_eq!(fingerprint([a.token_type()].into_iter()), expected);
    }
}
