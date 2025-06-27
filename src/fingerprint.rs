use std::hash::{Hash, Hasher};

use fxhash::FxHasher;

use crate::{Token, prelim_index::PrelimDoc};

/// This function generates a fingerprint for an iterator of token types.
pub fn fingerprint2(prelim_doc: &PrelimDoc) -> u64 {
    // Push each token type into the u64  fingerprint.
    let num_bits_per_type = Token::type_id_num_bits();
    let mut fingerprint = 0;
    let mut current_pos = 0;
    let max_tokens = 64 / num_bits_per_type as usize;
    for token_type in prelim_doc
        .without_whitespace()
        .map(|token| token.token_type())
        .take(max_tokens)
    {
        fingerprint |= (token_type.0 as u64) << current_pos;
        current_pos += num_bits_per_type;
    }
    fingerprint
}

pub fn fingerprint(prelim_doc: &PrelimDoc) -> u64 {
    let mut hasher = FxHasher::default();
    let mut num_tokens = 0;
    for token in prelim_doc.without_whitespace() {
        (token.token_type().0 as u64).hash(&mut hasher);
        num_tokens += 1;
    }
    // hash num tokens
    (num_tokens as u64).hash(&mut hasher);

    hasher.finish()
}
#[cfg(test)]
mod test {
    use super::{fingerprint, fingerprint2};
    use crate::{
        Token,
        prelim_index::{CompositeToken, PrelimDoc},
    };

    fn create_prelim_doc(tokens: Vec<Token>) -> PrelimDoc {
        let mut composite_tokens = Vec::new();
        for (i, token) in tokens.iter().enumerate() {
            composite_tokens.push(CompositeToken::new(token.token_type(), i as u32));
        }
        PrelimDoc(composite_tokens)
    }

    #[test]
    fn multiple_tokens_pack_in_order() {
        let a = Token::Number("42"); // type_id() is 2
        let b = Token::Number("42"); // type_id() is 2
        let prelim_doc = create_prelim_doc(vec![a, b]);
        let bits = Token::type_id_num_bits() as u64;
        let expected = 2 | (2 << bits);

        assert_eq!(fingerprint2(&prelim_doc), expected);
    }

    #[test]
    fn single_token() {
        let a = Token::Number("42"); // type_id() is 2
        let prelim_doc = create_prelim_doc(vec![a]);
        let expected = 2;

        assert_eq!(fingerprint2(&prelim_doc), expected);
    }
}
