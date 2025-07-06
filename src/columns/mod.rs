use std::path::{Path, PathBuf};

mod read;
mod write;

pub use read::*;
pub use write::*;

use crate::TemplateId;

pub fn get_template_path(folder: &Path, template_id: TemplateId) -> PathBuf {
    folder.join(format!("template_{}.zst", template_id.0))
}
