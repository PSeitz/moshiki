#[cfg(feature = "doc_groups_vec")]
pub mod doc_groups;
#[cfg(not(feature = "doc_groups_vec"))]
pub mod doc_groups_hashmap;
pub mod fingerprint;
pub mod index_writer;
pub mod patterns;
pub mod preliminary_index;
pub mod termmap;
pub mod write_dict;

pub use fingerprint::fingerprint;
pub use index_writer::IndexWriter;
pub use preliminary_index::*;

#[cfg(feature = "doc_groups_vec")]
pub use doc_groups::*;
#[cfg(not(feature = "doc_groups_vec"))]
pub use doc_groups_hashmap::*;
