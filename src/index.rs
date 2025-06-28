use std::{fs::File, io::Write, path::Path};

use crate::{patterns::pattern_scan, prelim_index::preliminary_index};

pub struct IndexWriter {
    output_folder: String,
}

impl IndexWriter {
    pub fn new(output_folder: String) -> Self {
        IndexWriter { output_folder }
    }

    pub fn index(&self, lines: impl Iterator<Item = String>) {
        let preliminary_index = preliminary_index(lines);
        let templates_and_docs = pattern_scan(&preliminary_index);

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
}
