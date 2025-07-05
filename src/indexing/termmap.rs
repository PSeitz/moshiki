use stacker::ArenaHashMap;

/// A structure that stores term → id mappings used by the indexer.  
///
/// For *id‑like* terms (usually primary‑key values) we skip the hash map
/// and append them to a contiguous buffer where each entry is encoded
/// as `[u32 len | bytes | u32 id]`, all little‑endian.
///
#[derive(Default)]
pub struct IndexingTermmap {
    term_hash_map: ArenaHashMap,
    catch_all_term_hash_map: ArenaHashMap,
    /// Flat buffer that holds repeated (len, bytes, id) tuples.
    unique_term_hash_map: Vec<u8>,
    next_term_id: u32,
    next_term_id_catch_all: u32,
}

/// Iterator over the flattened `unique_term_hash_map` buffer.
pub struct UniqueTermIter<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for UniqueTermIter<'a> {
    type Item = (&'a [u8], u32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.buf.len() {
            return None;
        }
        if self.pos + 4 > self.buf.len() {
            return None; // malformed
        }
        let len = u32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap()) as usize;
        self.pos += 4;
        if self.pos + len + 4 > self.buf.len() {
            return None; // malformed
        }
        let bytes = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        let term_id = u32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Some((bytes, term_id))
    }
}

impl IndexingTermmap {
    pub(crate) fn catch_all_len(&self) -> usize {
        self.catch_all_term_hash_map.len()
    }

    /// Counts the number of `(bytes, id)` pairs in the flat buffer.
    fn unique_term_count(&self) -> usize {
        UniqueTermIter {
            buf: &self.unique_term_hash_map,
            pos: 0,
        }
        .count()
    }

    pub(crate) fn len(&self) -> usize {
        self.term_hash_map.len() + self.unique_term_count()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&[u8], u32)> {
        self.term_hash_map
            .iter()
            .map(|(term_bytes, old_id_addr)| {
                let old_id: u32 = self.term_hash_map.read(old_id_addr);
                (term_bytes, old_id)
            })
            .chain(self.iter_unique())
    }

    /// Returns an iterator over the unique‑term buffer.
    pub(crate) fn iter_unique(&self) -> UniqueTermIter<'_> {
        UniqueTermIter {
            buf: &self.unique_term_hash_map,
            pos: 0,
        }
    }

    pub(crate) fn iter_catch_all(&self) -> impl Iterator<Item = (&[u8], u32)> {
        self.catch_all_term_hash_map
            .iter()
            .map(|(term_bytes, old_id_addr)| {
                let old_id: u32 = self.catch_all_term_hash_map.read(old_id_addr);
                (term_bytes, old_id)
            })
    }

    /// Insert the `key` if necessary and return its id.
    #[inline]
    pub fn mutate_or_create(&mut self, key: &str, is_id_like: bool, is_catch_all: bool) -> u32 {
        if is_catch_all {
            let mut term_id = 0;
            self.catch_all_term_hash_map
                .mutate_or_create(key.as_bytes(), |opt| {
                    if let Some(existing) = opt {
                        term_id = existing;
                        term_id
                    } else {
                        term_id = self.next_term_id_catch_all;
                        self.next_term_id_catch_all += 1;
                        term_id
                    }
                });
            return term_id;
        }

        if is_id_like {
            let term_id = self.next_term_id;
            self.next_term_id += 1;
            self.push_unique(key.as_bytes(), term_id);
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

    /// Append an `(bytes, id)` entry to `unique_term_hash_map`.
    fn push_unique(&mut self, bytes: &[u8], term_id: u32) {
        let len = bytes.len() as u32;
        self.unique_term_hash_map
            .extend_from_slice(&len.to_le_bytes());
        self.unique_term_hash_map.extend_from_slice(bytes);
        self.unique_term_hash_map
            .extend_from_slice(&term_id.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_serialization_and_iteration() {
        let mut map = IndexingTermmap::default();

        let id1 = map.mutate_or_create("abc", true, false);
        let id2 = map.mutate_or_create("defg", true, false);
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);

        let collected: Vec<(&[u8], u32)> = map.iter_unique().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].0, b"abc");
        assert_eq!(collected[0].1, 0);
        assert_eq!(collected[1].0, b"defg");
        assert_eq!(collected[1].1, 1);
    }

    #[test]
    fn test_len_counts_all() {
        let mut map = IndexingTermmap::default();
        map.mutate_or_create("aaa", false, false); // normal term
        map.mutate_or_create("bbb", true, false); // unique/id‑like term
        map.mutate_or_create("catch", false, true); // catch‑all term

        assert_eq!(map.len(), 2); // 1 normal + 1 unique
        assert_eq!(map.catch_all_len(), 1);
    }
}
