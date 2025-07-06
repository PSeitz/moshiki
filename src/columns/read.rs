use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::get_template_path;

pub struct Column {
    data: Vec<u32>,
}
impl Column {
    pub fn new(data: Vec<u32>) -> Self {
        Column { data }
    }

    pub fn get_terms(&self) -> &[u32] {
        &self.data
    }

    pub fn term_at(&self, index: usize) -> Option<u32> {
        self.data.get(index).copied()
    }
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.data.iter().copied()
    }
}

pub struct Columns {
    data: Vec<Column>,
}
impl Columns {
    pub fn new(data: Vec<Column>) -> Self {
        Columns { data }
    }

    pub fn col_at(&self, index: usize) -> Option<&Column> {
        self.data.get(index)
    }
    pub fn iter_columns(&self) -> impl Iterator<Item = &Column> {
        self.data.iter()
    }
}

pub fn decompress_column(
    folder: &Path,
    template_id: u32,
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
