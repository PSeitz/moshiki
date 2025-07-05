use std::io::{self, Read};
use std::path::PathBuf;

use fxhash::FxHashMap;

use crate::dict::Dict;
use crate::indexing::write_columns::get_template_path;

pub struct Searcher {
    dictionary: Dict,
    output_folder: PathBuf,
}

impl Searcher {
    pub fn new(output_folder: &str) -> io::Result<Self> {
        let dictionary = Dict::new(output_folder)?;
        Ok(Searcher {
            dictionary,
            output_folder: PathBuf::from(output_folder),
        })
    }

    /// Search for terms in the dictionary and return a mapping of term IDs to template IDs.
    /// The query is tokenized, and each token is looked up in the dictionary.
    pub fn search(&self, query: &str) -> io::Result<FxHashMap<u32, Vec<u32>>> {
        self.dictionary.search(query)
    }

    /// Search for a singe term in the dictionary and return its term ID and associated template
    /// IDs.
    pub fn search_single_term(&self, term: &str) -> io::Result<Option<(u32, Vec<u32>)>> {
        self.dictionary.search_single_term(term)
    }

    pub fn search_in_zstd_column(&self, term_id: u32, template_id: u32) -> io::Result<bool> {
        let zstd_column_path = get_template_path(&self.output_folder, template_id);
        let file = std::fs::File::open(zstd_column_path)?;
        let mut decoder = zstd::Decoder::new(file)?;
        let mut buffer = [0u8; 4];
        while let Ok(()) = decoder.read_exact(&mut buffer) {
            if u32::from_le_bytes(buffer) == term_id {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
