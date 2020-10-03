// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

//! A Hash-array mapped trie implemented on kelvin
#![warn(missing_docs)]

use std::borrow::Borrow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::iter::Iterator;
use std::marker::PhantomData;
use std::mem;

use canonical::{Canon, Store};
use canonical_derive::Canon;

use kelvin::{
    annotations::{Annotation, Cardinality, Void},
    Compound, Handle, HandleMut, HandleRef, Method, SearchResult, ValPath,
    ValPathMut, KV,
};

fn hash<T: Hash, S: Store>(t: &T) -> S::Ident {
    let mut hasher = DefaultHasher::new();
    t.hash(&mut hasher);
    let bytes = hasher.finish().to_be_bytes();
    S::Ident::from(&bytes)
}

/// Default HAMT-map without annotations
pub type DefaultHAMTMap<K, V, S> = HAMT<K, V, Void, S>;
/// Default HAMT-map with Cardinality annotation (for `.count()`)
pub type CountingHAMTMap<K, V, S> = HAMT<K, V, Cardinality<u64>, S>;

/// A hash array mapped trie with branching factor of 16
#[derive(Clone, Canon)]
pub struct HAMT<K, V, A, S>([Handle<Self, S>; 16])
where
    K: Canon<S> + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store;

impl<K, V, A, S> Default for HAMT<K, V, A, S>
where
    K: Canon<S> + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
{
    fn default() -> Self {
        HAMT(Default::default())
    }
}

/// A hash array mapped trie with branching factor of 4
#[derive(Clone, Canon)]
pub struct NarrowHAMT<K, V, A, S>([Handle<Self, S>; 4])
where
    K: Canon<S> + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store;

impl<K, V, A, S> Default for NarrowHAMT<K, V, A, S>
where
    K: Canon<S> + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
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
trait HAMTTrait<K, V, S>: Compound<S> + Default + SlotSelect
where
    S: Store,
    K: Eq + Hash + Clone,
    V: Clone,
    Self::Leaf: Borrow<KV<K, V>> + From<KV<K, V>> + Into<KV<K, V>>,
{
    fn sub_insert(
        &mut self,
        depth: usize,
        h: S::Ident,
        k: K,
        v: V,
    ) -> Result<Option<V>, S::Error> {
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
                return node.val_mut(|node| {
                    node.sub_insert(depth + 1, h, k.clone(), v.clone())
                })
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

                let old_h = hash::<_, S>(&key);

                let mut new_node = Self::default();
                new_node.sub_insert(depth + 1, h, k, v)?;
                new_node.sub_insert(depth + 1, old_h, key, val)?;
                self.children_mut()[s] = Handle::new_node(new_node)?;
                None
            }
        })
    }

    fn sub_remove<O>(
        &mut self,
        depth: usize,
        h: S::Ident,
        k: &O,
    ) -> Result<Removed<KV<K, V>>, S::Error>
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
                    match node
                        .val_mut(|node| node.sub_remove(depth + 1, h, k))?
                    {
                        Removed::Collapse(removed, reinsert) => {
                            removed_leaf = removed.into();
                            node.replace(Handle::new_leaf(reinsert.into()));
                        }
                        a => return Ok(a),
                    }
                }
            }
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

    fn remove_singleton(&mut self) -> Result<Option<KV<K, V>>, S::Error> {
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
pub struct HAMTSearch<'a, K, V, O, S>
where
    O: ?Sized,
    S: Store,
{
    hash: S::Ident,
    key: &'a O,
    depth: usize,
    _marker: PhantomData<(K, V)>,
}

impl<'a, K, V, O, S> From<&'a O> for HAMTSearch<'a, K, V, O, S>
where
    O: ?Sized + Hash,
    S: Store,
{
    fn from(key: &'a O) -> Self {
        HAMTSearch {
            hash: hash::<_, S>(&key),
            key,
            depth: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, K, V, A, O, S> Method<HAMT<K, V, A, S>, S>
    for HAMTSearch<'a, K, V, O, S>
where
    HAMT<K, V, A, S>: SlotSelect,
    K: Borrow<O> + Canon<S> + Eq + Hash + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    O: ?Sized + Eq,
    S: Store,
{
    fn select(
        &mut self,
        compound: &HAMT<K, V, A, S>,
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

impl<'a, K, V, A, O, S> Method<NarrowHAMT<K, V, A, S>, S>
    for HAMTSearch<'a, K, V, O, S>
where
    NarrowHAMT<K, V, A, S>: SlotSelect,
    K: Borrow<O> + Canon<S> + Eq + Hash + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    O: ?Sized + Eq,
    S: Store,
{
    fn select(
        &mut self,
        compound: &NarrowHAMT<K, V, A, S>,
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

impl<K, V, A, S> HAMTTrait<K, V, S> for HAMT<K, V, A, S>
where
    K: Canon<S> + Eq + Hash + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
{
}

impl<K, V, A, S> HAMTTrait<K, V, S> for NarrowHAMT<K, V, A, S>
where
    K: Canon<S> + Eq + Hash + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
{
}

impl<K, V, A, S> SlotSelect for HAMT<K, V, A, S>
where
    K: Canon<S> + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
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

impl<K, V, A, S> SlotSelect for NarrowHAMT<K, V, A, S>
where
    K: Canon<S> + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
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

impl<K, V, A, S> HAMT<K, V, A, S>
where
    K: Canon<S> + Clone + Eq + Hash,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
{
    /// Creates a new HAMT
    pub fn new() -> Self {
        HAMT(Default::default())
    }

    /// Insert key-value pair into the HAMT, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> Result<Option<V>, S::Error> {
        self.sub_insert(0, hash::<_, S>(&k), k, v)
    }

    /// Get a reference to a value in the map
    pub fn get<O>(
        &self,
        k: &O,
    ) -> Result<Option<ValPath<K, V, Self, S>>, S::Error>
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
    ) -> Result<Option<ValPathMut<K, V, Self, S>>, S::Error>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        ValPathMut::new(self, &mut HAMTSearch::from(k.borrow()))
    }

    /// Remove element with given key, returning it.
    pub fn remove<O>(&mut self, k: &O) -> Result<Option<V>, S::Error>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        match self.sub_remove(0, hash::<_, S>(&k), k)? {
            Removed::None => Ok(None),
            Removed::Leaf(KV { key: _, val }) => Ok(Some(val)),
            _ => unreachable!(),
        }
    }
}

impl<K, V, A, S> NarrowHAMT<K, V, A, S>
where
    K: Canon<S> + Clone + Eq + Hash,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
{
    /// Creates a new HAMT
    pub fn new() -> Self {
        NarrowHAMT(Default::default())
    }

    /// Insert key-value pair into the HAMT, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> Result<Option<V>, S::Error> {
        self.sub_insert(0, hash::<_, S>(&k), k, v)
    }

    /// Get a reference to a value in the map
    pub fn get<O>(
        &self,
        k: &O,
    ) -> Result<Option<ValPath<K, V, Self, S>>, S::Error>
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
    ) -> Result<Option<ValPathMut<K, V, Self, S>>, S::Error>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        ValPathMut::new(self, &mut HAMTSearch::from(k.borrow()))
    }

    /// Remove element with given key, returning it.
    pub fn remove<O>(&mut self, k: &O) -> Result<Option<V>, S::Error>
    where
        O: ?Sized + Hash + Eq,
        K: Borrow<O>,
    {
        match self.sub_remove(0, hash::<_, S>(&k), k)? {
            Removed::None => Ok(None),
            Removed::Leaf(KV { key: _, val }) => Ok(Some(val)),
            _ => unreachable!(),
        }
    }
}

impl<K, V, A, S> Compound<S> for HAMT<K, V, A, S>
where
    K: Canon<S> + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
{
    type Leaf = KV<K, V>;
    type Annotation = A;

    fn children_mut(&mut self) -> &mut [Handle<Self, S>] {
        &mut self.0
    }

    fn children(&self) -> &[Handle<Self, S>] {
        &self.0
    }
}

impl<K, V, A, S> Compound<S> for NarrowHAMT<K, V, A, S>
where
    K: Canon<S> + Clone,
    V: Canon<S> + Clone,
    A: Annotation<KV<K, V>, S>,
    S: Store,
{
    type Leaf = KV<K, V>;
    type Annotation = A;

    fn children_mut(&mut self) -> &mut [Handle<Self, S>] {
        &mut self.0
    }

    fn children(&self) -> &[Handle<Self, S>] {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use kelvin::{quickcheck_map, Blake2b, Erased, Store};

    #[test]
    fn trivial_map() {
        let mut h = HAMT::<_, _, Void, Blake2b>::new();
        h.insert(28, 28).unwrap();
        assert_eq!(*h.get(&28).unwrap().unwrap(), 28);
    }

    #[test]
    fn bigger_map() {
        let mut h = HAMT::<_, _, Void, Blake2b>::new();
        for i in 0..1024 {
            assert!(h.get(&i).unwrap().is_none());
            h.insert(i, i).unwrap();
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn borrowed_keys() {
        let mut map = HAMT::<String, u8, Void, Blake2b>::new();
        map.insert("hello".into(), 8).unwrap();
        assert_eq!(*map.get("hello").unwrap().unwrap(), 8);
        assert_eq!(map.remove("hello").unwrap().unwrap(), 8);
    }

    #[test]
    fn erased() {
        let store = Store::<Blake2b>::ephemeral();

        let mut hamt = DefaultHAMTMap::new();

        for i in 0u32..128 {
            hamt.insert(i, i).unwrap();
        }

        let mut erased = Erased::wrap(hamt, &store).unwrap();
        let query =
            erased.query::<DefaultHAMTMap<u32, u32, Blake2b>>().unwrap();

        assert_eq!(*query.get(&37).unwrap().unwrap(), 37);

        let mut transaction = erased
            .transaction::<DefaultHAMTMap<u32, u32, Blake2b>>()
            .unwrap();

        transaction.insert(0, 1234).unwrap();
        transaction.commit().unwrap();

        let second_query =
            erased.query::<DefaultHAMTMap<u32, u32, Blake2b>>().unwrap();

        assert_eq!(*query.get(&0).unwrap().unwrap(), 0);
        assert_eq!(*second_query.get(&0).unwrap().unwrap(), 1234);
    }

    #[test]
    fn nested_maps() {
        let mut map_a = HAMT::<_, _, Void, Blake2b>::new();
        for i in 0..128 {
            let mut map_b = HAMT::<_, _, Void, Blake2b>::new();

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
        type CountingNarrowHAMTMap<K, V, S> =
            NarrowHAMT<K, V, Cardinality<u64>, S>;
        quickcheck_map!(|| CountingNarrowHAMTMap::default());
    }
}
