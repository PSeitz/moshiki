use std::iter::{Cloned, Filter};
use std::mem;

use super::{Addr, MemoryArena};
use crate::fastcpy::fast_short_slice_copy;

/// Returns the actual memory size in bytes required to create a table with a
/// given capacity when storing a value of type `V` inside the entry.
#[inline]
pub fn compute_table_memory_size<V: Copy + Default>(capacity: usize) -> usize {
    capacity * mem::size_of::<KeyValue<V>>()
}

#[cfg(not(feature = "compare_hash_only"))]
type HashType = u32;

#[cfg(feature = "compare_hash_only")]
type HashType = u64;

/// `KeyValue` is the item stored in the hash table.
///
/// * `key_addr` points into the [`MemoryArena`] where the key bytes live
///   (including the 2‑byte length prefix).
/// * `hash` caches the hash of the key so we never recompute it while probing.
/// * `value` is the **generic user value** stored directly in the table entry.
#[derive(Copy, Clone)]
struct KeyValue<V: Copy + Default> {
    key_addr: Addr,
    hash: HashType,
    value: V,
}

impl<V: Copy + Default> Default for KeyValue<V> {
    #[inline]
    fn default() -> Self {
        Self {
            key_addr: Addr::null_pointer(),
            hash: 0,
            value: V::default(),
        }
    }
}

impl<V: Copy + Default> KeyValue<V> {
    #[inline]
    fn is_empty(&self) -> bool {
        self.key_addr.is_null()
    }

    #[inline]
    fn is_not_empty_ref(&self) -> bool {
        !self.key_addr.is_null()
    }
}

/// A lightweight hash‑map specialised for `&[u8]` keys whose bytes live in a
/// shared [`MemoryArena`].  All values live directly in the table so look‑ups
/// only dereference the arena for the key (never for the value).
pub struct SharedArenaHashMap<V: Copy + Default> {
    table: Vec<KeyValue<V>>,
    mask: usize,
    len: usize,
}

struct LinearProbing {
    pos: usize,
    mask: usize,
}

impl LinearProbing {
    #[inline]
    fn compute(hash: HashType, mask: usize) -> LinearProbing {
        LinearProbing {
            pos: hash as usize,
            mask,
        }
    }

    #[inline]
    fn next_probe(&mut self) -> usize {
        // Not saving the masked version removes a dependency on AMD/Intel µarch.
        self.pos = self.pos.wrapping_add(1);
        self.pos & self.mask
    }
}

type IterNonEmpty<'a, V> =
    Filter<Cloned<std::slice::Iter<'a, KeyValue<V>>>, fn(&KeyValue<V>) -> bool>;

pub struct Iter<'a, V>
where
    V: Copy + Default,
{
    hashmap: &'a SharedArenaHashMap<V>,
    memory_arena: &'a MemoryArena,
    inner: IterNonEmpty<'a, V>,
}

impl<'a, V> Iterator for Iter<'a, V>
where
    V: Copy + Default,
{
    type Item = (&'a [u8], V);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|kv| {
            let key = self.hashmap.read_key_bytes(kv.key_addr, self.memory_arena);
            (key, kv.value)
        })
    }
}

/// Returns the greatest power‑of‑two ≤ `n` (n must be > 0).
#[inline]
fn compute_previous_power_of_two(n: usize) -> usize {
    assert!(n > 0);
    let msb = (63u32 - (n as u64).leading_zeros()) as u8;
    1usize << msb
}

impl<V: Copy + Default> Default for SharedArenaHashMap<V> {
    fn default() -> Self {
        Self::with_capacity(4)
    }
}

impl<V: Copy + Default> SharedArenaHashMap<V> {
    /// Create a new table able to hold at least `table_size` items before the
    /// first resize.  Capacity is rounded **down** to the nearest power‑of‑two
    /// (minimum 1).
    pub fn with_capacity(table_size: usize) -> Self {
        let table_size_pow2 = compute_previous_power_of_two(table_size.max(1));
        let table = vec![KeyValue::<V>::default(); table_size_pow2];
        Self {
            table,
            mask: table_size_pow2 - 1,
            len: 0,
        }
    }

    #[inline]
    #[cfg(not(feature = "compare_hash_only"))]
    fn hash_key(&self, key: &[u8]) -> HashType {
        murmurhash32::murmurhash2(key)
    }

    #[inline]
    #[cfg(feature = "compare_hash_only")]
    fn hash_key(&self, key: &[u8]) -> HashType {
        use std::hash::Hasher;
        let mut hasher = ahash::AHasher::default();
        hasher.write(key);
        hasher.finish() as HashType
    }

    /// Returns a linear‑probing cursor for the given hash value.
    #[inline]
    fn probe(&self, hash: HashType) -> LinearProbing {
        LinearProbing::compute(hash, self.mask)
    }

    /// True if we reached 50 % fill ratio.
    #[inline]
    fn is_saturated(&self) -> bool {
        self.table.len() <= self.len * 2
    }

    /// Read the key bytes back from the arena.
    #[inline]
    fn read_key_bytes<'a>(&self, addr: Addr, arena: &'a MemoryArena) -> &'a [u8] {
        let data = arena.slice_from(addr);
        let len = u16::from_le_bytes(data[..2].try_into().unwrap()) as usize;
        // SAFETY: we ensured len is in‑bounds when we wrote it.
        unsafe { data.get_unchecked(2..2 + len) }
    }

    #[inline]
    fn key_matches(&self, stored_addr: Addr, target: &[u8], arena: &MemoryArena) -> bool {
        #[cfg(not(feature = "compare_hash_only"))]
        {
            use crate::fastcmp::fast_short_slice_compare;
            let stored = self.read_key_bytes(stored_addr, arena);
            fast_short_slice_compare(stored, target)
        }
        #[cfg(feature = "compare_hash_only")]
        {
            // hash already matched, we assume no collisions.
            let _ = (stored_addr, target, arena); // silence unused warnings if feature is on
            true
        }
    }

    /// Grow table by ×2, re‑inserting every entry.
    fn resize(&mut self) {
        let new_len = (self.table.len() * 2).max(8);
        let new_mask = new_len - 1;
        let mut new_table = vec![KeyValue::<V>::default(); new_len];

        for kv in self
            .table
            .iter()
            .cloned()
            .filter(KeyValue::<V>::is_not_empty_ref)
        {
            let mut probe = LinearProbing::compute(kv.hash, new_mask);
            loop {
                let bucket = probe.next_probe();
                if new_table[bucket].is_empty() {
                    new_table[bucket] = kv;
                    break;
                }
            }
        }

        self.table = new_table;
        self.mask = new_mask;
    }

    /// Memory usage **in bytes** of the backing table (not counting the arena).
    #[inline]
    pub fn mem_usage(&self) -> usize {
        self.table.len() * mem::size_of::<KeyValue<V>>()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Iterate over all `(key, value)` pairs in arbitrary order.
    #[inline]
    pub fn iter<'a>(&'a self, arena: &'a MemoryArena) -> Iter<'a, V> {
        fn not_empty<K: Copy + Default>(kv: &KeyValue<K>) -> bool {
            kv.is_not_empty_ref()
        }

        Iter {
            inner: self.table.iter().cloned().filter(not_empty::<V>),
            hashmap: self,
            memory_arena: arena,
        }
    }

    /// Lookup the value associated with `key`.
    #[inline]
    pub fn get(&self, key: &[u8], arena: &MemoryArena) -> Option<V> {
        let hash = self.hash_key(key);
        let mut probe = self.probe(hash);
        loop {
            let bucket = probe.next_probe();
            let kv = self.table[bucket];
            if kv.is_empty() {
                return None;
            }
            if kv.hash == hash && self.key_matches(kv.key_addr, key, arena) {
                return Some(kv.value);
            }
        }
    }

    /// Insert or update the value for `key` using `updater`.
    ///
    /// * If `key` is not present, we allocate room for it in the arena and call
    ///   `updater(None)` to obtain the initial value.
    /// * If `key` exists, we call `updater(Some(old_value))` and store the
    ///   returned new value.
    ///
    /// Returns the freshly stored value (helpful for chaining).
    #[inline]
    pub fn mutate_or_create(
        &mut self,
        key: &[u8],
        arena: &mut MemoryArena,
        mut updater: impl FnMut(Option<V>) -> V,
    ) -> V {
        if self.is_saturated() {
            self.resize();
        }

        // Keys are capped to u16::MAX for compact length prefixing.
        let key = &key[..std::cmp::min(key.len(), u16::MAX as usize)];
        let hash = self.hash_key(key);
        let mut probe = self.probe(hash);
        let mut bucket = probe.next_probe();
        let mut kv = self.table[bucket];

        loop {
            if kv.is_empty() {
                let initial_val = updater(None);

                // 2 bytes length + key bytes.
                let num_bytes = 2 + key.len();
                let key_addr = arena.allocate_space(num_bytes);
                {
                    let data = arena.slice_mut(key_addr, num_bytes);
                    data[..2].copy_from_slice(&(key.len() as u16).to_le_bytes());
                    fast_short_slice_copy(key, &mut data[2..]);
                }

                self.table[bucket] = KeyValue {
                    key_addr,
                    hash,
                    value: initial_val,
                };
                self.len += 1;
                return initial_val;
            }
            if kv.hash == hash && self.key_matches(kv.key_addr, key, arena) {
                let new_val = updater(Some(kv.value));
                self.table[bucket].value = new_val;
                return new_val;
            }

            bucket = probe.next_probe();
            kv = self.table[bucket];
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{compute_previous_power_of_two, SharedArenaHashMap};
    use crate::MemoryArena;

    #[test]
    fn test_hash_map() {
        let mut arena = MemoryArena::default();
        let mut map: SharedArenaHashMap<u32> = SharedArenaHashMap::default();

        map.mutate_or_create(b"abc", &mut arena, |o| {
            assert_eq!(o, None);
            3
        });
        map.mutate_or_create(b"abcd", &mut arena, |o| {
            assert_eq!(o, None);
            4
        });
        map.mutate_or_create(b"abc", &mut arena, |o| {
            assert_eq!(o, Some(3));
            5
        });

        let mut vanilla = HashMap::new();
        for (k, v) in map.iter(&arena) {
            vanilla.insert(k.to_vec(), v);
        }
        assert_eq!(vanilla.len(), 2);
    }

    #[test]
    fn test_long_key_truncation() {
        let mut arena = MemoryArena::default();
        let mut map: SharedArenaHashMap<u32> = SharedArenaHashMap::default();

        let key1: Vec<u8> = (0..u16::MAX as usize).map(|i| i as u8).collect();
        map.mutate_or_create(&key1, &mut arena, |o| {
            assert_eq!(o, None);
            4
        });

        // Due to truncation, this is the *same* logical key.
        let key2: Vec<u8> = (0..u16::MAX as usize + 1).map(|i| i as u8).collect();
        map.mutate_or_create(&key2, &mut arena, |o| {
            assert_eq!(o, Some(4));
            3
        });

        let mut vanilla = HashMap::new();
        for (k, v) in map.iter(&arena) {
            vanilla.insert(k.to_vec(), v);
            assert_eq!(k.len(), key1.len());
            assert_eq!(k, &key1[..]);
        }
        assert_eq!(vanilla.len(), 1);
    }

    #[test]
    fn test_empty_hashmap() {
        let arena = MemoryArena::default();
        let map: SharedArenaHashMap<u32> = SharedArenaHashMap::default();
        assert_eq!(map.get(b"abc", &arena), None);
    }

    #[test]
    fn test_compute_previous_power_of_two() {
        assert_eq!(compute_previous_power_of_two(8), 8);
        assert_eq!(compute_previous_power_of_two(9), 8);
        assert_eq!(compute_previous_power_of_two(7), 4);
        assert_eq!(compute_previous_power_of_two(u64::MAX as usize), 1 << 63);
    }

    #[test]
    fn test_many_terms() {
        let mut arena = MemoryArena::default();
        let mut terms: Vec<String> = (0..20_000).map(|v| v.to_string()).collect();
        let mut map: SharedArenaHashMap<u32> = SharedArenaHashMap::default();

        for t in &terms {
            map.mutate_or_create(t.as_bytes(), &mut arena, |_| 5);
        }

        let mut roundtrip: Vec<String> = map
            .iter(&arena)
            .map(|(bytes, _)| String::from_utf8(bytes.to_vec()).unwrap())
            .collect();
        roundtrip.sort();
        terms.sort();

        assert_eq!(roundtrip, terms);
    }
}
