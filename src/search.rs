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
}
