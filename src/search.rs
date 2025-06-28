use std::fs::File;
use std::io;
use std::path::Path;

use fst::Map;

use crate::tokenizer::tokenize;

pub struct Searcher {
    dictionary: Map<memmap2::Mmap>,
}

impl Searcher {
    pub fn new(output_folder: &str) -> io::Result<Self> {
        let dictionary_path = Path::new(output_folder).join("dictionary.fst");
        let dictionary = Map::new(unsafe { memmap2::Mmap::map(&File::open(dictionary_path)?)? })
            .map_err(io::Error::other)?;
        Ok(Searcher { dictionary })
    }

    pub fn search(&self, query: &str) -> Vec<u64> {
        let mut term_ids = Vec::new();
        for token in tokenize(query) {
            if let Some(term) = token.as_str(query) {
                if let Some(term_id) = self.dictionary.get(term) {
                    term_ids.push(term_id);
                }
            }
        }
        term_ids
    }
}