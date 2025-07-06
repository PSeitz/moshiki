use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::TemplateId;
use crate::constants::TEMPLATE_FILE_NAME;
use crate::dict::Dict;
use crate::indexing::{self, IndexingTemplate, PreliminaryIndex, TemplateTokenWithMeta};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum MatchResult {
    FullMatch,
    NoMatch,
    VariableMayMatch,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub num_docs: usize,
    pub template_id: TemplateId,
    pub parts: Vec<TemplateToken>,
}
impl Template {
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
                MatchResult::FullMatch => return MatchResult::FullMatch,
                MatchResult::VariableMayMatch => match_result = MatchResult::VariableMayMatch,
                MatchResult::NoMatch => continue,
            }
        }
        match_result
    }

    pub(crate) fn num_docs(&self) -> usize {
        self.num_docs
    }
}
impl From<&IndexingTemplate> for Template {
    fn from(template: &IndexingTemplate) -> Self {
        Template {
            num_docs: template.num_docs,
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
impl TemplateToken {
    pub fn check_match(&self, term: &str) -> MatchResult {
        match self {
            TemplateToken::Constant(constant) => {
                if term.as_bytes() == constant {
                    MatchResult::FullMatch
                } else {
                    MatchResult::NoMatch
                }
            }
            TemplateToken::Variable => MatchResult::VariableMayMatch,
            TemplateToken::Whitespace(_) => {
                if term.is_empty() {
                    MatchResult::FullMatch
                } else {
                    MatchResult::NoMatch
                }
            }
        }
    }
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

pub fn write_templates(index: &PreliminaryIndex, folder: &Path) -> io::Result<()> {
    let path = folder.join(TEMPLATE_FILE_NAME);
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    let templates_only: Vec<Template> = index.iter_templates().map(Template::from).collect();
    let bytes: Vec<u8> = postcard::to_allocvec(&templates_only).map_err(io::Error::other)?;
    writer.write_all(&bytes)?;
    writer.flush()?;

    Ok(())
}
pub fn read_templates(folder: &Path) -> io::Result<Vec<Template>> {
    let path = folder.join(TEMPLATE_FILE_NAME);
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    postcard::from_bytes(&buf).map_err(io::Error::other)
}
