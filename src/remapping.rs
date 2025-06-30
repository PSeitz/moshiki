use crate::prelim_index::{CompositeToken, PrelimDocGroup};
use fnv::FnvHashMap;

pub fn remap_term_ids(
    preliminary_docs: &mut FnvHashMap<u64, PrelimDocGroup>,
    old_to_new_id_map: &[u32],
) {
    for group in preliminary_docs.values_mut() {
        for column in group.columns.iter_mut() {
            for old_term_id in column.iter_mut() {
                let ordinal_term_id = old_to_new_id_map[*old_term_id as usize];
                *old_term_id = ordinal_term_id;
            }
        }
    }
}

pub fn remap_term_ids_in_template(template: &mut [CompositeToken], old_to_new_id_map: &[u32]) {
    for composite_token in template.iter_mut() {
        if !composite_token.token_type().is_whitespace() {
            let old_term_id = composite_token.term_id();
            let ordinal_term_id = old_to_new_id_map[old_term_id as usize];
            *composite_token = CompositeToken::new(composite_token.token_type(), ordinal_term_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::IndexWriter;
    use crate::prelim_index::preliminary_index;
    use tempfile::tempdir;

    #[test]
    fn test_remap() {
        let lines = vec![
            "hello world".to_string(),
            "hello there".to_string(),
            "goodbye world".to_string(),
        ];
        let mut prelim_index = preliminary_index(lines.into_iter());
        let dir = tempdir().unwrap();
        let old_to_new_id_map = IndexWriter::write_dictionary_and_generate_mapping(
            dir.path().to_str().unwrap(),
            &prelim_index.term_hash_map,
        )
        .unwrap();

        remap_term_ids(&mut prelim_index.preliminary_docs, &old_to_new_id_map);

        //let mut remapped_tokens = Vec::new();
        //for group in prelim_index.preliminary_docs.values() {
        //for doc in group.iter_docs() {
        //for token in doc.without_whitespace() {
        //remapped_tokens.push(token.term_id());
        //}
        //}
        //}
        //remapped_tokens.sort();

        //// The term IDs should be 0, 1, 2, 3, corresponding to the sorted terms
        //// "goodbye", "hello", "there", "world"
        //assert_eq!(remapped_tokens, vec![0, 1, 1, 2, 3, 3]);
    }
}
