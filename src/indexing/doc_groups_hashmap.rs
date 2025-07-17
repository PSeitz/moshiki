use fxhash::FxHashMap;

use crate::{
    Token,
    indexing::{PrelimDocGroup, fingerprint, termmap::IndexingTermmap},
};

/// Group identifier *equal to the fingerprint* of the token-type sequence.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroupId(pub u64);

/// All document groups kept in a single hash-map bucket.
#[derive(Debug, Default, Clone)]
pub struct DocGroups {
    groups: FxHashMap<GroupId, PrelimDocGroup>,
}

impl DocGroups {
    /// Creates an empty collection.
    #[inline]
    pub fn new() -> Self {
        Self {
            groups: FxHashMap::default(),
        }
    }

    /// Inserts a document.
    ///
    /// * Every distinct **fingerprint** gets its own group.
    pub fn insert(&mut self, tokens: &[Token], line: &str, term_hash_map: &mut IndexingTermmap) {
        let id = GroupId(fingerprint(tokens));

        match self.groups.get_mut(&id) {
            Some(entry) => {
                entry.push(tokens, line, term_hash_map);
            }
            None => {
                let group = PrelimDocGroup::new(id, tokens, line, term_hash_map);
                self.groups.insert(id, group);
            }
        }
    }

    /// Inserts a document.
    ///
    pub fn insert_group(&mut self, group: PrelimDocGroup) {
        self.groups.insert(group.group_id, group);
    }

    /// Total number of groups.
    #[inline]
    pub fn num_groups(&self) -> usize {
        self.groups.len()
    }

    /// Immutable iterator over *(GroupId, &PrelimDocGroup)*.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (GroupId, &PrelimDocGroup)> {
        self.groups.values().map(|e| (e.group_id, e))
    }

    /// Mutable access via `GroupId`.
    #[inline]
    pub fn get_mut(&mut self, id: GroupId) -> Option<&mut PrelimDocGroup> {
        self.groups.get_mut(&id)
    }

    /// Shared access via `GroupId`.
    #[inline]
    pub fn get(&self, id: GroupId) -> Option<&PrelimDocGroup> {
        self.groups.get(&id)
    }

    /// Removes and returns the group identified by `id`, if present.
    #[inline]
    pub fn remove(&mut self, id: GroupId) -> Option<PrelimDocGroup> {
        self.groups.remove(&id)
    }

    /// Iterator over all groups.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &PrelimDocGroup> {
        self.groups.values()
    }

    /// Mutable iterator over all groups.
    #[inline]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut PrelimDocGroup> {
        self.groups.values_mut()
    }
}
