use crate::Token;

/// This function generates a fingerprint for a slice of tokens based on their types.
pub fn fingerprint(tokens: &[Token]) -> u64 {
    // Push each token type into the u32  fingerprint.
    let num_bits_per_type = Token::type_id_num_bits();
    let mut fingerprint = 0;
    let mut current_pos = 0;
    let max_tokens = 64 / num_bits_per_type as usize;
    for token in tokens.iter().take(max_tokens) {
        let token_type = token.type_id();
        fingerprint |= (token_type as u64) << current_pos;
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

        assert_eq!(fingerprint(&[a, b]), expected);
    }
}
