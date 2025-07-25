use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::indexing::PrelimDocGroup;

use super::get_template_path;

/// The columns are flattened as [Column1Term1, Column1Term2, ..., Column2Term1, ...]
/// Each column has the same number of terms
pub fn write_column_and_remap(
    folder: &Path,
    group: &PrelimDocGroup,
    old_to_new_id_map: &[u32],
) -> std::io::Result<()> {
    //let mut byte_buffer = Vec::new();
    //for (term_id) in group.iter_columns() {
    //let mut num_buffer = Vec::new();
    //for term_id in term_id {
    //// Convert the term ID to the new ID using the mapping
    //let new_term_id = &old_to_new_id_map[*term_id as usize];
    //// Append the new term ID to the byte buffer
    //num_buffer.push(*new_term_id);
    //}
    //if !num_buffer.is_empty() {
    //let compressed_data: Vec<u8> =
    //q_compress::auto_compress(&num_buffer, q_compress::DEFAULT_COMPRESSION_LEVEL);
    //byte_buffer.extend_from_slice(&compressed_data);
    //}
    //}

    let mut byte_buffer = Vec::new();
    for term_id in group.iter_columns() {
        for term_id in term_id {
            // Convert the term ID to the new ID using the mapping
            let new_term_id = &old_to_new_id_map[*term_id as usize];
            // Append the new term ID to the byte buffer
            byte_buffer.extend_from_slice(&new_term_id.to_le_bytes());
        }
    }

    let compressed_data = zstd::stream::encode_all(&*byte_buffer, 6).unwrap();

    let file_path = get_template_path(folder, group.template.template_id);
    let mut file = File::create(file_path).unwrap();
    file.write_all(&compressed_data).unwrap();
    Ok(())
}
