use crate::{Token, tokenizer::TokenType};

/// This function generates a fingerprint for an iterator of token types.
pub fn fingerprint(tokens: impl Iterator<Item = TokenType>) -> u64 {
    // Push each token type into the u64  fingerprint.
    let num_bits_per_type = Token::type_id_num_bits();
    let mut fingerprint = 0;
    let mut current_pos = 0;
    let max_tokens = 64 / num_bits_per_type as usize;
    for token_type in tokens.take(max_tokens) {
        fingerprint |= (token_type.0 as u64) << current_pos;
        current_pos += num_bits_per_type;
    }
    fingerprint
}

#[cfg(test)]
mod test {
    use super::Token;
    use super::fingerprint;

    #[test]
    fn multiple_tokens_pack_in_order() {
        let a = Token::Number("42"); // type_id() is 1
        let b = Token::Number("42"); // type_id() is 1
        let bits = Token::type_id_num_bits() as u64;
        let expected = (1 << 0) | (1 << bits);

        assert_eq!(
            fingerprint([a.type_id(), b.type_id()].into_iter()),
            expected
        );
    }
}
