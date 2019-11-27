use std::io;
use std::iter::Iterator;
use std::mem;

use crate::{
    Compound, Content, Handle, HandleMut, HandleOwned, HandleRef, HandleType,
    Method, Sink, Source, ValPath, ValPathMut, ValRef, ValRefMut,
};
use bytehash::ByteHash;
use seahash::SeaHasher;
use std::hash::{Hash, Hasher};

const N_BUCKETS: usize = 16;

/// A hash array mapped trie
#[derive(Clone, PartialEq, Eq)]
pub struct HAMT<L, H: ByteHash>([Handle<Self, H>; N_BUCKETS])
where
    Self: Compound<H>;

impl<L: Content<H>, H: ByteHash> Default for HAMT<L, H> {
    fn default() -> Self {
        HAMT(Default::default())
    }
}

#[inline(always)]
fn hash<T: Hash>(t: T) -> u64 {
    let mut hasher = SeaHasher::new();
    t.hash(&mut hasher);
    hasher.finish()
}

#[inline(always)]
fn calculate_slot(mut h: u64, mut depth: usize) -> usize {
    debug_assert!(N_BUCKETS == 16);
    while depth > 15 {
        h = hash(h);
        depth -= 16;
    }
    let shifted = h >> (depth * 4);
    (shifted & 0x0f) as usize
}

#[derive(Clone)]
pub struct HAMTSearch {
    hash: u64,
    depth: usize,
}

impl<T> From<&T> for HAMTSearch
where
    T: Hash,
{
    fn from(t: &T) -> Self {
        HAMTSearch {
            hash: hash(t),
            depth: 0,
        }
    }
}

impl Method for HAMTSearch {
    fn select<C, H>(&mut self, _: &[Handle<C, H>]) -> Option<usize>
    where
        C: Compound<H>,
        H: ByteHash,
    {
        let slot = calculate_slot(self.hash, self.depth);
        self.depth += 1;
        Some(slot)
    }
}

enum Removed<L> {
    None,
    Leaf(L),
    Collapse(L, L),
}

impl<K, V, H> HAMT<(K, V), H>
where
    K: Content<H> + Hash,
    V: Content<H>,
    H: ByteHash,
{
    /// Creates a new HAMT
    pub fn new() -> Self {
        HAMT(Default::default())
    }

    /// Insert key-value pair into the HAMT, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> io::Result<Option<V>> {
        self.sub_insert(0, hash(&k), k, v)
    }

    fn sub_insert(
        &mut self,
        depth: usize,
        h: u64,
        k: K,
        v: V,
    ) -> io::Result<Option<V>> {
        let s = calculate_slot(h, depth);
        let slot = &mut self.0[s];

        Ok(match slot.inner_mut()? {
            HandleMut::None => {
                slot.replace(HandleOwned::Leaf((k, v)));
                None
            }
            HandleMut::Leaf((ref old_k, ref mut old_v)) => {
                if old_k == &k {
                    Some(mem::replace(old_v, v))
                } else {
                    let (old_k, old_v) = slot
                        .replace(HandleOwned::None)
                        .expect("Replace should return leaf");

                    let old_h = hash(&old_k);

                    let mut new_node = HAMT::new();
                    new_node.sub_insert(depth + 1, h, k, v)?;
                    new_node.sub_insert(depth + 1, old_h, old_k, old_v)?;
                    slot.replace(HandleOwned::Node(new_node));
                    None
                }
            }
            HandleMut::Node(node) => node.sub_insert(depth + 1, h, k, v)?,
        })
    }

    /// Returns a reference to a value in the map, if any
    pub fn get(&self, k: &K) -> io::Result<Option<impl ValRef<V>>> {
        ValPath::new(self, &mut HAMTSearch::from(&k), k)
    }

    /// Returns a reference to a mutable value in the map, if any
    pub fn get_mut<'a>(
        &'a mut self,
        k: &K,
    ) -> io::Result<Option<impl ValRefMut<V>>> {
        ValPathMut::new(self, &mut HAMTSearch::from(&k), k)
    }

    /// Remove element with given key, returning it.
    pub fn remove(&mut self, k: &K) -> io::Result<Option<V>> {
        match self.sub_remove(0, hash(&k), k)? {
            Removed::None => Ok(None),
            Removed::Leaf((_, v)) => Ok(Some(v)),
            _ => unreachable!(),
        }
    }

    fn sub_remove(
        &mut self,
        depth: usize,
        h: u64,
        k: &K,
    ) -> io::Result<Removed<(K, V)>> {
        let s = calculate_slot(h, depth);
        let slot = &mut self.0[s];

        let mut collapse = None;

        match slot.inner_mut()? {
            HandleMut::None => return Ok(Removed::None),
            HandleMut::Leaf((place_k, _)) => {
                if place_k == k {
                    // pass on control flow
                } else {
                    return Ok(Removed::None);
                }
            }
            HandleMut::Node(node) => match node.sub_remove(depth + 1, h, k)? {
                Removed::Collapse(removed, reinsert) => {
                    collapse = Some((removed, reinsert));
                }
                a => return Ok(a),
            },
        };

        let removed_leaf;

        // lower level collapsed
        if let Some((removed, reinsert)) = collapse {
            removed_leaf = removed;
            slot.replace(HandleOwned::Leaf(reinsert));
        } else {
            removed_leaf = slot
                .replace(HandleOwned::None)
                .expect("leaf checked in previous match");
        }

        // at depth > 0, we might have to collapse the branch
        if depth > 0 {
            match self.remove_singleton()? {
                Some(kv) => Ok(Removed::Collapse(removed_leaf, kv)),
                None => Ok(Removed::Leaf(removed_leaf)),
            }
        } else {
            Ok(Removed::Leaf(removed_leaf))
        }
    }

    fn remove_singleton(&mut self) -> io::Result<Option<(K, V)>> {
        let mut singleton = None;

        for (i, child) in self.0.iter().enumerate() {
            match (child.inner()?, singleton) {
                (HandleRef::None, _) => (),
                (HandleRef::Leaf(_), None) => singleton = Some(i),
                (HandleRef::Leaf(_), Some(_)) => return Ok(None),
                (HandleRef::Node(_), _) => return Ok(None),
            }
        }
        if let Some(idx) = singleton {
            Ok(self.0[idx].replace(HandleOwned::None))
        } else {
            Ok(None)
        }
    }

    #[cfg(test)]
    #[doc(hidden)]
    pub fn assert_correct_empty_state(&self) {
        for child in self.0.iter() {
            match child.handle_type() {
                HandleType::None => (),
                _ => panic!("invalid empty state"),
            }
        }
    }
}

impl<L, H> Content<H> for HAMT<L, H>
where
    L: Content<H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        debug_assert!(N_BUCKETS == 16);
        let mut mask = 0u16;
        for i in 0..N_BUCKETS {
            if let HandleType::None = self.0[i].handle_type() {
                // no-op
            } else {
                mask |= 1 << i;
            }
        }

        <u16 as Content<H>>::persist(&mut mask, sink)?;

        for (i, handle) in self.0.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                handle.persist(sink)?
            }
        }
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut bucket: [Handle<Self, H>; N_BUCKETS] = Default::default();
        let mask = <u16 as Content<H>>::restore(source)?;
        for (i, handle) in bucket.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                *handle = Handle::restore(source)?
            }
        }
        Ok(HAMT(bucket))
    }
}

impl<L, H> Compound<H> for HAMT<L, H>
where
    H: ByteHash,
    L: Content<H>,
{
    type Leaf = L;

    fn children_mut(&mut self) -> &mut [Handle<Self, H>] {
        &mut self.0
    }

    fn children(&self) -> &[Handle<Self, H>] {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::quickcheck_map;
    use bytehash::Blake2b;

    #[test]
    fn trivial_map() {
        let mut h = HAMT::<_, Blake2b>::new();
        h.insert(28, 28).unwrap();
        assert_eq!(*h.get(&28).unwrap().unwrap(), 28);
    }

    #[test]
    fn bigger_map() {
        let mut h = HAMT::<_, Blake2b>::new();
        for i in 0..1000 {
            h.insert(i, i).unwrap();
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn nested_maps() {
        let mut map_a = HAMT::<_, Blake2b>::new();
        for i in 0..100 {
            let mut map_b = HAMT::<_, Blake2b>::new();

            for o in 0..100 {
                map_b.insert(o, o).unwrap();
            }

            map_a.insert(i, map_b).unwrap();
        }

        for i in 0..100 {
            let map_b = map_a.get(&i).unwrap().unwrap();

            for o in 0..100 {
                assert_eq!(*map_b.get(&o).unwrap().unwrap(), o);
            }
        }
    }

    quickcheck_map!(|| HAMT::new());
}
