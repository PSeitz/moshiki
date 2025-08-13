//! This module provides functionality for reading and writing columns of data.

use std::path::{Path, PathBuf};

use crate::TemplateId;

pub mod read;
pub(crate) mod write;

/// Returns the path to the template file for a given template ID.
pub fn get_template_path(folder: &Path, template_id: TemplateId) -> PathBuf {
    folder.join(format!("{template_id:?}.col"))
}
