use fxhash::{FxHashMap, FxHashSet};

use crate::indexing::{
    ConstTemplateToken, DocGroup, GroupId, IndexingTemplateToken, PreliminaryIndex,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
// Token groups that can be merged in a template
enum MergeableTokenGroup {
    // Can merge if the String matches
    Constant(Vec<u8>),
    // Can always be merged currently
    Variable,
    // Can merge if the number of whitespace tokens matches
    #[cfg(feature = "whitespace")]
    Whitespace(u32),
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
            IndexingTemplateToken::Variable { .. } => MergeableTokenGroup::Variable,
            #[cfg(feature = "whitespace")]
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

/// NOTE: This does not improve compression.
///
/// Sometimes it makes sense to pull out a variable into a constant in a template.
/// E.g. if a variable occurs 100_000 times, we can be sure that it is worth it to convert it to a
/// constant.
/// The exact threshold depends on the compression (needs to be tested).
///
/// Ideally we would pull out also co-occurences of variables from different columns.
/// This is a little bit more expensive to check
///
/// Example: Below in column 1, we want to pull out the term id 1 in column 1 into a constant
/// Columns
/// 1 2 3 --> Move row to new group
/// 1 2 3 --> Move row to new group
/// 2 3 4 --> Stays in the same group
/// 1 1 1 --> Move row to new group
/// 1 4 3 --> Move row to new group
/// 1 1 5 --> Move row to new group
pub fn split_templates(index: &mut PreliminaryIndex) {
    // 1. Count the term frequencies in each group
    // 2. Extract into constants for all variables that occur more than the threshold

    // Read threashold from environment variable if set, or default
    let threshold = std::env::var("SPLIT_TEMPLATE_THRESHOLD")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(400_000);

    // Stage new groups and add afterwards
    let mut new_groups = Vec::new();
    // This may change later
    for group in index.doc_groups.values_mut() {
        // Collect term frequencies
        // TODO:: This can be done more efficiently.
        // E.g. We can use a vec there and reuse it between groups (if the group is large enough)
        let mut term_frequencies: FxHashMap<u32, u32> = FxHashMap::default();

        for token in group.template.tokens.clone() {
            if let IndexingTemplateToken::Variable { column_index, .. } = token.token {
                let column = &group.columns[column_index];
                for term_id in column {
                    *term_frequencies.entry(*term_id).or_insert(0) += 1;
                }
                // Convert variables to constants if they occur more than the threshold
                for (term_id, frequency) in &term_frequencies {
                    if *frequency > threshold {
                        println!("Num Values in Columns Before: {}", group.vals_in_columns());
                        let num_tokens = group.template.tokens.len();
                        println!(
                            "Moving term id {term_id} with freq {frequency} from template {num_tokens} to a constant",
                        );
                        let new_group = move_term_id_to_new_group(
                            group,
                            *term_id,
                            column_index,
                            &mut index.term_hash_map,
                        );
                        println!(
                            "Num Values in Columns After: {}, new_group {}, Total {}",
                            group.vals_in_columns(),
                            new_group.vals_in_columns(),
                            group.vals_in_columns() + new_group.vals_in_columns()
                        );
                        new_groups.push(new_group);
                        // Add the new group to the index
                    }
                    // TODO: Check if some columns are constants now
                }
                term_frequencies.clear();
            }
        }
    }

    for new_group in new_groups {
        index.doc_groups.insert_new_group(new_group);
    }
}
pub fn move_term_id_to_new_group(
    group: &mut DocGroup,
    term_id: u32,
    column_index: usize,
    term_hash_map: &mut crate::indexing::termmap::IndexingTermmap,
) -> DocGroup {
    let mut marked_rows_to_move_new_group = FxHashSet::default();
    for (row_idx, &term_id_in_row) in group.columns[column_index].iter().enumerate() {
        if term_id_in_row == term_id {
            marked_rows_to_move_new_group.insert(row_idx as u32);
        }
    }
    // Create a new group
    // TODO: That's copying too much, we don't need to copy all columns
    // Generally it could be handled as a projected view
    let mut new_group = group.clone();

    // Move the rows to the new group
    new_group.remove_rows(|row| marked_rows_to_move_new_group.contains(row));
    group.remove_rows(|row| !marked_rows_to_move_new_group.contains(row));
    // Update num_docs
    new_group.num_docs = marked_rows_to_move_new_group.len();
    group.num_docs -= new_group.num_docs;

    // Replace the variable token with a constant token
    for token in &mut new_group.template.tokens {
        if let IndexingTemplateToken::Variable {
            token_type,
            column_index: col_idx,
            is_id_like: _,
        } = &mut token.token
            && *col_idx == column_index
        {
            // Convert the variable to a constant
            let text = term_hash_map
                .regular
                .find_term_for_term_id(term_id)
                .to_vec();
            token.token =
                IndexingTemplateToken::Constant(ConstTemplateToken::new(*token_type, text.clone()));
        }
    }
    // Remove the column from the new group
    new_group.columns.remove(column_index);
    // Update the column_indices in the new group, if the column index is greater than the
    // column_index, we need to decrement it
    for token in &mut new_group.template.tokens {
        if let IndexingTemplateToken::Variable {
            column_index: col_idx,
            ..
        } = &mut token.token
            && *col_idx > column_index
        {
            *col_idx -= 1;
        }
    }

    new_group
}

pub fn merge_templates(index: &mut PreliminaryIndex) {
    let mut token_group_to_group_id: FxHashMap<Vec<MergeableTokenGroup>, Vec<GroupId>> =
        FxHashMap::default();
    for (group_id, group) in index.doc_groups.iter() {
        let mergeable_token_types: Vec<MergeableTokenGroup> = group
            .template
            .tokens
            .iter()
            .map(|token| MergeableTokenGroup::from_token(&token.token, group.num_docs))
            .collect();
        token_group_to_group_id
            .entry(mergeable_token_types)
            .and_modify(|e| e.push(group_id))
            .or_insert(vec![group_id]);
    }

    // For each group, we will group them by their token types
    for (token_group, group_id) in token_group_to_group_id {
        if group_id.len() < 2 {
            continue; // No need to merge if there's only one group
        }
        // At first convert const tokens to variable ones
        for (token_idx, token_group_type) in token_group.iter().enumerate() {
            if let MergeableTokenGroup::Variable = token_group_type {
                // Iterate over the indices and convert the constant tokens to variable tokens
                for &idx in &group_id {
                    let group = &mut index.doc_groups.get_mut(idx).unwrap();
                    // Convert the constant token at the current index to a variable token
                    group.convert_to_variable(token_idx, &mut index.term_hash_map);
                }
            }
        }

        // append all to the first group,
        let mut first_group = index.doc_groups.remove(group_id[0]).unwrap();
        for &idx in &group_id[1..] {
            first_group.append(index.doc_groups.get(idx).unwrap());
            index.doc_groups.remove(idx);
        }
        // Insert the merged group back into the index
        index.doc_groups.insert_new_group(first_group);
    }
}

pub fn assign_template_ids(index: &mut PreliminaryIndex) {
    for (template_id, group) in index.doc_groups.values_mut().enumerate() {
        group.template.template_id = (template_id as u32).into();
        group.template.num_docs = group.num_docs;
    }
}
