//! This module provides functionality for reading columns of data.
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::TemplateId;

use super::get_template_path;

/// A column of data, containing a list of term IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    data: Vec<u32>,
}
impl Column {
    /// Creates a new column from a vector of term IDs.
    pub fn new(data: Vec<u32>) -> Self {
        Column { data }
    }

    /// Returns the term ID at a given index.
    pub fn term_at(&self, index: usize) -> Option<u32> {
        self.data.get(index).copied()
    }
    /// Returns an iterator over the term IDs in this column.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.data.iter().copied()
    }
}

/// A collection of columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Columns {
    data: Vec<Column>,
}
impl Columns {
    /// Creates a new collection of columns.
    pub fn new(data: Vec<Column>) -> Self {
        Columns { data }
    }

    /// Returns an iterator over the columns.
    pub fn iter_columns(&self) -> impl Iterator<Item = &Column> {
        self.data.iter()
    }
    /// Returns an iterator over the term IDs for a given document ID.
    pub fn get_term_ids(&self, doc: u32) -> impl Iterator<Item = u32> + '_ {
        self.iter_columns()
            .flat_map(move |column| column.term_at(doc as usize))
    }

    /// Returns an iterator over the document IDs that match a given predicate.
    pub fn get_doc_ids<'a>(
        &'a self,
        match_fn: &'a impl Fn(u32) -> bool,
    ) -> impl Iterator<Item = u32> + 'a {
        self.iter_columns().flat_map(move |column| {
            column.iter().enumerate().filter_map(move |(docid, term)| {
                if match_fn(term) {
                    Some(docid as u32)
                } else {
                    None
                }
            })
        })
    }
}

/// Decompresses a column from a file.
///
/// # Arguments
///
/// * `folder` - The folder containing the column file.
/// * `template_id` - The ID of the template to decompress.
/// * `num_docs` - The number of documents in the column.
///
/// # Errors
///
/// Returns an error if the column file cannot be read or decompressed.
pub fn decompress_column(
    folder: &Path,
    template_id: TemplateId,
    num_docs: usize,
) -> std::io::Result<Columns> {
    let file_path = get_template_path(folder, template_id);
    let file = File::open(file_path)?;
    let mut decoder = zstd::Decoder::new(file)?;
    let mut decompressed_data = Vec::new();
    decoder.read_to_end(&mut decompressed_data)?;

    let mut columns = Vec::new();
    // Convert the decompressed data into a vector of u32
    let mut terms = Vec::new();
    for chunk in decompressed_data.chunks_exact(4) {
        let term_id = u32::from_le_bytes(chunk.try_into().unwrap());
        terms.push(term_id);
        if terms.len() == num_docs {
            columns.push(Column::new(terms.clone()));
            terms.clear();
        }
    }

    Ok(Columns::new(columns))
}
