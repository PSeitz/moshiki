use stacker::ArenaHashMap;

use crate::{
    fingerprint::fingerprint,
    tokenizer::{TokenType, Tokenizer},
};

pub struct PreliminaryIndex {
    pub term_hash_map: ArenaHashMap,
    pub preliminary_docs: Vec<PreliminaryDoc>,
}

pub fn preliminary_index(lines: impl Iterator<Item = String>) -> PreliminaryIndex {
    let mut term_hash_map = ArenaHashMap::with_capacity(4);
    let mut preliminary_docs = Vec::new();

    for line in lines {
        // Tokenize and process
        let mut token_type_with_term_ids = Vec::with_capacity(48);
        let tokenizer = Tokenizer::new(&line);
        for token in tokenizer {
            let next_id = term_hash_map.len() as u32;
            let mut term_id = 0;
            term_hash_map.mutate_or_create(token.as_str().as_bytes(), |opt| {
                term_id = opt.unwrap_or(next_id);
                term_id
            });
            token_type_with_term_ids.push((token.type_id(), term_id));
        }
        let fingerprint = fingerprint(
            token_type_with_term_ids
                .iter()
                .map(|(token_type, _)| *token_type),
        );

        preliminary_docs.push(PreliminaryDoc::new(token_type_with_term_ids, fingerprint));
    }

    PreliminaryIndex {
        term_hash_map,
        preliminary_docs,
    }
}

pub struct PreliminaryDoc {
    pub token_type_with_term_ids: Vec<(TokenType, u32)>,
    pub fingerprint: u64,
}

impl PreliminaryDoc {
    fn new(token_type_with_term_ids: Vec<(TokenType, u32)>, fingerprint: u64) -> Self {
        PreliminaryDoc {
            token_type_with_term_ids,
            fingerprint,
        }
    }
}
