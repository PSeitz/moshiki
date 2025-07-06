use fxhash::FxHashMap;

use crate::indexing::{IndexingTemplateToken, PrelimDocGroup, PreliminaryIndex};

type TermIdMap<'a> = Vec<&'a [u8]>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
// Token groups that can be merged in a template
enum MergeableTokenGroup {
    // Can merge if the String matches
    Constant(Vec<u8>),
    // Can always be merged currently
    Variable,
    // Can merge if the number of whitespace tokens matches
    Whitespace(u32),
    // Catch all can only be merged with other catch alls
    CatchAll,
}

impl MergeableTokenGroup {
    fn from_token(token: &IndexingTemplateToken, num_docs: usize) -> Self {
        match token {
            IndexingTemplateToken::Constant(constant_token) => {
                if num_docs < 1000 {
                    MergeableTokenGroup::Variable
                } else {
                    MergeableTokenGroup::Constant(constant_token.text.to_vec())
                }
            }
            IndexingTemplateToken::Variable { token_type, .. } => {
                if token_type.is_catch_all() {
                    MergeableTokenGroup::CatchAll
                } else {
                    MergeableTokenGroup::Variable
                }
            }
            IndexingTemplateToken::Whitespace(num) => {
                if num_docs < 100 {
                    MergeableTokenGroup::Variable
                } else {
                    MergeableTokenGroup::Whitespace(*num)
                }
            }
        }
    }
}

pub fn merge_templates(index: &mut PreliminaryIndex) {
    let mut token_group_to_fingerprints: FxHashMap<Vec<MergeableTokenGroup>, Vec<u64>> =
        FxHashMap::default();
    for (pos, group) in &index.doc_groups {
        let mergeable_token_types: Vec<MergeableTokenGroup> = group
            .template
            .tokens
            .iter()
            .map(|token| MergeableTokenGroup::from_token(&token.token, group.num_docs))
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

pub fn assign_template_ids(index: &mut PreliminaryIndex) {
    for (template_id, group) in index.doc_groups.values_mut().enumerate() {
        group.template.template_id = (template_id as u32).into();
        group.template.num_docs = group.num_docs;
    }
}

/// Calculates and prints term frequency statistics for large groups.
pub fn print_stats_group(
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
                    .map(|p| format!("{p:.2}%"))
                    .collect::<Vec<_>>()
                    .join(", "),
                sorted_counts[0].1
            );
        }
    }
}
