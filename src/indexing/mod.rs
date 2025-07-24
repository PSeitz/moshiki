pub(crate) mod doc_groups;
pub(crate) mod doc_groups_hashmap;
pub(crate) mod fingerprint;
pub(crate) mod index_writer;
pub(crate) mod patterns;
/// Indexes the input lines into a preliminary index structure.
pub(crate) mod preliminary_index;
pub(crate) mod termmap;
pub(crate) mod write_dict;

pub(crate) use fingerprint::fingerprint_tokens;
pub use index_writer::IndexWriter;
pub use preliminary_index::*;

pub(crate) use doc_groups::*;
