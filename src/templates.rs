use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::indexing::{self, IndexingTemplate, PreliminaryIndex, TemplateTokenWithMeta};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub template_id: u32,
    pub parts: Vec<TemplateToken>,
}
impl From<&IndexingTemplate> for Template {
    fn from(template: &IndexingTemplate) -> Self {
        Template {
            template_id: template.template_id,
            parts: template.tokens.iter().map(TemplateToken::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TemplateToken {
    Constant(Vec<u8>),
    Variable,
    Whitespace(u32),
}
impl From<&TemplateTokenWithMeta> for TemplateToken {
    fn from(token_with_meta: &TemplateTokenWithMeta) -> Self {
        match token_with_meta.token {
            indexing::IndexingTemplateToken::Constant(ref const_token) => {
                TemplateToken::Constant(const_token.text.to_vec())
            }
            indexing::IndexingTemplateToken::Variable { .. } => TemplateToken::Variable,
            indexing::IndexingTemplateToken::Whitespace(id) => TemplateToken::Whitespace(id),
        }
    }
}

pub fn write_templates(index: &PreliminaryIndex, path: &Path) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    let templates_only: Vec<Template> = index.iter_templates().map(Template::from).collect();
    let bytes: Vec<u8> = postcard::to_allocvec(&templates_only).map_err(io::Error::other)?;
    writer.write_all(&bytes)?;
    writer.flush()?;

    Ok(())
}
pub fn read_templates(path: &Path) -> io::Result<Vec<Template>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;

    postcard::from_bytes(&buf).map_err(io::Error::other)
}
