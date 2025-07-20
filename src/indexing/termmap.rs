use stacker::ArenaHashMap;
use std::iter;

pub trait TermStore {
    fn num_terms(&self) -> usize;
    fn iter(&self) -> Box<dyn Iterator<Item = (&[u8], u32)> + '_>;
}

impl<T: TermStore + ?Sized> TermStore for &T {
    fn num_terms(&self) -> usize {
        (**self).num_terms()
    }
    fn iter(&self) -> Box<dyn Iterator<Item = (&[u8], u32)> + '_> {
        (**self).iter()
    }
}

pub struct RegularTermMap {
    map: ArenaHashMap<u32>,
    unique_terms: Vec<u8>,
    next_term_id: u32,
}
impl Default for RegularTermMap {
    fn default() -> Self {
        RegularTermMap {
            map: ArenaHashMap::default(),
            unique_terms: Vec::with_capacity(1024 * 1024), // 1 MiB initial capacity
            next_term_id: 0,
        }
    }
}

impl RegularTermMap {
    #[inline]
    fn push_unique(&mut self, bytes: &[u8], id: u32) {
        let len = bytes.len() as u32;
        self.unique_terms.extend_from_slice(&len.to_le_bytes());
        self.unique_terms.extend_from_slice(bytes);
        self.unique_terms.extend_from_slice(&id.to_le_bytes());
    }

    /// Iterator over the flat buffer — kept `pub(crate)` for the tests.
    pub(crate) fn iter_unique(&self) -> impl Iterator<Item = (&[u8], u32)> {
        let buf = &self.unique_terms;
        let mut pos = 0usize;

        iter::from_fn(move || {
            if pos + 4 > buf.len() {
                return None;
            }
            let len = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;
            if pos + len + 4 > buf.len() {
                return None;
            }
            let bytes = &buf[pos..pos + len];
            pos += len;
            let id = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
            pos += 4;
            Some((bytes, id))
        })
    }
}
impl RegularTermMap {
    #[inline]
    fn mutate_or_create(&mut self, key: &[u8], is_id_like: bool) -> u32 {
        if is_id_like {
            let id = self.next_term_id;
            self.push_unique(key, id);
            self.next_term_id += 1;
            return id;
        }

        let mut id = 0;
        self.map.mutate_or_create(key, |opt| {
            id = opt.unwrap_or_else(|| {
                let new_id = self.next_term_id;
                self.next_term_id += 1;
                new_id
            });
            id
        });
        id
    }

    #[inline]
    /// This is VERY expensive, so use it only when necessary.
    /// We scan the dict.
    pub fn find_term_for_term_id(&self, term_id: u32) -> &[u8] {
        self.map
            .iter()
            .find_map(|(bytes, id)| if id == term_id { Some(bytes) } else { None })
            .unwrap_or_else(|| {
                // If not found, we check the unique buffer
                self.iter_unique()
                    .find_map(|(bytes, id)| if id == term_id { Some(bytes) } else { None })
                    .expect("Term ID not found in either map")
            })
    }
}

impl TermStore for RegularTermMap {
    fn num_terms(&self) -> usize {
        self.next_term_id as usize
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&[u8], u32)> + '_> {
        Box::new(self.map.iter().chain(self.iter_unique()))
    }
}

#[derive(Default)]
pub struct IndexingTermmap {
    pub regular: RegularTermMap,
}

impl IndexingTermmap {
    #[inline]
    pub fn mutate_or_create(&mut self, key: &[u8], is_id_like: bool) -> u32 {
        self.regular.mutate_or_create(key, is_id_like)
    }

    #[inline]
    /// This is VERY expensive, so use it only when necessary.
    /// We scan the dict.
    ///
    /// ONLY scans the regular map.
    pub fn find_term_for_term_id(&self, term_id: u32) -> &[u8] {
        self.regular.find_term_for_term_id(term_id)
    }
}

/* ------------------------------ tests ------------------------------ */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_serialization_and_iteration() {
        let mut map = IndexingTermmap::default();

        let id1 = map.mutate_or_create(b"abc", true);
        let id2 = map.mutate_or_create(b"defg", true);
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);

        // internal check via pub(crate) helper
        let collected: Vec<(&[u8], u32)> = map.regular.iter_unique().collect();
        assert_eq!(collected, vec![(b"abc".as_ref(), 0), (b"defg".as_ref(), 1)]);
    }

    #[test]
    fn test_len_counts_all() {
        let mut map = IndexingTermmap::default();
        map.mutate_or_create(b"aaa", false); // regular
        map.mutate_or_create(b"bbb", true); // id-like

        assert_eq!(map.regular.num_terms(), 2); // 1 regular + 1 unique
    }
}
