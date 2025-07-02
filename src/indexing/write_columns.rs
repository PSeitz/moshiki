use super::pattern_detection::TemplateAndDocs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn get_template_path(folder: &Path, template_id: u32) -> PathBuf {
    folder.join(format!("template_{}.zst", template_id))
}

pub fn write_column(folder: &Path, template_and_doc: &TemplateAndDocs) -> std::io::Result<()> {
    let mut byte_buffer = Vec::new();
    for term_id in &template_and_doc.docs_term_ids {
        byte_buffer.extend_from_slice(&term_id.to_le_bytes());
    }

    let compressed_data = zstd::stream::encode_all(&*byte_buffer, 6).unwrap();
    let file_path = get_template_path(folder, template_and_doc.template.template_id);
    let mut file = File::create(file_path).unwrap();
    file.write_all(&compressed_data).unwrap();
    Ok(())
}
