use std::io;
use std::path::Path;

use tantivy_common::file_slice::FileSlice;
use tantivy_sstable::MonotonicU64SSTable;

use crate::tokenizer::tokenize;

pub struct Searcher {
    dictionary: tantivy_sstable::Dictionary<MonotonicU64SSTable>,
}

impl Searcher {
    pub fn new(output_folder: &str) -> io::Result<Self> {
        let dictionary_path = Path::new(output_folder).join("dictionary.fst");
        let file = FileSlice::open(&dictionary_path)?;
        let dictionary = tantivy_sstable::Dictionary::<MonotonicU64SSTable>::open(file).unwrap();
        Ok(Searcher { dictionary })
    }

    pub fn search(&self, query: &str) -> Vec<u64> {
        let mut term_ids = Vec::new();
        for token in tokenize(query) {
            if let Some(term) = token.as_str(query) {
                if let Ok(Some(term_id)) = self.dictionary.get(term) {
                    term_ids.push(term_id);
                }
            }
        }
        term_ids
    }

    pub fn search_in_zstd_column(&self, term_id: u64, zstd_column_path: &Path) -> io::Result<bool> {
        let file = std::fs::File::open(zstd_column_path)?;
        let mut decoder = zstd::Decoder::new(file)?;
        let mut buffer = Vec::new();
        io::Read::read_to_end(&mut decoder, &mut buffer)?;
        Ok(buffer
            .chunks_exact(8)
            .any(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()) == term_id))
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
        let mut dictionary_builder = tantivy_sstable::Dictionary::<MonotonicU64SSTable>::builder(
            std::fs::File::create(&dictionary_path).unwrap(),
        )
        .unwrap();
        dictionary_builder.insert("a", &12).unwrap();
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
        let term_ids = searcher.search("a");
        assert_eq!(term_ids, vec![12]);
        assert!(
            searcher
                .search_in_zstd_column(12, &compressed_path)
                .unwrap()
        );
        assert!(!searcher.search_in_zstd_column(4, &compressed_path).unwrap());
    }
}
