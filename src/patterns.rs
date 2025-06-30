use fnv::FnvHashMap;

use crate::prelim_index::{PrelimDocGroup, PreliminaryIndex, TemplateToken, TemplateTokenWithMeta};

type TermIdMap<'a> = Vec<&'a [u8]>;

#[derive(Debug)]
pub struct TemplateAndDocs {
    pub template: Template,
    pub docs_term_ids: Vec<u32>,
}

#[derive(Debug)]
pub struct Template {
    pub template_id: u32,
    pub parts: Vec<TemplateTokenWithMeta>,
}

pub fn pattern_scan(index: &PreliminaryIndex, old_to_new_id_map: &[u32]) -> Vec<TemplateAndDocs> {
    let mut term_id_to_term_map: Vec<&[u8]> = vec![&[]; index.term_hash_map.len()];
    for (term_bytes, old_id) in index.term_hash_map.iter() {
        term_id_to_term_map[old_id as usize] = term_bytes;
    }

    let mut template_and_docs = Vec::new();
    let mut template_id_counter = 0;

    for group in index.preliminary_docs.values() {
        let num_docs = group.num_docs;
        if num_docs > 2_000_000 {
            print_stats_group(group, &term_id_to_term_map);
        }

        let mut term_ids = Vec::new();
        for template_token in &group.template.tokens {
            // Skip constant columns or whitespace columns
            match template_token.token {
                TemplateToken::Variable {
                    is_id_like: _,
                    column_index,
                } => {
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
            template: Template {
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
fn print_stats_group(group: &PrelimDocGroup, term_id_to_term_map: &TermIdMap) {
    let num_docs = group.num_docs;
    println!("\n--- Stats for template with {} docs ---", num_docs);
    for (col_idx, column_terms) in group.columns.iter().enumerate() {
        let mut counts = FnvHashMap::default();
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
