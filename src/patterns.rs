use stacker::ArenaHashMap;

use crate::{
    fingerprint::fingerprint,
    tokenizer::{TokenType, Tokenizer},
};

pub struct PreliminaryIndex {
    pub term_hash_map: ArenaHashMap,
    pub preliminary_docs: Vec<PreliminaryDoc>,
}

// A 32-bit composite: top 4 bits store token type, lower 28 bits store term ID
#[repr(transparent)]
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
    pub fn token_type(&self) -> TokenType {
        let token_type = ((self.0 >> 28) & 0xF) as u8;
        token_type.into()
    }

    /// Extract the 28-bit term ID
    pub fn term_id(&self) -> u32 {
        self.0 & 0x0FFF_FFFF
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
    let mut preliminary_docs = Vec::new();

    for line in lines {
        // Tokenize and process
        let mut token_type_with_term_ids: Vec<CompositeToken> = Vec::with_capacity(32);
        let tokenizer = Tokenizer::new(&line);
        for token in tokenizer {
            let next_id = term_hash_map.len() as u32;
            let mut term_id = 0;
            if let Some(token_str) = token.as_str() {
                term_hash_map.mutate_or_create(token_str.as_bytes(), |opt| {
                    term_id = opt.unwrap_or(next_id);
                    term_id
                });
            }
            token_type_with_term_ids.push((token.type_id(), term_id).into());
        }
        let fingerprint = fingerprint(
            token_type_with_term_ids
                .iter()
                .map(|comp_token| comp_token.token_type()),
        );

        preliminary_docs.push(PreliminaryDoc::new(token_type_with_term_ids, fingerprint));
    }

    PreliminaryIndex {
        term_hash_map,
        preliminary_docs,
    }
}

pub struct PreliminaryDoc {
    pub token_type_with_term_ids: Vec<CompositeToken>,
    pub fingerprint: u64,
}

impl PreliminaryDoc {
    fn new(token_type_with_term_ids: Vec<CompositeToken>, fingerprint: u64) -> Self {
        PreliminaryDoc {
            token_type_with_term_ids,
            fingerprint,
        }
    }
}
