use fxhash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::indexing::{
    IndexingTemplateToken, PrelimDocGroup, PreliminaryIndex, TemplateTokenWithMeta,
};

type TermIdMap<'a> = Vec<&'a [u8]>;

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateAndDocs {
    pub template: IndexingTemplate,
    pub docs_term_ids: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct IndexingTemplate {
    pub template_id: u32,
    pub parts: Vec<TemplateTokenWithMeta>,
}

pub fn merge_templates(index: &mut PreliminaryIndex) {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum MergeableTokenGroup {
        Constant(String),
        Variable,
        Whitespace(u32),
    }

    let mut token_group_to_fingerprints: FxHashMap<Vec<MergeableTokenGroup>, Vec<u64>> =
        FxHashMap::default();
    for (pos, group) in &index.doc_groups {
        let mergeable_token_types: Vec<MergeableTokenGroup> = group
            .template
            .tokens
            .iter()
            .map(|token| match &token.token {
                IndexingTemplateToken::Constant(constant_token) => {
                    // Wo only consider merging them if we don't have many docs
                    if group.num_docs < 1000 {
                        MergeableTokenGroup::Variable
                    } else {
                        MergeableTokenGroup::Constant(constant_token.text.to_string())
                    }
                }
                IndexingTemplateToken::Variable { .. } => {
                    // Variable tokens are always mergeable
                    MergeableTokenGroup::Variable
                }
                IndexingTemplateToken::Whitespace(num) => {
                    if group.num_docs < 100 {
                        MergeableTokenGroup::Variable
                    } else {
                        MergeableTokenGroup::Whitespace(*num)
                    }
                }
            })
            .collect();
        token_group_to_fingerprints
            .entry(mergeable_token_types)
            .and_modify(|e| e.push(*pos))
            .or_insert(vec![*pos]);
    }

    // For each group, we will group them by their token types
    for (token_group, fingerprints) in token_group_to_fingerprints {
        if fingerprints.len() < 2 {
            continue; // No need to merge if there's only one group
        }
        // At first convert const tokens to variable ones
        for (token_idx, token_group_type) in token_group.iter().enumerate() {
            if let MergeableTokenGroup::Variable = token_group_type {
                // Iterate over the indices and convert the constant tokens to variable tokens
                for &idx in &fingerprints {
                    let group = &mut index.doc_groups.get_mut(&idx).unwrap();
                    // Convert the constant token at the current index to a variable token
                    group.convert_to_variable(token_idx, &mut index.term_hash_map);
                }
            }
        }

        // append all to the first group,
        let mut first_group = index.doc_groups.remove(&fingerprints[0]).unwrap();
        for &idx in &fingerprints[1..] {
            first_group.append(index.doc_groups.get(&idx).unwrap());
            index.doc_groups.remove(&idx);
        }
        // Insert the merged group back into the index
        index.doc_groups.insert(fingerprints[0], first_group);
    }
}

pub fn pattern_detection(
    index: &PreliminaryIndex,
    old_to_new_id_map: &[u32],
) -> Vec<TemplateAndDocs> {
    let mut term_id_to_term_map: Vec<&[u8]> = vec![&[]; index.term_hash_map.len()];
    for (term_bytes, old_id) in index.term_hash_map.iter() {
        term_id_to_term_map[old_id as usize] = term_bytes;
    }

    let mut template_and_docs = Vec::new();
    let mut template_id_counter = 0;

    for group in index.doc_groups.values() {
        let num_docs = group.num_docs;
        if num_docs > 2_000_000 {
            print_stats_group(group, template_id_counter, &term_id_to_term_map);
        }

        let mut term_ids = Vec::new();
        for template_token in &group.template.tokens {
            // Skip constant columns or whitespace columns
            match template_token.token {
                IndexingTemplateToken::Variable {
                    is_id_like: _,
                    column_index,
                } => {
                    //if is_id_like
                    //&& template_id_counter == 22
                    //&& (column_index == 0 || column_index == 1)
                    //{
                    //// Special case skip
                    //println!(
                    //"Template ID: {template_id_counter}, Column Index: {column_index}"
                    //);
                    //continue;
                    //}
                    for term_id in &group.columns[column_index] {
                        term_ids.push(old_to_new_id_map[*term_id as usize]);
                    }
                }
                _ => continue,
            }
        }
        // Write row
        //for doc in 0..num_docs {
        //for column in &group.columns {
        //let term_id = &column[doc];
        //term_ids.push(old_to_new_id_map[*term_id as usize]);
        //}
        //}

        template_and_docs.push(TemplateAndDocs {
            template: IndexingTemplate {
                template_id: template_id_counter,
                parts: group.template.tokens.clone(),
            },
            docs_term_ids: term_ids,
        });
        template_id_counter += 1;
    }

    template_and_docs
}

/// Calculates and prints term frequency statistics for large groups.
fn print_stats_group(
    group: &PrelimDocGroup,
    template_id_counter: u32,
    term_id_to_term_map: &TermIdMap,
) {
    let num_docs = group.num_docs;
    println!("\n--- Stats for template {template_id_counter} with {num_docs} docs ---");
    for (col_idx, column_terms) in group.columns.iter().enumerate() {
        let mut counts = FxHashMap::default();
        for &term_id in column_terms {
            *counts.entry(term_id).or_insert(0) += 1;
        }

        let mut sorted_counts: Vec<_> = counts.into_iter().collect();
        sorted_counts.sort_by_key(|&(_, count)| std::cmp::Reverse(count));

        println!(
            "  Column {}: ({} unique terms)",
            col_idx,
            sorted_counts.len()
        );
        if sorted_counts.len() < 500 {
            // Print histogram
            //
            // get the frist 5 percentages
            let percentages: Vec<_> = sorted_counts
                .iter()
                .map(|(_, count)| (*count as f32 / num_docs as f32) * 100.0)
                .collect();

            let term_bytes = term_id_to_term_map[sorted_counts[0].0 as usize];
            let term_string = String::from_utf8_lossy(term_bytes);
            println!(
                "    Top 5: ({})  Top:{term_string:?}: {}",
                percentages
                    .iter()
                    .take(5)
                    .map(|p| format!("{:.2}%", p))
                    .collect::<Vec<_>>()
                    .join(", "),
                sorted_counts[0].1
            );
        }
    }
}
