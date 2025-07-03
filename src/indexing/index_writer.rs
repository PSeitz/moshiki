use std::{
    io,
    path::{Path, PathBuf},
};

use super::{
    pattern_detection::pattern_detection,
    preliminary_index::preliminary_index,
    term_id_idx_to_template_ids,
    write_columns::{self, write_column},
    write_dict::{self, write_dictionary_and_generate_mapping},
};
use crate::templates::write_templates;

pub struct IndexWriter {
    output_folder: PathBuf,
}

impl IndexWriter {
    pub fn new(output_folder: String) -> Self {
        IndexWriter {
            output_folder: output_folder.into(),
        }
    }

    pub fn index(&self, lines: impl Iterator<Item = String>) -> io::Result<()> {
        let preliminary_index = preliminary_index(lines);
        preliminary_index.print_stats();
        let term_id_idx = term_id_idx_to_template_ids(&preliminary_index);
        let old_to_new_id_map = write_dictionary_and_generate_mapping(
            &self.output_folder,
            &preliminary_index.term_hash_map,
            term_id_idx,
        )
        .unwrap();

        let templates_and_docs = pattern_detection(&preliminary_index, &old_to_new_id_map);
        let templates_path = Path::new(&self.output_folder).join("templates.json");
        write_templates(&templates_and_docs, &templates_path).unwrap();

        for template_and_doc in templates_and_docs {
            write_column(&self.output_folder, &template_and_doc)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::indexing::{
        pattern_detection::pattern_detection, preliminary_index,
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
            tempfolder.path(),
            &preliminary_index.term_hash_map,
            vec![Default::default(); preliminary_index.term_hash_map.len()],
        )
        .unwrap();

        let templates_and_docs = pattern_detection(&preliminary_index, &old_to_new_id_map);
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
