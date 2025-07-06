use std::io::{self};
use std::path::PathBuf;

use fxhash::FxHashMap;

use crate::columns::{Columns, decompress_column};
use crate::dict::Dict;
use crate::templates::{MatchResult, Template, read_templates};

pub struct Searcher {
    dictionary: Dict,
    folder: PathBuf,
    templates: Vec<Template>,
}

impl Searcher {
    pub fn new(folder: &str) -> io::Result<Self> {
        let dictionary = Dict::new(folder)?;
        let folder = PathBuf::from(folder);
        let templates = read_templates(&folder)?;
        for (idx, template) in templates.iter().enumerate() {
            assert_eq!(
                idx, template.template_id as usize,
                "Template ID mismatch at index {idx}",
            );
        }
        Ok(Searcher {
            dictionary,
            templates,
            folder,
        })
    }

    /// TODO: Only single term search is implemented.
    ///
    /// 1. Search for a term in the dictionary - this will return the term ID and associated
    ///    template IDs.
    /// 2. Check which of the templates match the term.
    /// 3. Scan the zstd column files for the term ID to see if it exists in the template.
    /// 4. If the term ID exists in the template, return all term IDs of the document.
    /// 5. Use the term IDs with the template to reconstruct the documents.
    ///
    pub fn search(&self, query: &str) -> io::Result<Vec<String>> {
        let term = query.as_bytes();
        let search_result = self.dictionary.search_single_term(term)?;

        let matching_template_ids: FxHashMap<u32, MatchResult> = self
            .templates
            .iter()
            .filter_map(|template| {
                let match_result = template.check_match(query);
                match match_result {
                    MatchResult::FullMatch | MatchResult::VariableMayMatch => {
                        Some((template.template_id, match_result))
                    }
                    MatchResult::NoMatch => None,
                }
            })
            .collect();

        let mut matching_documents = Vec::new();

        for template in self.templates.iter() {
            let template_id = template.template_id;
            // If the template matches, we can check if the term ID exists in the zstd column.
            if let Some(match_result) = matching_template_ids.get(&template_id) {
                match match_result {
                    MatchResult::FullMatch => {
                        // If the template fully matches, we don't need to check the zstd
                        // column.
                        let docs = self.search_in_zstd_column(|_| true, template_id, Some(10))?;
                        matching_documents.push((template_id, docs));
                        continue;
                    }
                    MatchResult::VariableMayMatch => {
                        if let Some(ref search_result) = search_result
                            && search_result.template_ids().contains(&template_id)
                        {
                            // Check if the term ID exists in the zstd column.
                            let term_id = search_result.term_id();
                            let docs = self.search_in_zstd_column(
                                |hit| term_id == hit,
                                template_id,
                                Some(10),
                            )?;
                            matching_documents.push((template_id, docs));
                        }
                    }
                    MatchResult::NoMatch => continue, // Skip this template
                }
            }
        }
        // Retrieve the documents for the term ID and template IDs.
        let mut documents = Vec::new();
        for (template_id, doc_ids) in matching_documents {
            for doc_terms in doc_ids {
                let reconstructed = self.templates[template_id as usize]
                    .reconstruct(&doc_terms, &self.dictionary)?;
                documents.push(reconstructed);
            }
        }

        Ok(documents)
    }

    pub fn search_in_zstd_column(
        &self,
        match_fn: impl Fn(u32) -> bool,
        template_id: u32,
        max_hits: Option<usize>,
    ) -> io::Result<Vec<Vec<u32>>> {
        // The number of variables with num_docs will used to retrieve the other terms
        // of a document.
        let num_docs = self.templates[template_id as usize].num_docs();
        let columns: Columns = decompress_column(&self.folder, template_id, num_docs)?;

        let mut documents_ids_hit = Vec::new();
        for column in columns.iter_columns() {
            for (doc_id, term_id) in column.iter().enumerate() {
                if match_fn(term_id) {
                    documents_ids_hit.push(doc_id as u32);
                    if let Some(max) = max_hits {
                        if documents_ids_hit.len() >= max {
                            break;
                        }
                    }
                }
            }
        }
        if documents_ids_hit.is_empty() {
            return Ok(Vec::new());
        }
        let mut all_documents = Vec::new();
        // Now we have the document IDs that contain the term ID.
        // We need to retrieve the other termids of the documents.
        for doc_id in documents_ids_hit.iter() {
            let document_terms = columns
                .iter_columns()
                .map(|col| col.term_at(*doc_id as usize).expect("Term ID not found"))
                .collect::<Vec<u32>>();
            all_documents.push(document_terms);
        }
        Ok(all_documents)
    }
}
