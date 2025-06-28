use stacker::ArenaHashMap;

use crate::prelim_index::{CompositeToken, PrelimDoc};

pub fn generate_term_id_mapping(term_hash_map: &ArenaHashMap) -> Vec<u32> {
    let mut sorted_terms: Vec<(&[u8], u32)> = Vec::with_capacity(term_hash_map.len());
    let max_old_id = term_hash_map.len() as u32;
    for (term_bytes, old_id_addr) in term_hash_map.iter() {
        let old_id: u32 = term_hash_map.read(old_id_addr);
        sorted_terms.push((term_bytes, old_id));
    }
    sorted_terms.sort_by(|(term_a, _), (term_b, _)| term_a.cmp(term_b));

    let mut old_to_new_id_map: Vec<u32> = vec![0; (max_old_id + 1) as usize];
    for (new_id, (_, old_id)) in sorted_terms.into_iter().enumerate() {
        old_to_new_id_map[old_id as usize] = new_id as u32;
    }
    old_to_new_id_map
}

pub fn remap_term_ids(preliminary_docs: &mut [Vec<PrelimDoc>], old_to_new_id_map: &[u32]) {
    for docs_vec in preliminary_docs.iter_mut() {
        for doc in docs_vec.iter_mut() {
            for composite_token in doc.0.iter_mut() {
                if !composite_token.token_type().is_whitespace() {
                    let old_term_id = composite_token.term_id();
                    let ordinal_term_id = old_to_new_id_map[old_term_id as usize];
                    *composite_token =
                        CompositeToken::new(composite_token.token_type(), ordinal_term_id);
                }
            }
        }
    }
}
