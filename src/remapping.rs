use crate::prelim_index::{CompositeToken, PrelimDoc};

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
    use fst::{IntoStreamer, Map, Streamer};
    use std::fs::File;
    use std::io::Read;
    use tempfile::tempdir;

    #[test]
    fn test_remap() {
        let lines = vec![
            "hello world".to_string(),
            "hello there".to_string(),
            "goodbye world".to_string(),
        ];
        let prelim_index = preliminary_index(lines.into_iter());
        let dir = tempdir().unwrap();
        let index_writer = IndexWriter::new(dir.path().to_str().unwrap().to_string());
        let old_to_new_id_map = index_writer
            .write_dictionary_and_generate_mapping(&prelim_index.term_hash_map)
            .unwrap();

        let mut docs_vec: Vec<Vec<PrelimDoc>> = prelim_index
            .preliminary_docs
            .values()
            .map(|el| el.docs.clone())
            .collect();
        remap_term_ids(&mut docs_vec, &old_to_new_id_map);

        let mut remapped_tokens = Vec::new();
        for docs in docs_vec {
            for doc in docs {
                for token in doc.iter() {
                    if !token.token_type().is_whitespace() {
                        remapped_tokens.push(token.term_id());
                    }
                }
            }
        }
        remapped_tokens.sort();

        // The term IDs should be 0, 1, 2, 3, corresponding to the sorted terms
        // "goodbye", "hello", "there", "world"
        assert_eq!(remapped_tokens, vec![0, 1, 1, 2, 3, 3]);
    }

    #[test]
    fn test_write_dictionary() {
        let lines = vec![
            "hello world".to_string(),
            "hello there".to_string(),
            "goodbye world".to_string(),
        ];
        let prelim_index = preliminary_index(lines.into_iter());
        let dir = tempdir().unwrap();
        let index_writer = IndexWriter::new(dir.path().to_str().unwrap().to_string());
        let _old_to_new_id_map = index_writer
            .write_dictionary_and_generate_mapping(&prelim_index.term_hash_map)
            .unwrap();

        let dict_path = dir.path().join("dictionary.fst");
        let mut f = File::open(dict_path).unwrap();
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer).unwrap();
        let map = Map::new(buffer).unwrap();

        let mut stream = map.into_stream();
        let mut keys = Vec::new();
        while let Some((key, _)) = stream.next() {
            keys.push(String::from_utf8(key.to_vec()).unwrap());
        }
        keys.sort();
        assert_eq!(keys, vec!["goodbye", "hello", "there", "world"]);

        assert_eq!(map.get("goodbye"), Some(0));
        assert_eq!(map.get("hello"), Some(1));
        assert_eq!(map.get("there"), Some(2));
        assert_eq!(map.get("world"), Some(3));
    }
}
