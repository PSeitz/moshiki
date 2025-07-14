use crate::{
    Token,
    indexing::{PrelimDocGroup, termmap::IndexingTermmap},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroupId {
    num_tokens: u32,
    /// Unique identifier for the group, guaranteed to be unique and never reused.
    id: u32,
}

/// `DocGroups` bucketed by token length
///
#[derive(Debug, Default, Clone)]
pub struct DocGroups {
    /// Buckets keyed by `tokens.len()`.
    group_by_token_len: Vec<Vec<PrelimDocGroup>>,
    next_group_id: u32,
}

impl DocGroups {
    /// Creates an empty collection.
    #[inline]
    pub fn new() -> Self {
        Self {
            group_by_token_len: Vec::new(),
            next_group_id: 0,
        }
    }

    /// Ensures the bucket for `token_len` exists.
    #[inline]
    fn ensure_bucket(&mut self, token_len: usize) {
        if self.group_by_token_len.len() <= token_len {
            self.group_by_token_len.resize_with(token_len + 1, Vec::new);
        }
    }

    /// Inserts a document
    ///
    /// * All documents with identical token types end up in the same group.
    pub fn insert(&mut self, tokens: &[Token], line: &str, term_hash_map: &mut IndexingTermmap) {
        let len = tokens.len();
        // 1) Try to find an existing matching group.
        if let Some(entry) = self
            .group_by_token_len
            .get_mut(len)
            // Always append to the first bucket with the same length.
            .and_then(|bucket| bucket.get_mut(0))
        {
            entry.push(tokens, line, term_hash_map);
        } else {
            // 2) Create a new group
            let id = self.get_group_id(len);
            let group = PrelimDocGroup::new(id, tokens, line, term_hash_map);
            self.insert_group(group);
        }
    }

    /// Total number of *groups*.
    #[inline]
    pub fn num_groups(&self) -> usize {
        self.group_by_token_len.iter().map(Vec::len).sum()
    }

    /// Immutable iterator over *(GroupId, &PrelimDocGroup)*.
    pub fn iter(&self) -> impl Iterator<Item = (GroupId, &PrelimDocGroup)> {
        self.group_by_token_len
            .iter()
            .flat_map(|bucket| bucket.iter().map(|group| (group.group_id.clone(), group)))
    }

    /// Mutable access via `GroupId` (linear scan in its bucket).
    #[inline]
    pub fn get_mut(&mut self, id: GroupId) -> Option<&mut PrelimDocGroup> {
        self.group_by_token_len
            .get_mut(id.num_tokens as usize)?
            .iter_mut()
            .find(|group| group.group_id == id)
    }

    /// Shared access via `GroupId`.
    #[inline]
    pub fn get(&self, id: GroupId) -> Option<&PrelimDocGroup> {
        self.group_by_token_len
            .get(id.num_tokens as usize)?
            .iter()
            .find(|group| group.group_id == id)
    }

    /// Removes and returns the group identified by `id`, if present.
    pub fn remove(&mut self, id: GroupId) -> Option<PrelimDocGroup> {
        if let Some(bucket) = self.group_by_token_len.get_mut(id.num_tokens as usize) {
            if let Some(pos) = bucket.iter().position(|group| group.group_id == id) {
                return Some(bucket.swap_remove(pos));
            }
        }
        None
    }

    /// Iterator over all groups.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &PrelimDocGroup> {
        self.group_by_token_len.iter().flat_map(|b| b.iter())
    }

    /// Mutable iterator over all groups.
    #[inline]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut PrelimDocGroup> {
        self.group_by_token_len
            .iter_mut()
            .flat_map(|b| b.iter_mut())
    }

    pub(crate) fn insert_group(&mut self, group: PrelimDocGroup) {
        let len = group.template.tokens.len();
        self.ensure_bucket(len);
        let id = GroupId {
            num_tokens: group.template.tokens.len() as u32,
            id: self.next_group_id,
        };
        self.next_group_id += 1;
        self.group_by_token_len[id.num_tokens as usize].push(group);
    }

    fn get_group_id(&mut self, num_tokens: usize) -> GroupId {
        self.ensure_bucket(num_tokens);
        let id = GroupId {
            num_tokens: num_tokens as u32,
            id: self.next_group_id,
        };
        self.next_group_id += 1;
        id
    }
}
