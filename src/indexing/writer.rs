use std::{fs::File, io::Write, path::Path};

use super::{
    pattern_detection::pattern_scan, prelim::preliminary_index,
    write_dict::write_dictionary_and_generate_mapping,
};
use crate::templates::write_templates;

pub struct IndexWriter {
    output_folder: String,
}

impl IndexWriter {
    pub fn new(output_folder: String) -> Self {
        IndexWriter { output_folder }
    }

    pub fn index(&self, lines: impl Iterator<Item = String>) {
        let preliminary_index = preliminary_index(lines);
        let old_to_new_id_map = write_dictionary_and_generate_mapping(
            &self.output_folder,
            &preliminary_index.term_hash_map,
        )
        .unwrap();

        let templates_and_docs = pattern_scan(&preliminary_index, &old_to_new_id_map);
        let templates_path = Path::new(&self.output_folder).join("templates.json");
        write_templates(&templates_and_docs, &templates_path).unwrap();

        for template_and_doc in templates_and_docs {
            let mut byte_buffer = Vec::new();
            for term_id in &template_and_doc.docs_term_ids {
                byte_buffer.extend_from_slice(&term_id.to_le_bytes());
            }

            let compressed_data = zstd::stream::encode_all(&*byte_buffer, 6).unwrap();
            let file_path = Path::new(&self.output_folder).join(format!(
                "template_{}.zst",
                template_and_doc.template.template_id
            ));
            let mut file = File::create(file_path).unwrap();
            file.write_all(&compressed_data).unwrap();
        }
    }
}

#[cfg(test)]
mod test {
    use crate::indexing::{
        pattern_detection::pattern_scan, preliminary_index,
        write_dict::write_dictionary_and_generate_mapping,
    };

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

        let old_to_new_id_map = write_dictionary_and_generate_mapping(
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
