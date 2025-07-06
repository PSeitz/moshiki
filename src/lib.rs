pub mod columns;
pub mod constants;
pub mod dict;
pub mod indexing;
pub mod search;
pub mod templates;
pub mod tokenizer;
pub use tokenizer::Token;

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

        let results = searcher.search("hello").unwrap();
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

        let results = searcher.search("hello").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "hello world");
        assert_eq!(results[1], "hello there");
    }
}
