use std::path::{Path, PathBuf};

mod read;
mod write;

pub use read::*;
pub use write::*;

pub fn get_template_path(folder: &Path, template_id: u32) -> PathBuf {
    folder.join(format!("template_{template_id}.zst",))
}
