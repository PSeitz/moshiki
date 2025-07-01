use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

use serde_json;

use crate::indexing::pattern_detection::{Template, TemplateAndDocs};

pub fn write_templates(templates: &[TemplateAndDocs], path: &Path) -> io::Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    let templates_only: Vec<_> = templates.iter().map(|t| &t.template).collect();
    serde_json::to_writer(writer, &templates_only).map_err(io::Error::other)
}

pub fn read_templates(path: &Path) -> io::Result<Vec<Template>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing::pattern_detection::{Template, TemplateAndDocs};
    use crate::indexing::{
        CompositeToken, ConstTemplateToken, TemplateToken, TemplateTokenWithMeta,
    };
    use crate::tokenizer::TokenType;
    use tempfile::TempDir;

    #[test]
    fn test_write_read_templates() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("templates.json");

        let templates = vec![TemplateAndDocs {
            template: Template {
                template_id: 0,
                parts: vec![TemplateTokenWithMeta {
                    token: TemplateToken::Constant(ConstTemplateToken {
                        composite_token: CompositeToken::new(TokenType::Word, 1),
                        text: "hello".to_string(),
                    }),
                    token_index: 0,
                }],
            },
            docs_term_ids: vec![1, 2, 3],
        }];

        write_templates(&templates, &path).unwrap();
        let read_templates_vec = read_templates(&path).unwrap();

        assert_eq!(templates.len(), read_templates_vec.len());
        assert_eq!(templates[0].template, read_templates_vec[0]);
    }
}
