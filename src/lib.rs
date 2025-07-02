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

    #[test]
    fn test_end_to_end() {
        let temp_dir = TempDir::new().unwrap();
        let output_folder = temp_dir.path().to_str().unwrap();

        // Index the data
        let writer = IndexWriter::new(output_folder.to_string());

        let lines = ["hello world", "hello there", "another line"];
        let lines = lines.iter().map(|line| line.to_string());
        writer.index(lines).unwrap();

        // Search the data
        let searcher = Searcher::new(output_folder).unwrap();

        let results = searcher.search("hello").unwrap();
        assert_eq!(results.len(), 1);

        // "world" is a variable and should be searchable
        let results = searcher.search("world").unwrap();
        assert_eq!(results.len(), 1);
        let template_ids = results.values().next().unwrap();
        assert_eq!(template_ids.len(), 1);

        // "there" is a variable and should be searchable
        let results = searcher.search("there").unwrap();
        assert_eq!(results.len(), 1);
        let template_ids = results.values().next().unwrap();
        assert_eq!(template_ids.len(), 1);

        let results = searcher.search("another").unwrap();
        assert_eq!(results.len(), 1);
        let results = searcher.search("line").unwrap();
        assert_eq!(results.len(), 1);

        let results = searcher.search("nonexistent").unwrap();
        assert_eq!(results.len(), 0);
    }
}
