use std::{
    io,
    path::{Path, PathBuf},
};

use super::{
    patterns::{assign_template_ids, merge_templates},
    preliminary_index::preliminary_index,
    term_id_idx_to_template_ids,
    write_columns::write_column,
    write_dict::write_dictionary_and_generate_mapping,
};
use crate::{
    constants::{CATCH_ALL_DICTIONARY_NAME, DICTIONARY_NAME},
    templates::write_templates,
};

pub struct IndexWriter {
    output_folder: PathBuf,
}

impl IndexWriter {
    pub fn new(output_folder: String) -> Self {
        IndexWriter {
            output_folder: output_folder.into(),
        }
    }

    pub fn index(&self, lines: impl Iterator<Item = String>, _report: bool) -> io::Result<()> {
        let mut preliminary_index = preliminary_index(lines);
        merge_templates(&mut preliminary_index);
        if std::env::var("STATS").is_ok() {
            preliminary_index.print_stats();
        }
        assign_template_ids(&mut preliminary_index);
        let (term_id_idx, term_id_idx_catch_all) = term_id_idx_to_template_ids(&preliminary_index);
        let old_to_new_id_map = write_dictionary_and_generate_mapping(
            &self.output_folder.join(DICTIONARY_NAME),
            &preliminary_index.term_hash_map.regular,
            &term_id_idx,
        )?;
        let old_catch_all_to_new_id_map = write_dictionary_and_generate_mapping(
            &self.output_folder.join(CATCH_ALL_DICTIONARY_NAME),
            &preliminary_index.term_hash_map.catch_all,
            &term_id_idx_catch_all,
        )?;

        let templates_path = Path::new(&self.output_folder).join("templates.json");
        write_templates(&preliminary_index, &templates_path)?;

        for group in preliminary_index.doc_groups.values() {
            write_column(
                &self.output_folder,
                group,
                &old_to_new_id_map,
                &old_catch_all_to_new_id_map,
            )?;
        }
        Ok(())
    }
}
