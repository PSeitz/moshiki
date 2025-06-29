use crate::prelim_index::{PreliminaryIndex, TemplateToken};

#[derive(Debug)]
pub struct TemplateAndDocs {
    pub template: Template,
    pub docs_term_ids: Vec<u32>,
}

#[derive(Debug)]
pub struct Template {
    pub template_id: u32,
    pub parts: Vec<TemplatePart>,
}

#[derive(Debug)]
pub enum TemplatePart {
    Constant(String),
    Placeholder,
}

pub fn pattern_scan(index: &PreliminaryIndex, old_to_new_id_map: &[u32]) -> Vec<TemplateAndDocs> {
    let mut term_id_to_term_map: Vec<&[u8]> = vec![&[]; index.term_hash_map.len()];
    for (term_bytes, old_id) in index.term_hash_map.iter() {
        term_id_to_term_map[old_id as usize] = term_bytes;
    }

    let mut template_and_docs = Vec::new();
    let mut template_id_counter = 0;

    for group in index.preliminary_docs.values() {
        let template_parts: Vec<TemplatePart> = group
            .template
            .tokens
            .iter()
            .filter_map(|tt| match tt {
                TemplateToken::Constant(ct) => {
                    let term = String::from_utf8_lossy(term_id_to_term_map[ct.term_id() as usize])
                        .to_string();
                    Some(TemplatePart::Constant(term))
                }
                TemplateToken::Variable { .. } => Some(TemplatePart::Placeholder),
                TemplateToken::Whitespace(_) => None,
            })
            .collect();

        let mut docs_ids = Vec::new();
        for (column_pos, column) in group.columns.iter().enumerate() {
            // Skip constant columns or whitespace columns
            if !group.template.tokens[column_pos].is_variable() {
                continue;
            }

            for term_id in column {
                docs_ids.push(old_to_new_id_map[term_id.term_id() as usize]);
            }
        }

        template_and_docs.push(TemplateAndDocs {
            template: Template {
                template_id: template_id_counter,
                parts: template_parts,
            },
            docs_term_ids: docs_ids,
        });
        template_id_counter += 1;
    }

    template_and_docs
}
