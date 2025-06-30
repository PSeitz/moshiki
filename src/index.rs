use fst::MapBuilder;
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

use crate::{patterns::pattern_scan, prelim_index::preliminary_index, termmap::IndexingTermmap};

pub struct IndexWriter {
    output_folder: String,
}

impl IndexWriter {
    pub fn new(output_folder: String) -> Self {
        IndexWriter { output_folder }
    }

    pub fn index(&self, lines: impl Iterator<Item = String>) {
        let preliminary_index = preliminary_index(lines);
        let old_to_new_id_map = Self::write_dictionary_and_generate_mapping(
            &self.output_folder,
            &preliminary_index.term_hash_map,
        )
        .unwrap();

        let templates_and_docs = pattern_scan(&preliminary_index, &old_to_new_id_map);

        for template_and_doc in templates_and_docs {
            let mut byte_buffer = Vec::new();
            for term_id in &template_and_doc.docs_term_ids {
                byte_buffer.extend_from_slice(&term_id.to_le_bytes());
            }

            let compressed_data = zstd::stream::encode_all(&*byte_buffer, 13).unwrap();
            let file_path = Path::new(&self.output_folder).join(format!(
                "template_{}.zst",
                template_and_doc.template.template_id
            ));
            let mut file = File::create(file_path).unwrap();
            file.write_all(&compressed_data).unwrap();
        }
    }

    pub fn write_dictionary_and_generate_mapping(
        output_folder: &str,
        term_hash_map: &IndexingTermmap,
    ) -> io::Result<Vec<u32>> {
        let mut sorted_terms: Vec<(&[u8], u32)> = Vec::with_capacity(term_hash_map.len());
        let max_old_id = term_hash_map.len() as u32;
        for (term_bytes, old_id) in term_hash_map.iter() {
            sorted_terms.push((term_bytes, old_id));
        }

        sorted_terms.sort_by(|(term_a, _), (term_b, _)| term_a.cmp(term_b));

        let mut old_to_new_id_map: Vec<u32> = vec![0; (max_old_id + 1) as usize];
        let dictionary_path = Path::new(output_folder).join("dictionary.fst");
        let wtr = BufWriter::new(File::create(dictionary_path)?);
        let mut map_builder = MapBuilder::new(wtr).map_err(io::Error::other)?;

        // We may have duplicate terms, so we need to ensure that we assign the same new ID to the
        // same term and not insert it multiple times.
        let mut previous_term = None;
        let mut new_id = 0;
        for (term_bytes, old_id) in sorted_terms.into_iter() {
            if previous_term == Some(term_bytes) {
                // If the term is the same as the previous one, use the same new ID
                old_to_new_id_map[old_id as usize] = new_id as u32;
                continue;
            }
            previous_term = Some(term_bytes);
            old_to_new_id_map[old_id as usize] = new_id as u32;
            map_builder.insert(term_bytes, new_id as u64).unwrap();
            new_id += 1;
        }
        map_builder.finish().map_err(io::Error::other)?;
        Ok(old_to_new_id_map)
    }
}

#[cfg(test)]
mod test {
    use crate::{index::IndexWriter, patterns::pattern_scan, prelim_index::preliminary_index};

    #[test]
    fn test_mini_index() {
        let tempfolder = tempfile::tempdir().unwrap();
        //let writer = IndexWriter::new(tempfolder.path().to_str().unwrap().to_string());
        let lines = vec![r#"aaa ccc"#.to_string(), r#"aaa bbb"#.to_string()];
        let preliminary_index = preliminary_index(lines.into_iter());

        // Check that our docs are in the preliminary index
        let vals = preliminary_index.preliminary_docs.values().next().unwrap();
        assert_eq!(vals.columns.len(), 1, "Should have one column");
        assert_eq!(vals.num_docs, 2, "Should have two documents");
        assert_eq!(vals.columns[0].len(), 2, "Column should have two entries");

        let old_to_new_id_map = IndexWriter::write_dictionary_and_generate_mapping(
            tempfolder.path().to_str().unwrap(),
            &preliminary_index.term_hash_map,
        )
        .unwrap();

        let templates_and_docs = pattern_scan(&preliminary_index, &old_to_new_id_map);
        assert!(
            !templates_and_docs.is_empty(),
            "Templates and docs should not be empty"
        );
        assert_eq!(templates_and_docs.len(), 1, "Should have one template");
        // Reconstruct
        let template = &templates_and_docs[0];
        assert_eq!(
            template.docs_term_ids.len(),
            2,
            "Should have two docs term IDs"
        );
        assert_eq!(template.docs_term_ids, &[2, 1], "Term IDs should match");
    }
}
