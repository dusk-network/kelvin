//! A Hash-array mapped trie implemented on kelvin
#![warn(missing_docs)]

use std::borrow::Borrow;
use std::hash::Hash;
use std::io;
use std::iter::Iterator;
use std::marker::PhantomData;
use std::mem;

use kelvin::{
    annotations::{Annotation, Cardinality, VoidAnnotation},
    ByteHash, Compound, Content, Handle, HandleMut, HandleRef, HandleType,
    Method, SearchResult, Sink, Source, ValPath, ValPathMut, KV,
};

/// Default HAMT-map without annotations
pub type DefaultHAMTMap<K, V, H> = HAMT<K, V, VoidAnnotation, H>;
/// Default HAMT-map with Cardinality annotation (for `.count()`)
pub type CountingHAMTMap<K, V, H> = HAMT<K, V, Cardinality<u64>, H>;

/// A hash array mapped trie with branching factor of 16
#[derive(Clone)]
pub struct HAMT<K, V, A, H>([Handle<Self, H>; 16])
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash;

impl<K, V, A, H> Default for HAMT<K, V, A, H>
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    fn default() -> Self {
        HAMT(Default::default())
    }
}

/// A hash array mapped trie with branching factor of 4
#[derive(Clone)]
pub struct NarrowHAMT<K, V, A, H>([Handle<Self, H>; 4])
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash;

impl<K, V, A, H> Default for NarrowHAMT<K, V, A, H>
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    fn default() -> Self {
        NarrowHAMT(Default::default())
    }
}

// Trait to implement child selection based on an input byte path
// public but hidden, since it needs to be part of the Method impl
#[doc(hidden)]
pub trait SlotSelect {
    fn select_slot(hash: &[u8], depth: usize) -> usize;
}

// Trait to abstract over the width of the HAMT
trait HAMTTrait<K, V, H>: Compound<H> + Default + SlotSelect
where
    H: ByteHash,
    K: Eq + Hash,
    Self::Leaf: Borrow<KV<K, V>> + From<KV<K, V>> + Into<KV<K, V>>,
{
    fn sub_insert(
        &mut self,
        depth: usize,
        h: H::Digest,
        k: K,
        v: V,
    ) -> io::Result<Option<V>> {
        let s = Self::select_slot(h.as_ref(), depth);

        enum Action {
            Split,
            Insert,
            Replace,
        }

        let action = match self.children_mut()[s].inner_mut()? {
            HandleMut::None(_) => Action::Insert,
            HandleMut::Leaf(ref mut leaf) => {
                let KV { ref key, val: _ } = (**leaf).borrow();

                if key == &k {
                    Action::Replace
                } else {
                    Action::Split
                }
            }
            HandleMut::Node(ref mut node) => {
                return node.sub_insert(depth + 1, h, k, v)
            }
        };

        Ok(match action {
            Action::Insert => {
                self.children_mut()[s] = Handle::new_leaf(KV::new(k, v).into());
                None
            }
            Action::Replace => {
                let KV { key: _, val } = mem::replace(
                    &mut self.children_mut()[s],
                    Handle::new_leaf(KV::new(k, v).into()),
                )
                .into_leaf()
                .into();

                Some(val)
            }
            Action::Split => {
                let KV { key, val } = mem::replace(
                    &mut self.children_mut()[s],
                    Handle::new_empty(),
                )
                .into_leaf()
                .into();

                let old_h = H::hash(&key);

                let mut new_node = Self::default();
                new_node.sub_insert(depth + 1, h, k, v)?;
                new_node.sub_insert(depth + 1, old_h, key, val)?;
                self.children_mut()[s] = Handle::new_node(new_node);
                None
            }
        })
    }

    fn sub_remove<O>(
        &mut self,
        depth: usize,
        h: H::Digest,
        k: &O,
    ) -> io::Result<Removed<KV<K, V>>>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        let removed_leaf;
        {
            let s = Self::select_slot(h.as_ref(), depth);
            let slot = &mut self.children_mut()[s];

            match slot.inner_mut()? {
                HandleMut::None(_) => return Ok(Removed::None),
                HandleMut::Leaf(ref mut leaf) => {
                    let KV { key, val: _ } = (**leaf).borrow();
                    if key.borrow() != k {
                        return Ok(Removed::None);
                    } else {
                        removed_leaf = leaf.replace(Handle::new_empty());
                    }
                }
                HandleMut::Node(ref mut node) => {
                    match node.sub_remove(depth + 1, h, k)? {
                        Removed::Collapse(removed, reinsert) => {
                            removed_leaf = removed.into();
                            node.replace(Handle::new_leaf(reinsert.into()));
                        }
                        a => {
                            return Ok(a);
                        }
                    }
                }
            };
        }
        // we might have to collapse the branch
        if depth > 0 {
            match self.remove_singleton()? {
                Some(kv) => Ok(Removed::Collapse(removed_leaf.into(), kv)),
                None => Ok(Removed::Leaf(removed_leaf.into())),
            }
        } else {
            Ok(Removed::Leaf(removed_leaf.into()))
        }
    }

    fn remove_singleton(&mut self) -> io::Result<Option<KV<K, V>>> {
        let mut singleton = None;

        for (i, child) in self.children().iter().enumerate() {
            match (child.inner()?, singleton) {
                (HandleRef::None, _) => (),
                (HandleRef::Leaf(_), None) => singleton = Some(i),
                (HandleRef::Leaf(_), Some(_)) => return Ok(None),
                (HandleRef::Node(_), _) => return Ok(None),
            }
        }
        if let Some(idx) = singleton {
            Ok(Some(
                mem::take(&mut self.children_mut()[idx]).into_leaf().into(),
            ))
        } else {
            Ok(None)
        }
    }
}

/// Type for searching for keys in the HAMT
pub struct HAMTSearch<'a, K, V, O, H>
where
    O: ?Sized,
    H: ByteHash,
{
    hash: H::Digest,
    key: &'a O,
    depth: usize,
    _marker: PhantomData<(K, V)>,
}

impl<'a, K, V, O, H> From<&'a O> for HAMTSearch<'a, K, V, O, H>
where
    O: ?Sized + Hash,
    H: ByteHash,
{
    fn from(key: &'a O) -> Self {
        let hash = H::hash(&key);
        HAMTSearch {
            hash,
            key,
            depth: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, K, V, A, O, H> Method<HAMT<K, V, A, H>, H>
    for HAMTSearch<'a, K, V, O, H>
where
    HAMT<K, V, A, H>: SlotSelect,
    K: Borrow<O> + Content<H> + Eq + Hash,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    O: ?Sized + Eq,
    H: ByteHash,
{
    fn select(
        &mut self,
        compound: &HAMT<K, V, A, H>,
        _: usize,
    ) -> SearchResult {
        let slot = HAMT::select_slot(self.hash.as_ref(), self.depth);
        self.depth += 1;
        match compound.0[slot].leaf().map(Borrow::borrow) {
            Some(KV { key, val: _ }) if key.borrow() == self.key => {
                SearchResult::Leaf(slot)
            }
            _ => SearchResult::Path(slot),
        }
    }
}

impl<'a, K, V, A, O, H> Method<NarrowHAMT<K, V, A, H>, H>
    for HAMTSearch<'a, K, V, O, H>
where
    NarrowHAMT<K, V, A, H>: SlotSelect,
    K: Borrow<O> + Content<H> + Eq + Hash,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    O: ?Sized + Eq,
    H: ByteHash,
{
    fn select(
        &mut self,
        compound: &NarrowHAMT<K, V, A, H>,
        _: usize,
    ) -> SearchResult {
        let slot = NarrowHAMT::select_slot(self.hash.as_ref(), self.depth);
        self.depth += 1;
        match compound.0[slot].leaf().map(Borrow::borrow) {
            Some(KV { key, val: _ }) if key.borrow() == self.key => {
                SearchResult::Leaf(slot)
            }
            _ => SearchResult::Path(slot),
        }
    }
}

enum Removed<L> {
    None,
    Leaf(L),
    Collapse(L, L),
}

impl<K, V, A, H> HAMTTrait<K, V, H> for HAMT<K, V, A, H>
where
    K: Content<H> + Eq + Hash,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
}

impl<K, V, A, H> HAMTTrait<K, V, H> for NarrowHAMT<K, V, A, H>
where
    K: Content<H> + Eq + Hash,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
}

impl<K, V, A, H> SlotSelect for HAMT<K, V, A, H>
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    fn select_slot(hash: &[u8], depth: usize) -> usize {
        let ofs = depth / 2;
        (if ofs % 2 == 0 {
            (hash[ofs] & 0xF0) >> 4
        } else {
            hash[ofs] & 0x0F
        }) as usize
    }
}

impl<K, V, A, H> SlotSelect for NarrowHAMT<K, V, A, H>
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    fn select_slot(hash: &[u8], depth: usize) -> usize {
        let ofs = depth / 4;
        let m = ofs % 4;
        (match m {
            0 => hash[ofs] & 0xC0 >> 6,
            1 => hash[ofs] & 0x30 >> 4,
            2 => hash[ofs] & 0x0C >> 2,
            3 => hash[ofs] & 0x03,
            _ => unreachable!(),
        }) as usize
    }
}

impl<K, V, A, H> HAMT<K, V, A, H>
where
    K: Content<H> + Eq + Hash,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    /// Creates a new HAMT
    pub fn new() -> Self {
        HAMT(Default::default())
    }

    /// Insert key-value pair into the HAMT, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> io::Result<Option<V>> {
        self.sub_insert(0, H::hash(&k), k, v)
    }

    /// Get a reference to a value in the map
    pub fn get<O>(&self, k: &O) -> io::Result<Option<ValPath<K, V, Self, H>>>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        ValPath::new(self, &mut HAMTSearch::from(k.borrow()))
    }

    /// Get a mutable reference to a value in the map
    pub fn get_mut<O>(
        &mut self,
        k: &O,
    ) -> io::Result<Option<ValPathMut<K, V, Self, H>>>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        ValPathMut::new(self, &mut HAMTSearch::from(k.borrow()))
    }

    /// Remove element with given key, returning it.
    pub fn remove<O>(&mut self, k: &O) -> io::Result<Option<V>>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        match self.sub_remove(0, H::hash(k), k)? {
            Removed::None => Ok(None),
            Removed::Leaf(KV { key: _, val }) => Ok(Some(val)),
            _ => unreachable!(),
        }
    }
}

impl<K, V, A, H> NarrowHAMT<K, V, A, H>
where
    K: Content<H> + Eq + Hash,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    /// Creates a new HAMT
    pub fn new() -> Self {
        NarrowHAMT(Default::default())
    }

    /// Insert key-value pair into the HAMT, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> io::Result<Option<V>> {
        self.sub_insert(0, H::hash(&k), k, v)
    }

    /// Get a reference to a value in the map
    pub fn get<O>(&self, k: &O) -> io::Result<Option<ValPath<K, V, Self, H>>>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        ValPath::new(self, &mut HAMTSearch::from(k.borrow()))
    }

    /// Get a mutable reference to a value in the map
    pub fn get_mut<O>(
        &mut self,
        k: &O,
    ) -> io::Result<Option<ValPathMut<K, V, Self, H>>>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        ValPathMut::new(self, &mut HAMTSearch::from(k.borrow()))
    }

    /// Remove element with given key, returning it.
    pub fn remove<O>(&mut self, k: &O) -> io::Result<Option<V>>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        match self.sub_remove(0, H::hash(k), k)? {
            Removed::None => Ok(None),
            Removed::Leaf(KV { key: _, val }) => Ok(Some(val)),
            _ => unreachable!(),
        }
    }
}

impl<K, V, A, H> Content<H> for HAMT<K, V, A, H>
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        let mut mask = 0u16;
        for i in 0..16 {
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
        let mut bucket: [Handle<Self, H>; 16] = Default::default();
        let mask = <u16 as Content<H>>::restore(source)?;
        for (i, handle) in bucket.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                *handle = Handle::restore(source)?
            }
        }
        Ok(HAMT(bucket))
    }
}

impl<K, V, A, H> Content<H> for NarrowHAMT<K, V, A, H>
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        let mut mask = 0u8;
        for i in 0..4 {
            if let HandleType::None = self.0[i].handle_type() {
                // no-op
            } else {
                mask |= 1 << i;
            }
        }

        <u8 as Content<H>>::persist(&mut mask, sink)?;

        for (i, handle) in self.0.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                handle.persist(sink)?
            }
        }
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut bucket: [Handle<Self, H>; 4] = Default::default();
        let mask = <u8 as Content<H>>::restore(source)?;
        for (i, handle) in bucket.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                *handle = Handle::restore(source)?
            }
        }
        Ok(NarrowHAMT(bucket))
    }
}

impl<K, V, A, H> Compound<H> for HAMT<K, V, A, H>
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    type Leaf = KV<K, V>;
    type Annotation = A;

    fn children_mut(&mut self) -> &mut [Handle<Self, H>] {
        &mut self.0
    }

    fn children(&self) -> &[Handle<Self, H>] {
        &self.0
    }
}

impl<K, V, A, H> Compound<H> for NarrowHAMT<K, V, A, H>
where
    K: Content<H>,
    V: Content<H>,
    A: Annotation<KV<K, V>, H>,
    H: ByteHash,
{
    type Leaf = KV<K, V>;
    type Annotation = A;

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

    use kelvin::quickcheck_map;
    use kelvin::Blake2b;

    #[test]
    fn trivial_map() {
        let mut h = HAMT::<_, _, VoidAnnotation, Blake2b>::new();
        h.insert(28, 28).unwrap();
        assert_eq!(*h.get(&28).unwrap().unwrap(), 28);
    }

    #[test]
    fn bigger_map() {
        let mut h = HAMT::<_, _, VoidAnnotation, Blake2b>::new();
        for i in 0..1024 {
            assert!(h.get(&i).unwrap().is_none());
            h.insert(i, i).unwrap();
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn borrowed_keys() {
        let mut map = HAMT::<String, u8, VoidAnnotation, Blake2b>::new();
        map.insert("hello".into(), 8).unwrap();
        assert_eq!(*map.get("hello").unwrap().unwrap(), 8);
        assert_eq!(map.remove("hello").unwrap().unwrap(), 8);
    }

    #[test]
    fn nested_maps() {
        let mut map_a = HAMT::<_, _, VoidAnnotation, Blake2b>::new();
        for i in 0..128 {
            let mut map_b = HAMT::<_, _, VoidAnnotation, Blake2b>::new();

            for o in 0..128 {
                map_b.insert(o, o).unwrap();
            }

            map_a.insert(i, map_b).unwrap();
        }

        for i in 0..128 {
            let map_b = map_a.get(&i).unwrap().unwrap();

            for o in 0..100 {
                assert_eq!(*map_b.get(&o).unwrap().unwrap(), o);
            }
        }
    }

    mod wide {
        use super::*;
        quickcheck_map!(|| CountingHAMTMap::default());
    }

    mod narrow {
        use super::*;
        type CountingNarrowHAMTMap<K, V, H> =
            NarrowHAMT<K, V, Cardinality<u64>, H>;
        quickcheck_map!(|| CountingNarrowHAMTMap::default());
    }
}
