use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json;

use crate::indexing::pattern_detection::{IndexingTemplate, TemplateAndDocs};
use crate::indexing::{self, TemplateTokenWithMeta};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub template_id: u32,
    pub parts: Vec<TemplateToken>,
}
impl From<&IndexingTemplate> for Template {
    fn from(template: &IndexingTemplate) -> Self {
        Template {
            template_id: template.template_id,
            parts: template.parts.iter().map(TemplateToken::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TemplateToken {
    Constant(String),
    Variable,
    Whitespace(u32),
}
impl From<&TemplateTokenWithMeta> for TemplateToken {
    fn from(token_with_meta: &TemplateTokenWithMeta) -> Self {
        match token_with_meta.token {
            indexing::IndexingTemplateToken::Constant(ref const_token) => {
                TemplateToken::Constant(const_token.text.to_string())
            }
            indexing::IndexingTemplateToken::Variable { .. } => TemplateToken::Variable,
            indexing::IndexingTemplateToken::Whitespace(id) => TemplateToken::Whitespace(id),
        }
    }
}

pub fn write_templates(templates: &[TemplateAndDocs], path: &Path) -> io::Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    let templates_only: Vec<Template> = templates
        .iter()
        .map(|t| Template::from(&t.template))
        .collect();
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
    use crate::indexing::pattern_detection::{IndexingTemplate, TemplateAndDocs};
    use crate::indexing::{
        CompositeToken, ConstTemplateToken, IndexingTemplateToken, TemplateTokenWithMeta,
    };
    use crate::tokenizer::TokenType;
    use tempfile::TempDir;

    #[test]
    fn test_write_read_templates() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("templates.json");

        let templates = vec![TemplateAndDocs {
            template: IndexingTemplate {
                template_id: 0,
                parts: vec![TemplateTokenWithMeta {
                    token: IndexingTemplateToken::Constant(ConstTemplateToken {
                        composite_token: CompositeToken::new(TokenType::Word, 1),
                        text: "hello".to_string(),
                    }),
                    token_index: 0,
                }],
            },
            docs_term_ids: vec![1, 2, 3],
        }];

        write_templates(&templates, &path).unwrap();
        let read_templates_vec: Vec<Template> = read_templates(&path).unwrap();

        assert_eq!(templates.len(), read_templates_vec.len());
        assert_eq!(
            read_templates_vec[0].template_id,
            templates[0].template.template_id
        );
        assert_eq!(
            read_templates_vec[0].parts.len(),
            templates[0].template.parts.len()
        );
        assert_eq!(
            read_templates_vec[0].parts[0],
            TemplateToken::Constant("hello".to_string())
        );
    }
}
