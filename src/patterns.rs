use fnv::{FnvHashMap, FnvHashSet};

use crate::prelim_index::{PrelimDoc, PreliminaryIndex};

pub fn pattern_scan(index: &PreliminaryIndex) {
    let mut docs_by_fingerprint = FnvHashMap::default();
    // 1. Group by fingerprint
    for doc in &index.preliminary_docs {
        docs_by_fingerprint
            .entry(doc.fingerprint)
            .or_insert_with(Vec::new)
            .push(doc.token_type_with_term_ids.clone());
    }
    // 2. For each fingerprint, create a pattern
    for docs in docs_by_fingerprint.values() {
        detect_template_parts(docs);
        // Here you can do something with the pattern, like printing or storing it
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
    let mut columnar: Vec<Vec<u32>> = Vec::new();
    if num_docs == 0 {
        return;
    }
    let num_tokens = docs[0].without_whitespace().count();
    columnar.resize(num_tokens + 1, Vec::new());
    // Make sure they all have the same number of tokens
    //for doc in docs {
    //if doc.without_whitespace().count() != num_tokens {
    //panic!(
    //"Documents have different number of tokens: expected {}, got {}",
    //num_tokens,
    //doc.without_whitespace().count()
    //);
    //}
    //}

    for doc in docs {
        for (i, token) in doc.without_whitespace().enumerate() {
            columnar[i].push(token.term_id());
        }
    }
    // 2. For each position, check how many distinct term_ids are there
    // We can early exit if we find a position with too many distinct term_ids

    let mut is_token_pos_template = vec![false; num_tokens + 1];
    for (i, column) in columnar.iter().enumerate() {
        let mut term_id_counts: FnvHashMap<u32, u32> = FnvHashMap::default();
        for term_id in column {
            term_id_counts
                .entry(*term_id)
                .and_modify(|count| *count += 1)
                .or_insert(1);
            if term_id_counts.len() > 10 {
                // Too many distinct term_ids, we can skip this position
                break;
            }
        }
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
                columnar[i]
                    .iter()
                    .cloned()
                    .collect::<FnvHashSet<u32>>()
                    .len()
            );
        }
    }
    println!(
        "Found {} template positions out of {} total positions",
        num_templates, num_tokens
    );
}
