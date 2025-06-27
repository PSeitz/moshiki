use fnv::FnvHashMap;

use crate::prelim_index::{PrelimDoc, PreliminaryIndex};

pub fn pattern_scan(index: &PreliminaryIndex) {
    let mut term_id_to_term_map: Vec<&[u8]> = vec![&[]; index.term_hash_map.len()];
    for (term_bytes, old_id_addr) in index.term_hash_map.iter() {
        let old_id: u32 = index.term_hash_map.read(old_id_addr);
        term_id_to_term_map[old_id as usize] = term_bytes;
    }

    for docs_vec in index.preliminary_docs.iter() {
        if docs_vec.is_empty() {
            continue;
        }
        detect_template_parts(docs_vec, &term_id_to_term_map);
    }
}

/// Detect template columns in a group of documents
pub fn detect_template_parts(docs: &[PrelimDoc], new_id_to_term_map: &[&[u8]]) {
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

    let mut column_term_id_counts: Vec<FnvHashMap<u32, u32>> =
        vec![FnvHashMap::default(); num_tokens];

    for doc in docs {
        for (i, token) in doc.without_whitespace().enumerate() {
            if column_term_id_counts[i].len() > 10 {
                // If we already have too many distinct term_ids, we can skip this position
                continue;
            }
            column_term_id_counts[i]
                .entry(token.term_id())
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }

    let max_distinct_terms_threshold = 10; // A position is a template if it has <= 10 distinct terms
    let min_most_frequent_term_percentage = 0.9; // Or if the most frequent term appears in >= 90% of documents

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
            let terms: Vec<String> = term_ids
                .iter()
                .map(|&id| String::from_utf8_lossy(new_id_to_term_map[id as usize]).to_string())
                .collect();
            let terms_with_percentages: Vec<String> = term_ids
                .iter()
                .map(|&id| {
                    let term = String::from_utf8_lossy(new_id_to_term_map[id as usize]).to_string();
                    let count = *column_term_id_counts[i].get(&id).unwrap_or(&0);
                    let percentage = (count as f64 / num_docs as f64) * 100.0;
                    format!("{}: {:.2}%", term, percentage)
                })
                .collect();
            println!("Position {}: Template with terms: {:?}", i, terms_with_percentages);
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
                i,
                num_distinct_terms,
                most_frequent_term_percentage
            );
        }
    }
    println!("{num_templates}/{num_tokens} are templates",);
}
