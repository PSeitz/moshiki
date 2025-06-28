use std::collections::HashMap;

use fnv::FnvHashMap;

use crate::prelim_index::{PrelimDoc, PreliminaryIndex};

pub fn pattern_scan(index: &PreliminaryIndex) {
    let mut term_id_to_term_map: Vec<&[u8]> = vec![&[]; index.term_hash_map.len()];
    for (term_bytes, old_id_addr) in index.term_hash_map.iter() {
        let old_id: u32 = index.term_hash_map.read(old_id_addr);
        term_id_to_term_map[old_id as usize] = term_bytes;
    }

    for docs_vec in index.preliminary_docs.values() {
        if docs_vec.is_empty() {
            continue;
        }
        split_and_detect_templates(docs_vec, &term_id_to_term_map);
    }
}

fn split_and_detect_templates(docs: &[PrelimDoc], new_id_to_term_map: &[&[u8]]) {
    let num_docs = docs.len();
    if num_docs == 0 {
        return;
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
        detect_template_parts(docs, new_id_to_term_map, &column_term_id_counts);
    } else {
        let mut sub_groups: HashMap<Vec<u32>, Vec<PrelimDoc>> = HashMap::new();
        for doc in docs {
            let key: Vec<u32> = variant_positions
                .iter()
                .map(|&i| doc.without_whitespace().nth(i).unwrap().term_id())
                .collect();
            sub_groups.entry(key).or_default().push(doc.clone());
        }

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
            detect_template_parts(
                sub_group,
                new_id_to_term_map,
                &sub_group_column_term_id_counts,
            );
        }
    }
}

/// Detect template columns in a group of documents
pub fn detect_template_parts(
    docs: &[PrelimDoc],
    new_id_to_term_map: &[&[u8]],
    column_term_id_counts: &[FnvHashMap<u32, u32>],
) {
    let num_docs = docs.len();
    println!("Number of documents: {}", num_docs);
    // Create a pattern from the group of docs
    // For each position (ordinal in Vec<CompositeToken>) we check how many distinct term_ids are there
    // Low cardinality terms can be part of the pattern
    // then we only need to store the term_ids for the high cardinality terms in a columnar storage
    // 1. Convert into a columnar representation
    if num_docs == 0 {
        return;
    }
    let num_tokens = docs[0].without_whitespace().count();
    // 2. For each position, check how many distinct term_ids are there
    // We can early exit if we find a position with too many distinct term_ids

    let max_distinct_terms_threshold = if num_docs <= 5 { 1 } else { 5 }; // A position is a template if it has <= 5 distinct terms
    let min_most_frequent_term_percentage = 0.99; // Or if the most frequent term appears in >= 99% of documents

    let mut is_token_pos_template = vec![false; num_tokens];
    for (i, term_id_counts) in column_term_id_counts.iter().enumerate() {
        let num_distinct_terms = term_id_counts.len();
        let mut is_template = false;

        if num_distinct_terms <= max_distinct_terms_threshold {
            is_template = true;
        } else {
            // If there are too many distinct terms, check if one term is overwhelmingly frequent
            let mut max_term_count = 0;
            for &count in term_id_counts.values() {
                if count > max_term_count {
                    max_term_count = count;
                }
            }
            let most_frequent_term_percentage = max_term_count as f64 / num_docs as f64;
            if most_frequent_term_percentage >= min_most_frequent_term_percentage {
                is_template = true;
            }
        }
        is_token_pos_template[i] = is_template;
    }
    // Print how many distinct term_ids are there for each position
    // Print the template positions
    let mut num_templates = 0;
    for (i, is_template) in is_token_pos_template.iter().enumerate() {
        if *is_template {
            num_templates += 1;
            let term_ids: Vec<u32> = column_term_id_counts[i].keys().cloned().collect();
            let terms_with_percentages: Vec<String> = term_ids
                .iter()
                .map(|&id| {
                    let term = String::from_utf8_lossy(new_id_to_term_map[id as usize]).to_string();
                    let count = *column_term_id_counts[i].get(&id).unwrap_or(&0);
                    let total_terms_at_position: u32 = column_term_id_counts[i].values().sum();
                    let percentage = (count as f64 / total_terms_at_position as f64) * 100.0;
                    if term_ids.len() > 1 {
                        format!("{}: {:.2}%", term, percentage)
                    } else {
                        term
                    }
                })
                .collect();
            println!(
                "Position {}: Template with terms: {:?}",
                i, terms_with_percentages
            );
        } else {
            let num_distinct_terms = column_term_id_counts[i].len();
            let mut max_term_count = 0;
            for &count in column_term_id_counts[i].values() {
                if count > max_term_count {
                    max_term_count = count;
                }
            }
            let most_frequent_term_percentage = max_term_count as f64 / num_docs as f64;
            println!(
                "Position {}: Not a template. Distinct terms: {}, Most frequent term percentage: {:.2}",
                i, num_distinct_terms, most_frequent_term_percentage
            );
        }
    }
    println!("{num_templates}/{num_tokens} are templates",);
}
