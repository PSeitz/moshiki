use std::{
    io,
    path::{Path, PathBuf},
};

use super::{
    patterns::{assign_template_ids, merge_templates},
    preliminary_index::preliminary_index,
    term_id_idx_to_template_ids,
    write_dict::write_dictionary_and_generate_mapping,
};
use crate::{
    columns::write::write_column_and_remap, constants::DICTIONARY_NAME,
    indexing::patterns::split_templates, templates::write_templates,
};

/// IndexWriter is responsible for indxing log lines and writing the index to disk.
pub struct IndexWriter {
    output_folder: PathBuf,
}

impl IndexWriter {
    /// Creates a new IndexWriter with the specified output folder.
    pub fn new(output_folder: String) -> Self {
        IndexWriter {
            output_folder: output_folder.into(),
        }
    }

    /// Indexes the provided lines and writes the index to disk.
    pub fn index<T: Into<String>>(
        &self,
        lines: impl Iterator<Item = T>,
        _report: bool,
    ) -> io::Result<()> {
        let mut preliminary_index = preliminary_index(lines);
        // More templates
        if std::env::var("ST").is_ok() {
            split_templates(&mut preliminary_index);
        }
        // Less templates
        merge_templates(&mut preliminary_index);

        if std::env::var("STATS").is_ok() {
            preliminary_index.print_stats();
        }
        assign_template_ids(&mut preliminary_index);
        let term_id_idx = term_id_idx_to_template_ids(&preliminary_index);
        let old_to_new_id_map = write_dictionary_and_generate_mapping(
            &self.output_folder.join(DICTIONARY_NAME),
            &preliminary_index.term_hash_map.regular,
            &term_id_idx,
        )?;

        write_templates(&preliminary_index, Path::new(&self.output_folder))?;

        for group in preliminary_index.doc_groups.values() {
            write_column_and_remap(&self.output_folder, group, &old_to_new_id_map)?;
        }
        Ok(())
    }
}
