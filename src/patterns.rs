use fnv::FnvHashMap;

use crate::prelim_index::{PrelimDocGroup, PreliminaryIndex};

#[derive(Debug)]
pub struct TemplateAndDocs {
    pub template: Template,
    pub docs_term_ids: Vec<u32>,
}

struct TemplateIdProvider {
    next_template_id: u32,
}
impl TemplateIdProvider {
    fn new() -> Self {
        TemplateIdProvider {
            next_template_id: 0,
        }
    }

    fn next_id(&mut self) -> u32 {
        let id = self.next_template_id;
        self.next_template_id += 1;
        id
    }
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

#[derive(Debug)]
pub struct TemplatedDocument {
    pub template_id: u32,
    /// Term IDs for the placeholders in the template.
    pub placeholder_values: Vec<u32>,
}

pub fn pattern_scan(index: &PreliminaryIndex, old_to_new_id_map: &[u32]) -> Vec<TemplateAndDocs> {
    let mut term_id_to_term_map: Vec<&[u8]> = vec![&[]; index.term_hash_map.len()];
    for (term_bytes, old_id_addr) in index.term_hash_map.iter() {
        let old_id: u32 = index.term_hash_map.read(old_id_addr);
        term_id_to_term_map[old_id as usize] = term_bytes;
    }

    let mut template_and_docs = Vec::new();

    let mut next_template_id = TemplateIdProvider::new();
    for prelim_doc_group in index.preliminary_docs.values() {
        if prelim_doc_group.is_empty() {
            continue;
        }
        let new_template_and_docs = split_and_detect_templates(
            prelim_doc_group,
            &term_id_to_term_map,
            &mut next_template_id,
            old_to_new_id_map,
        );
        template_and_docs.extend(new_template_and_docs);
    }
    template_and_docs
}

fn split_and_detect_templates(
    docs: &PrelimDocGroup,
    new_id_to_term_map: &[&[u8]],
    next_template_id: &mut TemplateIdProvider,
    old_to_new_id_map: &[u32],
) -> Vec<TemplateAndDocs> {
    let num_docs = docs.num_docs();
    if num_docs == 0 {
        return Vec::new();
    }

    let num_tokens = docs.num_tokens();
    let mut column_term_id_counts: Vec<FnvHashMap<u32, u32>> =
        vec![FnvHashMap::default(); num_tokens];

    // Assumption is that whitespace tokens are exactly the same for all documents here
    // This depends on the grouping of documents in the preliminary index
    for doc in docs.iter() {
        for (i, token) in doc.iter().enumerate() {
            column_term_id_counts[i]
                .entry(token.term_id())
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }

    //let variant_positions: Vec<usize> = (0..num_tokens)
    //.filter(|&i| {
    //let num_distinct_terms = column_term_id_counts[i].len();
    //num_distinct_terms > 1 && num_distinct_terms <= 5
    //})
    //.collect();

    //if true || variant_positions.is_empty() {
    vec![detect_template(
        docs,
        new_id_to_term_map,
        &column_term_id_counts,
        next_template_id,
        old_to_new_id_map,
    )]
    //} else {
    //let mut sub_groups: HashMap<Vec<u32>, PrelimDocGroup> = HashMap::new();
    //for doc in docs.iter() {
    //let key: Vec<u32> = variant_positions
    //.iter()
    //.map(|&i| doc.token_at(i).term_id())
    //.collect();
    //sub_groups.entry(key).or_default().push_indexed(*doc);
    //}

    //let mut templates_and_docs = Vec::new();

    //for sub_group in sub_groups.values() {
    //let mut sub_group_column_term_id_counts: Vec<FnvHashMap<u32, u32>> =
    //vec![FnvHashMap::default(); num_tokens];
    //for doc in sub_group {
    //for (i, token) in doc.iter().enumerate() {
    //sub_group_column_term_id_counts[i]
    //.entry(token.term_id())
    //.and_modify(|count| *count += 1)
    //.or_insert(1);
    //}
    //}
    //let new_template_and_docs = detect_template(
    //sub_group,
    //new_id_to_term_map,
    //&sub_group_column_term_id_counts,
    //next_template_id,
    //old_to_new_id_map,
    //);
    //templates_and_docs.push(new_template_and_docs);
    //}
    //templates_and_docs
    //}
}

fn detect_template(
    docs: &PrelimDocGroup,
    new_id_to_term_map: &[&[u8]],
    column_term_id_counts: &[FnvHashMap<u32, u32>],
    template_id: &mut TemplateIdProvider,
    old_to_new_id_map: &[u32],
) -> TemplateAndDocs {
    let num_tokens = docs.num_tokens();

    let mut template_parts = Vec::new();
    for term_id_counts in column_term_id_counts.iter().take(num_tokens) {
        let num_distinct_terms = term_id_counts.len();

        if num_distinct_terms == 1 {
            // If there's only one distinct term, it must be the constant term for this position.
            let (&term_id, _) = term_id_counts.iter().next().unwrap();
            let term = String::from_utf8_lossy(new_id_to_term_map[term_id as usize]).to_string();
            template_parts.push(TemplatePart::Constant(term));
        } else {
            template_parts.push(TemplatePart::Placeholder);
        }
    }

    let mut templated_docs = Vec::new();
    for doc in docs.iter() {
        let placeholder_values =
            doc.iter()
                .enumerate()
                .filter_map(|(i, token)| match template_parts[i] {
                    TemplatePart::Placeholder => Some(old_to_new_id_map[token.term_id() as usize]),
                    _ => None,
                });
        templated_docs.extend(placeholder_values);
    }

    TemplateAndDocs {
        template: Template {
            template_id: template_id.next_id(),
            parts: template_parts,
        },
        docs_term_ids: templated_docs,
    }
}
