use fxhash::FxHashMap;

use crate::{
    Token,
    indexing::{
        DocGroup, fingerprint::fingerprint_types, fingerprint_tokens, termmap::IndexingTermmap,
    },
};

pub(crate) type Fingerprint = u64;

/// All document groups kept in a single hash-map bucket.
#[derive(Debug, Default, Clone)]
pub struct DocGroups {
    groups: FxHashMap<Fingerprint, DocGroup>,
    /// Only used to insert duplicate groups with the same fingerprint.
    next_group_salt: u32,
}

impl DocGroups {
    #[must_use]
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    /// Inserts a document.
    ///
    /// * Every distinct **fingerprint** gets its own group.
    pub fn insert(&mut self, tokens: &[Token], line: &str, term_hash_map: &mut IndexingTermmap) {
        let id = fingerprint_tokens(tokens);

        match self.groups.get_mut(&id) {
            Some(entry) => {
                entry.push(tokens, line, term_hash_map);
            }
            None => {
                let group = DocGroup::new(tokens, line);
                self.groups.insert(id, group);
            }
        }
    }

    /// Inserts a different group with the same fingerprint.
    ///
    /// That means the original fingerprint is changed.
    ///
    pub fn insert_duplicate(&mut self, group: DocGroup) {
        // Increment the salt to ensure that groups with the same fingerprint
        // but different lines are not merged.
        self.next_group_salt = self.next_group_salt.wrapping_add(1);

        let template_tokens = group
            .template
            .tokens
            .iter()
            .map(|token| token.token.clone());
        let id = fingerprint_types(template_tokens) + self.next_group_salt as u64;

        self.groups.insert(id, group);
    }

    /// Immutable iterator over *(GroupId, &PrelimDocGroup)*.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (Fingerprint, &DocGroup)> {
        self.groups.iter().map(|(id, group)| (*id, group))
    }

    /// Mutable access via `GroupId`.
    #[inline]
    pub fn get_mut(&mut self, id: Fingerprint) -> Option<&mut DocGroup> {
        self.groups.get_mut(&id)
    }

    /// Shared access via `GroupId`.
    #[inline]
    pub fn get(&self, id: Fingerprint) -> Option<&DocGroup> {
        self.groups.get(&id)
    }

    /// Removes and returns the group identified by `id`, if present.
    #[inline]
    pub fn remove(&mut self, id: Fingerprint) -> Option<DocGroup> {
        self.groups.remove(&id)
    }

    /// Iterator over all groups.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &DocGroup> {
        self.groups.values()
    }

    /// Mutable iterator over all groups.
    #[inline]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut DocGroup> {
        self.groups.values_mut()
    }
}
