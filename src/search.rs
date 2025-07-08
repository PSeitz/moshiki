use std::io::{self};
use std::path::PathBuf;

use fxhash::FxHashMap;

use crate::columns::{Columns, decompress_column};
use crate::dict::{Dict, SearchResult};
use crate::templates::{MatchResult, TemplateWithMeta, read_templates};
use crate::{Doc, TemplateId};

pub struct Searcher {
    dictionary: Dict,
    folder: PathBuf,
    templates: Templates,
}
pub struct Templates {
    templates: Vec<TemplateWithMeta>,
}
impl Templates {
    pub fn get_template(&self, template_id: TemplateId) -> &TemplateWithMeta {
        &self.templates[template_id.0 as usize]
    }

    fn iter(&self) -> impl Iterator<Item = &TemplateWithMeta> {
        self.templates.iter()
    }
}

impl Searcher {
    pub fn new(folder: &str) -> io::Result<Self> {
        let dictionary = Dict::new(folder)?;
        let folder = PathBuf::from(folder);
        let templates = read_templates(&folder)?;
        for (idx, template) in templates.iter().enumerate() {
            assert_eq!(
                idx, template.template_id.0 as usize,
                "Template ID mismatch at index {idx}",
            );
        }
        let templates = Templates { templates };
        Ok(Searcher {
            dictionary,
            templates,
            folder,
        })
    }

    pub fn retrieve_doc(&self, docs: &[Doc]) -> io::Result<Vec<String>> {
        // Retrieve the documents for the term ID and template IDs.
        let mut documents = Vec::new();
        for doc in docs {
            let reconstructed = self
                .templates
                .get_template(doc.template_id)
                .template
                .reconstruct(&doc.term_ids, &self.dictionary)?;
            documents.push(reconstructed);
        }

        Ok(documents)
    }

    /// Search for a term and retrieve the documents that match the term.
    pub fn search_and_retrieve(&self, query: &str) -> io::Result<Vec<String>> {
        let docs = self.search(query)?;
        self.retrieve_doc(&docs)
    }

    /// Search for a term and retrieve potential templates that match the term.
    fn get_potential_templates(&self, query: &str) -> FxHashMap<TemplateId, MatchResult> {
        // Get potential matches
        let matching_template_ids: FxHashMap<TemplateId, MatchResult> = self
            .templates
            .iter()
            .filter_map(|template| {
                let match_result = template.template.check_match(query);
                match match_result {
                    MatchResult::FullMatch | MatchResult::VariableMayMatch => {
                        Some((template.template_id, match_result))
                    }
                    MatchResult::NoMatch => None,
                }
            })
            .collect();
        matching_template_ids
    }

    /// Get documents from templates based on the matching template IDs and search result.
    fn get_doc_from_templates(
        &self,
        matching_template_ids: FxHashMap<TemplateId, MatchResult>,
        search_result: Option<&SearchResult>,
    ) -> io::Result<Vec<Doc>> {
        let mut matching_documents: Vec<Doc> = Vec::new();
        for (template_id, match_result) in matching_template_ids.into_iter() {
            let docs = match match_result {
                MatchResult::FullMatch => {
                    // Constant in template matches
                    self.search_in_zstd_column(|_| true, template_id, Some(10))?
                }
                MatchResult::VariableMayMatch => {
                    if let Some(search_result) = search_result
                        && search_result.template_ids().contains(&template_id)
                    {
                        // Check if the term ID exists in the zstd column.
                        let term_id = search_result.term_id();
                        self.search_in_zstd_column(|hit| term_id == hit, template_id, Some(10))?
                    } else {
                        // If the term ID is not found, we skip this template.
                        continue;
                    }
                }
                MatchResult::NoMatch => {
                    continue;
                }
            };
            matching_documents.extend(docs.into_iter().map(|term_ids| Doc {
                template_id,
                term_ids,
            }));
        }

        Ok(matching_documents)
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
    pub fn search(&self, query: &str) -> io::Result<Vec<Doc>> {
        let term = query.as_bytes();
        // The term may not exist in the dictionary, only in the templates.
        let search_result = self.dictionary.search_single_term(term)?;

        // Get potential matches
        let matching_template_ids: FxHashMap<TemplateId, MatchResult> =
            self.get_potential_templates(query);

        let matching_documents: Vec<Doc> =
            self.get_doc_from_templates(matching_template_ids, search_result.as_ref())?;
        Ok(matching_documents)
    }

    /// Returns the term ids of each document
    pub fn search_in_zstd_column(
        &self,
        match_fn: impl Fn(u32) -> bool,
        template_id: TemplateId,
        max_hits: Option<usize>,
    ) -> io::Result<Vec<Vec<u32>>> {
        // The number of variables with num_docs will used to retrieve the other terms
        // of a document.
        let num_docs = self.templates.get_template(template_id).num_docs();
        let columns: Columns = decompress_column(&self.folder, template_id, num_docs)?;

        let mut documents_ids_hit = Vec::new();
        for doc_id in columns.get_doc_ids(&match_fn) {
            documents_ids_hit.push(doc_id);
            if let Some(max) = max_hits {
                if documents_ids_hit.len() >= max {
                    break;
                }
            }
        }

        let mut all_documents = Vec::new();
        // Now we have the document IDs that contain the term ID.
        // We need to retrieve the other termids of the documents.
        for doc_id in documents_ids_hit.iter() {
            let document_terms = columns.get_term_ids(*doc_id).collect();
            all_documents.push(document_terms);
        }
        Ok(all_documents)
    }
}
