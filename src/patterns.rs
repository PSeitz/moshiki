use std::collections::HashMap;

use fnv::FnvHashMap;

use crate::prelim_index::{CompositeToken, PrelimDoc, PreliminaryIndex};

#[derive(Debug)]
pub struct Template {
    pub parts: Vec<TemplatePart>,
}

#[derive(Debug)]
pub enum TemplatePart {
    Constant(u32),
    Placeholder,
}

#[derive(Debug)]
pub struct TemplatedDocument {
    pub template_id: u32,
    pub placeholder_values: Vec<u32>,
}

pub fn pattern_scan(index: &PreliminaryIndex) -> (Vec<Template>, Vec<TemplatedDocument>) {
    let mut term_id_to_term_map: Vec<&[u8]> = vec![&[]; index.term_hash_map.len()];
    for (term_bytes, old_id_addr) in index.term_hash_map.iter() {
        let old_id: u32 = index.term_hash_map.read(old_id_addr);
        term_id_to_term_map[old_id as usize] = term_bytes;
    }

    let mut templates = Vec::new();
    let mut templated_docs = Vec::new();

    for docs_vec in index.preliminary_docs.values() {
        if docs_vec.is_empty() {
            continue;
        }
        let (new_templates, new_templated_docs) =
            split_and_detect_templates(docs_vec, &term_id_to_term_map, templates.len() as u32);
        templates.extend(new_templates);
        templated_docs.extend(new_templated_docs);
    }
    (templates, templated_docs)
}

fn split_and_detect_templates(
    docs: &[PrelimDoc],
    new_id_to_term_map: &[&[u8]],
    template_id_offset: u32,
) -> (Vec<Template>, Vec<TemplatedDocument>) {
    let num_docs = docs.len();
    if num_docs == 0 {
        return (Vec::new(), Vec::new());
    }

    let num_tokens = docs[0].without_whitespace().count();
    let mut column_term_id_counts: Vec<FnvHashMap<u32, u32>> =
        vec![FnvHashMap::default(); num_tokens];

    for doc in docs {
        for (i, token) in doc.without_whitespace().enumerate() {
            column_term_id_counts[i]
                .entry(token.term_id())
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }

    let variant_positions: Vec<usize> = (0..num_tokens)
        .filter(|&i| {
            let num_distinct_terms = column_term_id_counts[i].len();
            num_distinct_terms > 1 && num_distinct_terms <= 5
        })
        .collect();

    if variant_positions.is_empty() {
        detect_template(
            docs,
            new_id_to_term_map,
            &column_term_id_counts,
            template_id_offset,
        )
    } else {
        let mut sub_groups: HashMap<Vec<u32>, Vec<PrelimDoc>> = HashMap::new();
        for doc in docs {
            let key: Vec<u32> = variant_positions
                .iter()
                .map(|&i| doc.without_whitespace().nth(i).unwrap().term_id())
                .collect();
            sub_groups.entry(key).or_default().push(doc.clone());
        }

        let mut templates = Vec::new();
        let mut templated_docs = Vec::new();
        let mut current_template_id = template_id_offset;

        for sub_group in sub_groups.values() {
            let mut sub_group_column_term_id_counts: Vec<FnvHashMap<u32, u32>> =
                vec![FnvHashMap::default(); num_tokens];
            for doc in sub_group {
                for (i, token) in doc.without_whitespace().enumerate() {
                    sub_group_column_term_id_counts[i]
                        .entry(token.term_id())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                }
            }
            let (new_templates, new_templated_docs) = detect_template(
                sub_group,
                new_id_to_term_map,
                &sub_group_column_term_id_counts,
                current_template_id,
            );
            templates.extend(new_templates);
            templated_docs.extend(new_templated_docs);
            current_template_id = templates.len() as u32;
        }
        (templates, templated_docs)
    }
}

fn detect_template(
    docs: &[PrelimDoc],
    new_id_to_term_map: &[&[u8]],
    column_term_id_counts: &[FnvHashMap<u32, u32>],
    template_id: u32,
) -> (Vec<Template>, Vec<TemplatedDocument>) {
    let num_docs = docs.len();
    if num_docs == 0 {
        return (Vec::new(), Vec::new());
    }

    let num_tokens = docs[0].without_whitespace().count();
    let max_distinct_terms_threshold = if num_docs <= 5 { 1 } else { 5 };
    let min_most_frequent_term_percentage = 0.99;

    let mut template_parts = Vec::new();
    for i in 0..num_tokens {
        let term_id_counts = &column_term_id_counts[i];
        let num_distinct_terms = term_id_counts.len();

        if num_distinct_terms <= max_distinct_terms_threshold {
            let most_frequent_term = term_id_counts
                .iter()
                .max_by_key(|(_, count)| *count)
                .unwrap()
                .0;
            template_parts.push(TemplatePart::Constant(*most_frequent_term));
        } else {
            template_parts.push(TemplatePart::Placeholder);
        }
    }

    let mut templated_docs = Vec::new();
    for doc in docs {
        let placeholder_values: Vec<u32> = doc
            .without_whitespace()
            .enumerate()
            .filter_map(|(i, token)| match template_parts[i] {
                TemplatePart::Placeholder => Some(token.term_id()),
                _ => None,
            })
            .collect();
        templated_docs.push(TemplatedDocument {
            template_id,
            placeholder_values,
        });
    }

    (vec![Template { parts: template_parts }], templated_docs)
}
