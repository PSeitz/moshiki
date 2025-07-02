use std::io;
use std::path::Path;

use fxhash::FxHashMap;
use tantivy_common::file_slice::FileSlice;

use crate::indexing::write_dict::VecU32ValueSSTable;
use crate::tokenizer::tokenize;

pub struct Searcher {
    dictionary: tantivy_sstable::Dictionary<VecU32ValueSSTable>,
}

impl Searcher {
    pub fn new(output_folder: &str) -> io::Result<Self> {
        let dictionary_path = Path::new(output_folder).join("dictionary.fst");
        let file = FileSlice::open(&dictionary_path)?;
        let dictionary = tantivy_sstable::Dictionary::<VecU32ValueSSTable>::open(file).unwrap();
        Ok(Searcher { dictionary })
    }

    /// Search for terms in the dictionary and return a mapping of term IDs to template IDs.
    /// The query is tokenized, and each token is looked up in the dictionary.
    pub fn search(&self, query: &str) -> io::Result<FxHashMap<u32, Vec<u32>>> {
        let mut term_ids_to_template_ids: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for token in tokenize(query) {
            if let Some(term) = token.as_str(query) {
                if let Ok(Some(term_ord)) = self.dictionary.term_ord(term) {
                    let template_ids = self.dictionary.term_info_from_ord(term_ord)?.unwrap();
                    term_ids_to_template_ids
                        .entry(term_ord as u32)
                        .or_default()
                        .extend(template_ids);
                }
            }
        }
        Ok(term_ids_to_template_ids)
    }
    /// Search for a singe term in the dictionary and return its term ID and associated template
    /// IDs.
    pub fn search_single_term(&self, term: &str) -> io::Result<Option<(u32, Vec<u32>)>> {
        if let Ok(Some(term_ord)) = self.dictionary.term_ord(term) {
            return Ok(self
                .dictionary
                .term_info_from_ord(term_ord)?
                .map(|template_ids| Some((term_ord as u32, template_ids)))
                .unwrap());
        }
        Ok(None)
    }

    pub fn search_in_zstd_column(&self, term_id: u32, zstd_column_path: &Path) -> io::Result<bool> {
        let file = std::fs::File::open(zstd_column_path)?;
        let mut decoder = zstd::Decoder::new(file)?;
        let mut buffer = Vec::new();
        io::Read::read_to_end(&mut decoder, &mut buffer)?;
        Ok(buffer
            .chunks_exact(4)
            .any(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()) == term_id))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_search_zstd_compressed() {
        let temp_dir = TempDir::new().unwrap();
        let output_folder = temp_dir.path().to_str().unwrap();

        // Create a dummy dictionary
        let dictionary_path = temp_dir.path().join("dictionary.fst");
        let mut dictionary_builder = tantivy_sstable::Dictionary::<VecU32ValueSSTable>::builder(
            std::fs::File::create(&dictionary_path).unwrap(),
        )
        .unwrap();
        dictionary_builder.insert("a", &vec![12]).unwrap();
        dictionary_builder.finish().unwrap();

        // Create a dummy compressed file
        let compressed_path = temp_dir.path().join("test.zstd");
        let compressed_file = std::fs::File::create(&compressed_path).unwrap();
        let mut encoder = zstd::Encoder::new(compressed_file, 0).unwrap();
        encoder
            .write_all(
                &[1u64, 2, 3, 12, 15]
                    .iter()
                    .flat_map(|el| el.to_le_bytes())
                    .collect::<Vec<u8>>(),
            )
            .unwrap();
        encoder.finish().unwrap();

        let searcher = Searcher::new(output_folder).unwrap();
        let term_ids = searcher.search_single_term("a").unwrap().unwrap();
        assert_eq!(term_ids, (0, vec![12]));
        assert!(
            searcher
                .search_in_zstd_column(12, &compressed_path)
                .unwrap()
        );
        assert!(!searcher.search_in_zstd_column(4, &compressed_path).unwrap());
    }
}
