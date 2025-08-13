#![deny(missing_docs)]
//! Moshiki is CLP like search engine for unstructured logs.
//! It provides functionality for indexing, searching, and managing log data.

/// For handling columns of data
pub mod columns;
pub mod constants;
/// For dictionary-related operations
pub(crate) mod dict;

/// For indexing data
pub mod indexing;

/// The main entry point for the index and searcher
pub mod index;
/// For searching the index
pub mod search;
/// For handling templates
pub(crate) mod templates;
/// Tokenizer and token types
pub mod tokenizer;

use serde::{Deserialize, Serialize};
pub(crate) use tokenizer::Token;

/// A unique identifier for a template.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TemplateId(pub u32);
impl From<u32> for TemplateId {
    fn from(id: u32) -> Self {
        TemplateId(id)
    }
}

/// A document in the index, containing a template ID and a list of term IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    /// The ID of the template that this document matches.
    pub template_id: TemplateId,
    /// The list of term IDs that are present in this document.
    pub term_ids: Vec<u32>,
}

#[cfg(test)]
mod tests {

    use tempfile::TempDir;

    use crate::index::Index;
    use crate::indexing::IndexWriter;

    pub fn index<T: Into<String>>(output_folder: &str, lines: impl Iterator<Item = T>) {
        let writer = IndexWriter::new(output_folder.to_string());
        writer.index(lines, false).unwrap();
    }
    #[test]
    fn integration_test_variable_search() {
        let temp_dir = TempDir::new().unwrap();
        let output_folder = temp_dir.path().to_str().unwrap();
        index(
            output_folder,
            ["hello world", "hello there", "nice line"].into_iter(),
        );

        let searcher = Index::new(output_folder).unwrap().searcher();

        let results = searcher.search_and_retrieve("hello").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "hello world");
        assert_eq!(results[1], "hello there");
    }

    #[test]
    fn integration_test_constant_search() {
        let temp_dir = TempDir::new().unwrap();
        let output_folder = temp_dir.path().to_str().unwrap();

        index(
            output_folder,
            ["hello world", "hello there", "cool nice line"].into_iter(),
        );

        let searcher = Index::new(output_folder).unwrap().searcher();

        let results = searcher.search_and_retrieve("hello").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "hello world");
        assert_eq!(results[1], "hello there");
    }
}
