use stacker::ArenaHashMap;
use std::iter;

pub trait TermStore {
    fn len(&self) -> usize;
    fn iter(&self) -> Box<dyn Iterator<Item = (&[u8], u32)> + '_>;
}

impl<T: TermStore + ?Sized> TermStore for &T {
    fn len(&self) -> usize {
        (**self).len()
    }
    fn iter(&self) -> Box<dyn Iterator<Item = (&[u8], u32)> + '_> {
        (**self).iter()
    }
}

#[derive(Default)]
pub struct RegularTermMap {
    map: ArenaHashMap,
    unique: Vec<u8>,
    next: u32,
}

impl RegularTermMap {
    fn push_unique(&mut self, bytes: &[u8], id: u32) {
        let len = bytes.len() as u32;
        self.unique.extend_from_slice(&len.to_le_bytes());
        self.unique.extend_from_slice(bytes);
        self.unique.extend_from_slice(&id.to_le_bytes());
    }

    /// Iterator over the flat buffer — kept `pub(crate)` for the tests.
    pub(crate) fn iter_unique(&self) -> impl Iterator<Item = (&[u8], u32)> {
        let buf = &self.unique;
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
    fn mutate_or_create(&mut self, key: &[u8], is_id_like: bool) -> u32 {
        if is_id_like {
            let id = self.next;
            self.next += 1;
            self.push_unique(key, id);
            return id;
        }

        let mut id = 0;
        self.map.mutate_or_create(key, |opt| {
            id = opt.unwrap_or_else(|| {
                let new_id = self.next;
                self.next += 1;
                new_id
            });
            id
        });
        id
    }
}

impl TermStore for RegularTermMap {
    fn len(&self) -> usize {
        self.map.len() + self.iter_unique().count()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&[u8], u32)> + '_> {
        Box::new(
            self.map
                .iter()
                .map(|(bytes, addr)| (bytes, self.map.read(addr)))
                .chain(self.iter_unique()),
        )
    }
}

/* ----------------------- catch-all map (hash-only) ----------------------- */

#[derive(Default)]
pub struct CatchAllTermMap {
    map: ArenaHashMap,
    next: u32,
}
impl CatchAllTermMap {
    fn mutate_or_create(&mut self, key: &[u8]) -> u32 {
        let mut id = 0;
        self.map.mutate_or_create(key, |opt| {
            id = opt.unwrap_or_else(|| {
                let new_id = self.next;
                self.next += 1;
                new_id
            });
            id
        });
        id
    }
}

impl TermStore for CatchAllTermMap {
    fn len(&self) -> usize {
        self.map.len()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&[u8], u32)> + '_> {
        Box::new(
            self.map
                .iter()
                .map(|(bytes, addr)| (bytes, self.map.read(addr))),
        )
    }
}

/* --------------------------- public façade --------------------------- */

#[derive(Default)]
pub struct IndexingTermmap {
    pub regular: RegularTermMap,
    pub catch_all: CatchAllTermMap,
}

impl IndexingTermmap {
    pub fn mutate_or_create(&mut self, key: &[u8], is_id_like: bool, is_catch_all: bool) -> u32 {
        if is_catch_all {
            self.catch_all.mutate_or_create(key)
        } else {
            self.regular.mutate_or_create(key, is_id_like)
        }
    }
}

/* ------------------------------ tests ------------------------------ */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_serialization_and_iteration() {
        let mut map = IndexingTermmap::default();

        let id1 = map.mutate_or_create(b"abc", true, false);
        let id2 = map.mutate_or_create(b"defg", true, false);
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);

        // internal check via pub(crate) helper
        let collected: Vec<(&[u8], u32)> = map.regular.iter_unique().collect();
        assert_eq!(collected, vec![(b"abc".as_ref(), 0), (b"defg".as_ref(), 1)]);
    }

    #[test]
    fn test_len_counts_all() {
        let mut map = IndexingTermmap::default();
        map.mutate_or_create(b"aaa", false, false); // regular
        map.mutate_or_create(b"bbb", true, false); // id-like
        map.mutate_or_create(b"catch", false, true); // catch-all

        assert_eq!(map.regular.len(), 2); // 1 regular + 1 unique
        assert_eq!(map.catch_all.len(), 1);
    }
}
