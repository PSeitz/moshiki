use fnv::FnvHashMap;
use stacker::ArenaHashMap;

use crate::{fingerprint::fingerprint, tokenizer::tokenize};

pub struct PreliminaryIndex {
    pub term_hash_map: ArenaHashMap,
    pub preliminary_docs: Vec<PreliminaryDoc>,
}

pub fn preliminary_index(lines: impl Iterator<Item = String>) -> PreliminaryIndex {
    let mut term_hash_map = ArenaHashMap::with_capacity(4);
    let mut preliminary_docs: Vec<PreliminaryDoc> = Vec::new();
    for line in lines {
        let mut token_type_with_term_ids: Vec<(u8, u32)> = Vec::new();
        let tokens = tokenize(&line);
        let fingerprint = fingerprint(&tokens);
        for token in tokens {
            let next_id: u32 = term_hash_map.len() as u32;
            let mut term_id = 0;
            term_hash_map.mutate_or_create(token.as_str().as_bytes(), |value: Option<u32>| {
                term_id = if let Some(id) = value { id } else { next_id };
                term_id
            });
            //let term_id = term_hash_map
            //.entry(token.as_str().to_string())
            //.or_insert(next_id);
            token_type_with_term_ids.push((token.type_id(), term_id));
        }
        preliminary_docs.push(PreliminaryDoc::new(token_type_with_term_ids, fingerprint));
    }
    PreliminaryIndex {
        term_hash_map,
        preliminary_docs,
    }
}

pub struct PreliminaryDoc {
    pub token_type_with_term_ids: Vec<(u8, u32)>,
    pub fingerprint: u64,
}

impl PreliminaryDoc {
    fn new(token_type_with_term_ids: Vec<(u8, u32)>, fingerprint: u64) -> Self {
        PreliminaryDoc {
            token_type_with_term_ids,
            fingerprint,
        }
    }
}
