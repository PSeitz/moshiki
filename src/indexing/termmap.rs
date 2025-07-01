use stacker::ArenaHashMap;

#[derive(Default)]
pub struct IndexingTermmap {
    term_hash_map: ArenaHashMap,
    unique_term_hash_map: Vec<(Vec<u8>, u32)>,
    next_term_id: u32,
}
impl IndexingTermmap {
    pub(crate) fn len(&self) -> usize {
        self.term_hash_map.len() + self.unique_term_hash_map.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&[u8], u32)> {
        self.term_hash_map
            .iter()
            .map(|(term_bytes, old_id_addr)| {
                let old_id: u32 = self.term_hash_map.read(old_id_addr);
                (term_bytes, old_id)
            })
            .chain(
                self.unique_term_hash_map
                    .iter()
                    .map(|(term_bytes, term_id)| (term_bytes.as_slice(), *term_id)),
            )
    }

    #[inline]
    pub fn mutate_or_create(&mut self, key: &str, is_unique: bool) -> u32 {
        if is_unique {
            let term_id = self.next_term_id;
            self.unique_term_hash_map
                .push((key.as_bytes().to_vec(), term_id));
            self.next_term_id += 1;
            return term_id;
        }
        let mut term_id = 0;
        self.term_hash_map.mutate_or_create(key.as_bytes(), |opt| {
            if let Some(existing) = opt {
                term_id = existing;
                term_id
            } else {
                term_id = self.next_term_id;
                self.next_term_id += 1;
                term_id
            }
        });
        term_id
    }
}
