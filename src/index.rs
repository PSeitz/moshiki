use std::io::{self};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use crate::columns::read::{Columns, decompress_column};
use crate::dict::Dict;
use crate::search::Searcher;
use crate::templates::{TemplateWithId, read_templates};
use crate::{Doc, TemplateId};

#[derive(Clone)]
/// The main entry point for the index
/// Just a wrapper around `IndexInner` with Arc
pub struct Index {
    inner: Arc<IndexInner>,
}
impl Index {
    /// Open an index from the specified folder.
    pub fn new(folder: &str) -> io::Result<Self> {
        let inner = IndexInner::new(folder)?;
        Ok(Index {
            inner: Arc::new(inner),
        })
    }

    /// Create a new searcher for this index.
    pub fn searcher(&self) -> Searcher {
        Searcher::new(self.clone())
    }
}
impl Deref for Index {
    type Target = IndexInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
/// The inner structure of the index, containing the dictionary and templates.
pub struct IndexInner {
    folder: PathBuf,
    pub(crate) dictionary: Arc<Dict>,
    pub(crate) templates: Templates,
}
pub(crate) struct Templates {
    templates: Vec<TemplateWithId>,
}
impl Templates {
    pub fn get_template(&self, template_id: TemplateId) -> &TemplateWithId {
        &self.templates[template_id.0 as usize]
    }

    pub fn iter(&self) -> impl Iterator<Item = &TemplateWithId> {
        self.templates.iter()
    }
}

impl IndexInner {
    pub(crate) fn new(folder: &str) -> io::Result<Self> {
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
        Ok(IndexInner {
            dictionary: Arc::new(dictionary),
            templates,
            folder,
        })
    }

    /// Retrieve documents based on the provided `Doc` (template ID and term IDs).
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
            if let Some(max) = max_hits
                && documents_ids_hit.len() >= max
            {
                break;
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
