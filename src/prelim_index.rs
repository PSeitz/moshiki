use fnv::{FnvHashMap, FnvHasher};
use stacker::ArenaHashMap;
use std::hash::Hasher;

use crate::tokenizer::{Token, TokenType, Tokenizer};

pub struct PreliminaryIndex {
    pub term_hash_map: ArenaHashMap,
    pub preliminary_docs: FnvHashMap<u64, Vec<PrelimDoc>>,
}

// A 32-bit composite: top 4 bits store token type, lower 28 bits store term ID
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompositeToken(u32);

impl CompositeToken {
    /// Pack a TokenType (4 bits) and a 28-bit ID into one u32
    #[inline]
    pub fn new(token_type: TokenType, term_id: u32) -> Self {
        // Ensure id fits in 28 bits
        assert!(term_id <= 0x0FFF_FFFF, "term ID out of range");
        let tt = (token_type.0 as u32) & 0xF;
        CompositeToken((tt << 28) | term_id)
    }

    /// Extract the TokenType from the top 4 bits
    #[inline]
    pub fn token_type(&self) -> TokenType {
        let token_type = ((self.0 >> 28) & 0xF) as u8;
        token_type.into()
    }

    /// Extract the 28-bit term ID
    pub fn term_id(&self) -> u32 {
        self.0 & 0x0FFF_FFFF
    }

    /// Get the raw u32 value
    #[inline]
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}
impl From<(TokenType, u32)> for CompositeToken {
    #[inline]
    fn from(value: (TokenType, u32)) -> Self {
        CompositeToken::new(value.0, value.1)
    }
}

pub fn preliminary_index(lines: impl Iterator<Item = String>) -> PreliminaryIndex {
    let mut term_hash_map = ArenaHashMap::with_capacity(4);
    let mut preliminary_docs: FnvHashMap<u64, Vec<PrelimDoc>> = FnvHashMap::default();

    for line in lines {
        let mut token_type_with_term_ids: Vec<CompositeToken> = Vec::with_capacity(32);
        let tokenizer = Tokenizer::new(&line);
        for token in tokenizer {
            let next_id = term_hash_map.len() as u32;
            match token {
                Token::IPv4(v)
                | Token::Number(v)
                | Token::Uuid(v)
                | Token::Word(v)
                | Token::Punctuation(v) => {
                    let mut term_id = 0;
                    term_hash_map.mutate_or_create(v.as_bytes(), |opt| {
                        term_id = opt.unwrap_or(next_id);
                        term_id
                    });
                    token_type_with_term_ids.push((token.token_type(), term_id).into());
                }
                Token::Whitespace(num) => {
                    token_type_with_term_ids.push((token.token_type(), num as u32).into());
                }
            }
        }

        let prelim_doc = PrelimDoc(token_type_with_term_ids);
        let mut hasher = FnvHasher::default();
        for token in prelim_doc.iter() {
            hasher.write_u8(token.token_type().0);
            // To distinguish documents with different whitespace, we include the
            // number of whitespace tokens
            if token.token_type().is_whitespace() {
                hasher.write_u32(token.term_id());
            }
        }
        let hash = hasher.finish();

        preliminary_docs.entry(hash).or_default().push(prelim_doc);
    }

    PreliminaryIndex {
        term_hash_map,
        preliminary_docs,
    }
}

#[derive(Debug, Clone)]
pub struct PrelimDoc(pub Vec<CompositeToken>);

impl PrelimDoc {
    pub fn iter(&self) -> impl Iterator<Item = &CompositeToken> {
        self.0.iter()
    }
    pub fn without_whitespace(&self) -> impl Iterator<Item = &CompositeToken> {
        self.0
            .iter()
            .filter(|token| !token.token_type().is_whitespace())
    }
}
