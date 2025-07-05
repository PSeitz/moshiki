use std::io;
use std::path::Path;

use fxhash::FxHashMap;
use tantivy_common::file_slice::FileSlice;

use crate::constants::DICTIONARY_NAME;
use crate::indexing::write_dict::VecU32ValueSSTable;
use crate::tokenizer::tokenize;

pub struct Dict {
    dictionary: tantivy_sstable::Dictionary<VecU32ValueSSTable>,
}

impl Dict {
    pub fn new(output_folder: &str) -> io::Result<Self> {
        let dictionary_path = Path::new(output_folder).join(DICTIONARY_NAME);
        let file = FileSlice::open(&dictionary_path)?;
        let dictionary = tantivy_sstable::Dictionary::<VecU32ValueSSTable>::open(file).unwrap();
        Ok(Dict { dictionary })
    }

    /// Search for terms in the dictionary and return a mapping of term IDs to template IDs.
    /// The query is tokenized, and each token is looked up in the dictionary.
    pub fn search(&self, query: &str) -> io::Result<FxHashMap<u32, Vec<u32>>> {
        let mut term_ids_to_template_ids: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for token in tokenize(query) {
            if let Some(term) = token.as_bytes(query) {
                if let Ok(Some((term_ord, template_ids))) = self.search_single_term(term) {
                    term_ids_to_template_ids
                        .entry(term_ord)
                        .or_default()
                        .extend(template_ids);
                }
            }
        }
        Ok(term_ids_to_template_ids)
    }
    /// Search for a singe term in the dictionary and return its term ID and associated template
    /// IDs.
    pub fn search_single_term(&self, term: &[u8]) -> io::Result<Option<(u32, Vec<u32>)>> {
        if let Ok(Some(term_ord)) = self.dictionary.term_ord(term) {
            return Ok(self
                .dictionary
                .term_info_from_ord(term_ord)?
                .map(|template_ids| Some((term_ord as u32, template_ids)))
                .expect("Term info should be present"));
        }
        Ok(None)
    }

    pub fn get_term_for_ord(&self, term_ord: u32) -> io::Result<Option<String>> {
        let mut out = Vec::new();
        if self.dictionary.ord_to_term(term_ord as u64, &mut out)? {
            return Ok(Some(String::from_utf8(out).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Failed to convert term bytes to String",
                )
            })?));
        }
        Ok(None)
    }
}
