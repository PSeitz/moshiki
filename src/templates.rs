use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::TemplateId;
use crate::constants::{TEMPLATE_DEBUG_FILE_NAME, TEMPLATE_FILE_NAME};
use crate::dict::Dict;
use crate::indexing::{self, IndexingTemplate, IndexingTemplateToken, PreliminaryIndex};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum MatchResult {
    Full,
    NoMatch,
    VariableMayMatch,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TemplateWithMeta {
    pub num_docs: usize,
    pub template_id: TemplateId,
    pub template: Template,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub(crate) parts: Vec<TemplateToken>,
}

impl TemplateWithMeta {
    pub(crate) fn num_docs(&self) -> usize {
        self.num_docs
    }
}

impl Template {
    /// Serialize this template to a readable String.
    pub fn ser_readable(&self) -> String {
        let mut out = String::new();
        for token in &self.parts {
            match token {
                TemplateToken::Constant(bytes) => {
                    out.push_str(std::str::from_utf8(bytes).unwrap());
                }
                TemplateToken::Variable => {
                    out.push('?');
                }
                TemplateToken::Whitespace(n) => {
                    for _ in 0..*n {
                        out.push(' ');
                    }
                }
            }
        }
        out
    }
    pub fn reconstruct(&self, term_ids: &[u32], dict: &Dict) -> io::Result<String> {
        let mut reconstructed = String::new();
        let mut term_id_idx = 0;
        for token in &self.parts {
            match token {
                TemplateToken::Constant(constant) => {
                    // TODO: FIX numbers(number_as_string feature flag)
                    reconstructed.push_str(std::str::from_utf8(constant).unwrap());
                }
                TemplateToken::Variable => {
                    let term = dict
                        .get_term_for_ord(term_ids[term_id_idx])?
                        .expect("Term ID out of bounds");
                    reconstructed.push_str(&term);
                    term_id_idx += 1;
                }
                TemplateToken::Whitespace(num) => {
                    reconstructed.extend(std::iter::repeat_n(' ', *num as usize));
                }
            }
        }
        Ok(reconstructed)
    }
    // If any of the tokens match, the whole template matches.
    pub fn check_match(&self, term: &str) -> MatchResult {
        let mut match_result = MatchResult::NoMatch;
        for token in &self.parts {
            let result = token.check_match(term);
            match result {
                MatchResult::Full => return MatchResult::Full,
                MatchResult::VariableMayMatch => match_result = MatchResult::VariableMayMatch,
                MatchResult::NoMatch => continue,
            }
        }
        match_result
    }
}
impl From<&IndexingTemplate> for TemplateWithMeta {
    fn from(template: &IndexingTemplate) -> Self {
        TemplateWithMeta {
            num_docs: template.num_docs,
            template_id: template.template_id,
            template: Template {
                parts: template
                    .tokens
                    .iter()
                    .map(|tok| TemplateToken::from(&tok.token))
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TemplateToken {
    Constant(Vec<u8>),
    Variable,
    Whitespace(u32),
}
impl TemplateToken {
    pub fn check_match(&self, term: &str) -> MatchResult {
        match self {
            TemplateToken::Constant(constant) => {
                if term.as_bytes() == constant {
                    MatchResult::Full
                } else {
                    MatchResult::NoMatch
                }
            }
            TemplateToken::Variable => MatchResult::VariableMayMatch,
            TemplateToken::Whitespace(_) => {
                if term.is_empty() {
                    MatchResult::Full
                } else {
                    MatchResult::NoMatch
                }
            }
        }
    }
}
impl From<&IndexingTemplateToken> for TemplateToken {
    fn from(token: &IndexingTemplateToken) -> Self {
        match token {
            indexing::IndexingTemplateToken::Constant(const_token) => {
                TemplateToken::Constant(const_token.text.to_vec())
            }
            indexing::IndexingTemplateToken::Variable { .. } => TemplateToken::Variable,
            #[cfg(feature = "whitespace")]
            indexing::IndexingTemplateToken::Whitespace(id) => TemplateToken::Whitespace(*id),
        }
    }
}

pub fn write_templates(index: &PreliminaryIndex, folder: &Path) -> io::Result<()> {
    let path = folder.join(TEMPLATE_FILE_NAME);
    let mut writer = BufWriter::new(File::create(path)?);
    let templates_only: Vec<TemplateWithMeta> =
        index.iter_templates().map(TemplateWithMeta::from).collect();
    let bytes: Vec<u8> = postcard::to_allocvec(&templates_only).map_err(io::Error::other)?;
    writer.write_all(&bytes)?;
    writer.flush()?;

    if std::env::var("DEBUG_TEMPLATES").is_ok() {
        let path = folder.join(TEMPLATE_DEBUG_FILE_NAME);
        let mut writer = BufWriter::new(File::create(path)?);
        for template in index.iter_templates() {
            let template = TemplateWithMeta::from(template);
            let template = template.template;
            writer.write_all(template.ser_readable().as_bytes())?;
            writer.write_all(b"\n")?;
        }
    }
    Ok(())
}
pub fn read_templates(folder: &Path) -> io::Result<Vec<TemplateWithMeta>> {
    let path = folder.join(TEMPLATE_FILE_NAME);
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    postcard::from_bytes(&buf).map_err(io::Error::other)
}
