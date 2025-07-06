pub mod columns;
pub mod constants;
pub mod dict;
pub mod indexing;
pub mod search;
pub mod templates;
pub mod tokenizer;
use serde::{Deserialize, Serialize};
pub use tokenizer::Token;

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
    pub template_id: TemplateId,
    pub term_ids: Vec<u32>,
}

#[cfg(test)]
mod tests {

    use tempfile::TempDir;

    use crate::indexing::IndexWriter;
    use crate::search::Searcher;

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

        let searcher = Searcher::new(output_folder).unwrap();

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

        let searcher = Searcher::new(output_folder).unwrap();

        let results = searcher.search_and_retrieve("hello").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "hello world");
        assert_eq!(results[1], "hello there");
    }
}
