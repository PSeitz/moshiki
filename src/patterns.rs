use fnv::FnvHashMap;

use crate::prelim_index::{PrelimDoc, PreliminaryIndex};

pub fn pattern_scan(index: &PreliminaryIndex) {
    // 2. For each fingerprint, create a pattern
    for docs_vec in index.preliminary_docs.iter() {
        if docs_vec.is_empty() {
            continue;
        }
        detect_template_parts(docs_vec);
    }
}

/// Detect template columns in a group of documents
pub fn detect_template_parts(docs: &[PrelimDoc]) {
    let num_docs = docs.len();
    println!("Number of documents: {}", num_docs);
    // Create a pattern from the group of docs
    // For each position (ordinal in Vec<CompositeToken>) we check how many distinct term_ids are there
    // Low cardinality terms can be part of the pattern
    // then we only need to store the term_ids for the high cardinality terms in a columnar storage
    // 1. Convert into a columnar representationlet mut pattern = String::new();
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

    let mut is_token_pos_template = vec![false; num_tokens];
    for (i, term_id_counts) in column_term_id_counts.iter().enumerate() {
        if term_id_counts.len() <= 10 {
            is_token_pos_template[i] = true;
        }
    }
    // Print how many distinct term_ids are there for each position
    // Print the template positions
    let mut num_templates = 0;
    for (i, is_template) in is_token_pos_template.iter().enumerate() {
        if *is_template {
            num_templates += 1;
            //let term_ids: HashSet<u32> = columnar[i].iter().cloned().collect();
            //println!("Position {}: Template with terms: {:?}", i, term_ids);
        } else {
            println!(
                "Position {}: Not a template with {} distinct terms",
                i,
                column_term_id_counts[i].len()
            );
        }
    }
    println!("{num_templates}/{num_tokens} are templates",);
}
