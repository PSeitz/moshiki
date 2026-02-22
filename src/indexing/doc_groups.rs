use super::doc_groups_hashmap::DocGroups as DocGroupsHashMap;
use crate::{
    Token,
    indexing::{DocGroup, doc_groups_hashmap::Fingerprint, termmap::IndexingTermmap},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroupId {
    num_tokens: u32,
    /// Unique identifier for the group, guaranteed to be unique and never reused.
    id: Fingerprint,
}

/// `DocGroups` bucketed by token length
///
#[derive(Debug, Default, Clone)]
pub struct DocGroupsByLen {
    /// Buckets keyed by `tokens.len()`.
    group_by_token_len: Vec<DocGroupsHashMap>,
}

impl DocGroupsByLen {
    /// Ensures the bucket for `token_len` exists.
    #[inline]
    fn ensure_bucket(&mut self, token_len: usize) {
        if self.group_by_token_len.len() <= token_len {
            self.group_by_token_len
                .resize_with(token_len + 1, Default::default);
        }
    }

    /// Inserts a document
    ///
    /// * All documents with identical token types end up in the same group.
    pub fn insert(&mut self, tokens: &[Token], line: &str, term_hash_map: &mut IndexingTermmap) {
        let len = tokens.len();
        self.ensure_bucket(len);
        let entry = &mut self.group_by_token_len[len];
        entry.insert(tokens, line, term_hash_map);
    }

    /// Total number of *groups*.
    #[inline]
    pub fn num_groups(&self) -> usize {
        self.group_by_token_len
            .iter()
            .map(DocGroupsHashMap::len)
            .sum()
    }

    /// Immutable iterator over *(GroupId, &PrelimDocGroup)*.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (GroupId, &DocGroup)> {
        self.group_by_token_len
            .iter()
            .enumerate()
            .flat_map(|(num_tokens, bucket)| {
                bucket.iter().map(move |(id, group)| {
                    (
                        GroupId {
                            num_tokens: num_tokens as u32,
                            id,
                        },
                        group,
                    )
                })
            })
    }

    /// Mutable access via `GroupId` (linear scan in its bucket).
    #[inline]
    pub(crate) fn get_mut(&mut self, id: GroupId) -> Option<&mut DocGroup> {
        self.group_by_token_len
            .get_mut(id.num_tokens as usize)?
            .get_mut(id.id)
    }

    /// Shared access via `GroupId`.
    #[inline]
    pub(crate) fn get(&self, id: GroupId) -> Option<&DocGroup> {
        self.group_by_token_len
            .get(id.num_tokens as usize)?
            .get(id.id)
    }

    /// Removes and returns the group identified by `id`, if present.
    pub(crate) fn remove(&mut self, id: GroupId) -> Option<DocGroup> {
        if let Some(bucket) = self.group_by_token_len.get_mut(id.num_tokens as usize) {
            return bucket.remove(id.id);
        }
        None
    }

    /// Iterator over all groups.
    #[inline]
    pub(crate) fn values(&self) -> impl Iterator<Item = &DocGroup> {
        self.group_by_token_len.iter().flat_map(|b| b.values())
    }

    /// Mutable iterator over all groups.
    #[inline]
    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut DocGroup> {
        self.group_by_token_len
            .iter_mut()
            .flat_map(|b| b.values_mut())
    }

    pub(crate) fn insert_new_group(&mut self, group: DocGroup) {
        let len = group.template.tokens.len();
        self.ensure_bucket(len);
        self.group_by_token_len[len].insert_duplicate(group);
    }
}
