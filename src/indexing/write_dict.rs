use std::{
    fs::File,
    io::{self, BufWriter},
    path::Path,
};

use super::{SingleOrHashSet, termmap::TermStore};
use tantivy_sstable::{
    SSTable,
    value::{ValueReader, ValueWriter},
};

pub fn write_dictionary_and_generate_mapping(
    path: &Path,
    term_hash_map: impl TermStore,
    term_id_to_template_id: &[SingleOrHashSet],
) -> io::Result<Vec<u32>> {
    let len = term_hash_map.len();
    let mut sorted_terms: Vec<(&[u8], u32)> = Vec::with_capacity(len);
    let max_old_id = term_hash_map.len() as u32;
    for (term_bytes, old_id) in term_hash_map.iter() {
        sorted_terms.push((term_bytes, old_id));
    }

    sorted_terms.sort_unstable_by(|a, b| a.0.cmp(b.0));

    let mut old_to_new_id_map: Vec<u32> = vec![0; (max_old_id + 1) as usize];
    let dictionary_path = path;
    let wtr = BufWriter::new(File::create(dictionary_path)?);

    let mut builder = tantivy_sstable::Dictionary::<VecU32ValueSSTable>::builder(wtr)?;

    // We may have duplicate terms, so we need to ensure that we assign the same new ID to the
    // same term and not insert it multiple times.
    let mut new_id: u32 = 0;
    let mut template_ids = Vec::new();

    let mut iter = sorted_terms.into_iter().peekable();
    while let Some((term_bytes, old_id)) = iter.next() {
        //if !is_catch_all {
        //dbg!(std::str::from_utf8(term_bytes).unwrap_or("Invalid UTF-8"));
        //}
        old_to_new_id_map[old_id as usize] = new_id;
        term_id_to_template_id[old_id as usize].copy_into_vec(&mut template_ids);
        while let Some((next_term_bytes, _)) = iter.peek()
            && *next_term_bytes == term_bytes
        {
            let (_, old_id) = iter.next().unwrap();
            old_to_new_id_map[old_id as usize] = new_id;
            term_id_to_template_id[old_id as usize].copy_into_vec(&mut template_ids);
            if template_ids.len() > 1 {
                template_ids.sort_unstable();
                template_ids.dedup();
            }
        }
        if template_ids.len() > 1 {
            template_ids.sort_unstable();
        }
        if template_ids.is_empty() {
            // If there are no template IDs, we can skip inserting this term.
            // This can happen if the term is only in a constant
            continue;
        }

        builder.insert(term_bytes, &template_ids)?;
        template_ids.clear();
        new_id += 1;
    }
    builder.finish().map_err(io::Error::other)?;
    Ok(old_to_new_id_map)
}

pub struct VecU32ValueSSTable;

impl SSTable for VecU32ValueSSTable {
    type Value = Vec<u32>;
    type ValueReader = VecU32ValueReader;
    type ValueWriter = VecU32ValueWriter;
}

#[derive(Default)]
pub struct VecU32ValueReader {
    vals: Vec<Vec<u32>>,
}

impl ValueReader for VecU32ValueReader {
    type Value = Vec<u32>;

    #[inline(always)]
    fn value(&self, idx: usize) -> &Self::Value {
        &self.vals[idx]
    }

    fn load(&mut self, mut data: &[u8]) -> io::Result<usize> {
        let original_num_bytes = data.len();
        self.vals.clear();

        // The first 4 bytes are the number of blocks
        let num_blocks = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
        data = &data[4..];

        for _ in 0..num_blocks {
            // Each block starts with a 4-byte length
            let segment_len = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
            data = &data[4..];

            // Read the segment IDs for this block
            let mut segment_ids = Vec::with_capacity(segment_len);
            for _ in 0..segment_len {
                let segment_id = u32::from_le_bytes(data[..4].try_into().unwrap());
                segment_ids.push(segment_id);
                data = &data[4..];
            }
            self.vals.push(segment_ids);
        }

        // Return the number of bytes consumed
        Ok(original_num_bytes - data.len())
    }
}

#[derive(Default)]
pub struct VecU32ValueWriter {
    vals: Vec<Vec<u32>>,
}

impl ValueWriter for VecU32ValueWriter {
    type Value = Vec<u32>;

    fn write(&mut self, val: &Self::Value) {
        self.vals.push(val.to_vec());
    }

    fn serialize_block(&self, output: &mut Vec<u8>) {
        let num_blocks = self.vals.len() as u32;
        output.extend_from_slice(&num_blocks.to_le_bytes());
        for vals in &self.vals {
            let len = vals.len() as u32;
            output.extend_from_slice(&len.to_le_bytes());
            for &segment_id in vals.iter() {
                output.extend_from_slice(&segment_id.to_le_bytes());
            }
        }
    }

    fn clear(&mut self) {
        self.vals.clear();
    }
}
