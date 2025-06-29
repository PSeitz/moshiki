use stacker::ArenaHashMap;

#[derive(Default)]
pub struct IndexingTermmap {
    term_hash_map: ArenaHashMap,
}
impl IndexingTermmap {
    pub(crate) fn len(&self) -> usize {
        self.term_hash_map.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&[u8], u32)> {
        self.term_hash_map.iter().map(|(term_bytes, old_id_addr)| {
            let old_id: u32 = self.term_hash_map.read(old_id_addr);
            (term_bytes, old_id)
        })
    }

    #[inline]
    pub fn mutate_or_create<V>(&mut self, key: &[u8], updater: impl FnMut(Option<V>) -> V)
    where
        V: Copy + 'static,
    {
        self.term_hash_map.mutate_or_create(key, updater)
    }
}
