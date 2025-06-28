use fst::MapBuilder;
use stacker::ArenaHashMap;
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

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
        let _old_to_new_id_map = self
            .write_dictionary_and_generate_mapping(&preliminary_index.term_hash_map)
            .unwrap();

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

    pub fn write_dictionary_and_generate_mapping(
        &self,
        term_hash_map: &ArenaHashMap,
    ) -> io::Result<Vec<u32>> {
        let mut sorted_terms: Vec<(&[u8], u32)> = Vec::with_capacity(term_hash_map.len());
        let max_old_id = term_hash_map.len() as u32;
        for (term_bytes, old_id_addr) in term_hash_map.iter() {
            let old_id: u32 = term_hash_map.read(old_id_addr);
            sorted_terms.push((term_bytes, old_id));
        }
        sorted_terms.sort_by(|(term_a, _), (term_b, _)| term_a.cmp(term_b));

        let mut old_to_new_id_map: Vec<u32> = vec![0; (max_old_id + 1) as usize];
        let dictionary_path = Path::new(&self.output_folder).join("dictionary.fst");
        let wtr = BufWriter::new(File::create(dictionary_path)?);
        let mut map_builder = MapBuilder::new(wtr).map_err(io::Error::other)?;

        for (new_id, (term_bytes, old_id)) in sorted_terms.into_iter().enumerate() {
            old_to_new_id_map[old_id as usize] = new_id as u32;
            map_builder.insert(term_bytes, new_id as u64).unwrap();
        }
        map_builder.finish().map_err(io::Error::other)?;
        Ok(old_to_new_id_map)
    }
}
