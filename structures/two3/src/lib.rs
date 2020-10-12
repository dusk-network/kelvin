// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

//! A 2-3 Tree implemented on kelvin
#![warn(missing_docs)]

use std::borrow::Borrow;
use std::marker::PhantomData;
use std::mem;

use arrayvec::ArrayVec;

use canonical::{Canon, Store};
use canonical_derive::Canon;

use kelvin::{
    annotation,
    annotations::{Annotation, Cardinality, Counter, MaxKey, MaxKeyType},
    Compound, Handle, HandleMut, HandleType, Method, SearchResult, ValPath,
    ValPathMut, KV,
};

/// The default 2-3 tree
pub type DefaultTwo3Map<K, V, S> = Two3Tree<K, V, MaxKey<K>, S>;

const N: usize = 2;
const M: usize = 3;

/// A 2-3 tree
#[derive(Clone, Canon)]
pub struct Two3Tree<K, V, A, S: Store>(ArrayVec<[Handle<Self, S>; M]>)
where
    Self: Compound<S>;

impl<K, V, A, S> Default for Two3Tree<K, V, A, S>
where
    K: Canon<S> + Ord,
    V: Canon<S>,
    A: Annotation<KV<K, V>, S>,
    S: Store,
{
    fn default() -> Self {
        Two3Tree(Default::default())
    }
}

annotation! {
    struct Two3TreeAnnotation<K, U> {
        key: MaxKey<K>,
        count: Cardinality<U>,
    }
    where
        K: MaxKeyType,
        U: Counter
}

/// Struct used to search the 2-3 Tree
pub struct Two3TreeSearch<'a, K, O: ?Sized>(&'a O, PhantomData<K>);

impl<'a, K, O> Two3TreeSearch<'a, K, O>
where
    O: ?Sized,
{
    fn new(key: &'a O) -> Self {
        Two3TreeSearch(key, PhantomData)
    }
}

impl<'a, K, O: ?Sized> From<&'a O> for Two3TreeSearch<'a, K, O> {
    fn from(k: &'a O) -> Self {
        Two3TreeSearch(k, PhantomData)
    }
}

impl<'a, K, V, A, O, S> Method<Two3Tree<K, V, A, S>, S>
    for Two3TreeSearch<'a, K, O>
where
    K: Ord + Borrow<O> + Canon<S>,
    V: Canon<S>,
    A: Annotation<KV<K, V>, S> + Borrow<MaxKey<K>>,
    O: Ord + ?Sized,
    S: Store,
{
    fn select(
        &mut self,
        compound: &Two3Tree<K, V, A, S>,
        _: usize,
    ) -> SearchResult {
        for (i, h) in compound.0.iter().enumerate() {
            if let Some(ann) = h.annotation() {
                let handle_key: &MaxKey<K> = (*ann).borrow();
                if self.0 == (**handle_key).borrow() {
                    // correct key
                    if h.handle_type() == HandleType::Leaf {
                        return SearchResult::Leaf(i);
                    } else {
                        return SearchResult::Path(i);
                    }
                } else if self.0 < (**handle_key).borrow() {
                    return SearchResult::Path(i);
                }
            }
        }
        let len = compound.0.len();
        // Always select last element if node
        if len > 0 && compound.0[len - 1].handle_type() == HandleType::Node {
            SearchResult::Path(len - 1)
        } else {
            SearchResult::None
        }
    }
}

enum InsertResult<C, S>
where
    C: Compound<S>,
    S: Store,
{
    Ok,
    Replaced(C::Leaf),
    Split(Handle<C, S>),
}

enum RemoveResult<C, S>
where
    C: Compound<S>,
    S: Store,
{
    Noop,
    Removed(C::Leaf),
    Merge(C::Leaf),
}

impl<K, V, A, S> Two3Tree<K, V, A, S>
where
    K: Canon<S> + Ord,
    V: Canon<S>,
    A: Annotation<KV<K, V>, S> + Borrow<MaxKey<K>>,
    S: Store,
{
    /// Creates a new Two3Tree
    pub fn new() -> Self {
        Two3Tree(Default::default())
    }

    /// Insert key-value pair into the Two3Tree, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> Result<Option<V>, S::Error> {
        match self._insert(Handle::new_leaf(KV::new(k, v)), 0)? {
            InsertResult::Ok => Ok(None),
            InsertResult::Replaced(KV { key: _, val }) => Ok(Some(val)),
            InsertResult::Split(_) => unreachable!(),
        }
    }

    /// Get a reference to a value in the map
    pub fn get<O>(
        &self,
        k: &O,
    ) -> Result<Option<ValPath<K, V, Self, S>>, S::Error>
    where
        O: ?Sized + Ord + Eq,
        K: Borrow<O>,
    {
        ValPath::new(self, &mut Two3TreeSearch::from(k.borrow()))
    }

    /// Get a mutable reference to a value in the map
    pub fn get_mut<O>(
        &mut self,
        k: &O,
    ) -> Result<Option<ValPathMut<K, V, Self, S>>, S::Error>
    where
        O: ?Sized + Ord + Eq + Borrow<K>,
        K: Borrow<O>,
    {
        ValPathMut::new(self, &mut Two3TreeSearch::from(k.borrow()))
    }

    fn _insert(
        &mut self,
        mut handle: Handle<Self, S>,
        depth: usize,
    ) -> Result<InsertResult<Self, S>, S::Error> {
        /// Use an enum to get around borrow issues
        #[derive(Debug)]
        enum Action {
            Insert(usize),
            Replace(usize),
            Split(usize),
            Placeholder,
        }
        // The default action
        let mut action = Action::Placeholder;

        let annotation = &*handle.annotation().expect("invalid handle");
        let borrow: &MaxKey<K> = annotation.borrow();
        let ann_key: &K = &**borrow;
        let len = self.0.len();

        match Two3TreeSearch::new(ann_key).select(self, 0) {
            SearchResult::Leaf(i) => {
                action = Action::Replace(i);
            }
            SearchResult::Path(i) => match self.0[i].inner_mut()? {
                HandleMut::None(_) => unreachable!(),
                HandleMut::Leaf(ref mut leaf) => {
                    let KV { ref key, val: _ } = (**leaf).borrow();
                    if *key > *ann_key {
                        action = Action::Insert(i);
                    } else if i + 1 == len {
                        action = Action::Insert(i + 1);
                    }
                }
                HandleMut::Node(ref mut n) => {
                    let node_ann =
                        n.annotation().expect("node without annotation");

                    let node_key: &MaxKey<K> = node_ann.borrow();

                    // Recurse, also if it's the last node
                    if **node_key >= *ann_key || i + 1 == len {
                        match n._insert(handle, depth + 1)? {
                            ok @ InsertResult::Ok => return Ok(ok),
                            replace @ InsertResult::Replaced(_) => {
                                return Ok(replace)
                            }
                            InsertResult::Split(new_handle) => {
                                handle = new_handle;
                                action = Action::Insert(i + 1);
                            }
                        }
                    }
                }
            },
            SearchResult::None => action = Action::Insert(len),
        }

        loop {
            match action {
                Action::Placeholder => unreachable!("reached placeholder"),
                Action::Replace(i) => {
                    let replaced =
                        mem::replace(&mut self.0[i], handle).into_leaf();
                    return Ok(InsertResult::Replaced(replaced));
                }
                Action::Insert(i) => {
                    if !self.0.is_full() {
                        self.0.insert(i, handle);
                        return Ok(InsertResult::Ok);
                    } else {
                        action = Action::Split(i)
                    }
                }
                Action::Split(i) => {
                    // initial state
                    // [ 1, 3, 5 ]

                    // # SPLIT CASE A

                    // [ 1, 3, 5 ]
                    // [ 0 ] [ 3, 5 ]
                    // [ 0, 1 ] [ 3, 5 ]

                    // # SPLIT CASE B

                    // [ 1, 3, 5 ]
                    // [ 1, 3 ] [ 4 ]
                    // [ 1, 3 ] [ 4, 5 ]

                    // The new (second) node is returned down the stack for merging

                    debug_assert!(self.0.len() == M);

                    let mut new_node = Self::new();
                    let popped =
                        self.0.pop().expect("attempt to split empty node");

                    if i < N {
                        // CASE A
                        let second =
                            self.0.pop().expect("attempt to split empty node");

                        new_node.0.push(second);
                        new_node.0.push(popped);

                        self.0.insert(i, handle);
                    } else {
                        // CASE B
                        new_node.0.push(popped);
                        new_node.0.insert(i - N, handle);
                    }

                    debug_assert!(self.0.len() == N);
                    debug_assert!(new_node.0.len() == N);

                    let new_handle = Handle::new_node(new_node);

                    if depth == 0 {
                        // if we're on the top level, we create a new root.
                        let old_root = mem::replace(self, Self::new());
                        self.0.push(Handle::new_node(old_root));
                        self.0.push(new_handle);

                        return Ok(InsertResult::Ok);
                    } else {
                        return Ok(InsertResult::Split(new_handle));
                    }
                }
            }
        }
    }

    /// Remove element with given key, returning it.
    pub fn remove<O>(&mut self, o: &O) -> Result<Option<V>, S::Error>
    where
        O: ?Sized + Ord + Eq,
        K: Borrow<O>,
    {
        match self._remove(o, 0)? {
            RemoveResult::Removed(KV { key: _, val }) => Ok(Some(val)),
            RemoveResult::Noop => Ok(None),
            _ => unreachable!(),
        }
    }

    fn _remove<O>(
        &mut self,
        k: &O,
        depth: usize,
    ) -> Result<RemoveResult<Self, S>, S::Error>
    where
        O: ?Sized + Ord + Eq,
        K: Borrow<O>,
    {
        enum Action<L> {
            Noop,
            Remove(usize),
            Merge(usize, L),
        }
        // The default action
        let mut action = Action::Noop;

        match Two3TreeSearch::new(k.borrow()).select(self, 0) {
            SearchResult::Leaf(i) => {
                action = Action::Remove(i);
            }
            SearchResult::Path(i) => {
                match self.0[i].inner_mut()? {
                    HandleMut::None(_) => unreachable!(),
                    HandleMut::Leaf(_) => (),
                    HandleMut::Node(ref mut n) => {
                        let ann =
                            n.annotation().expect("node without annotation");
                        let max_key: &MaxKey<K> = ann.borrow();
                        let max_key: &K = max_key.borrow();

                        // Recurse
                        if max_key.borrow() >= k {
                            match n._remove(k, depth + 1)? {
                                RemoveResult::Noop => (),
                                RemoveResult::Removed(leaf) => {
                                    return Ok(RemoveResult::Removed(leaf))
                                }
                                RemoveResult::Merge(removed_leaf) => {
                                    action = Action::Merge(i, removed_leaf)
                                }
                            }
                        }
                    }
                }
            }
            SearchResult::None => return Ok(RemoveResult::Noop),
        }

        match action {
            Action::Remove(i) => {
                let removed = self.0.remove(i);
                // are we under-filled at a depth of at least 1?
                if self.0.len() < N && depth > 0 {
                    Ok(RemoveResult::Merge(removed.into_leaf()))
                } else {
                    Ok(RemoveResult::Removed(removed.into_leaf()))
                }
            }
            Action::Noop => Ok(RemoveResult::Noop),
            Action::Merge(i, leaf) => {
                // Case A
                // [0, 1] [2] ... -> [0, 1, 2] ...

                // Case B
                // [0, 1, 2] [3] ... -> [0, 1] [2, 3] ...

                // Case C
                // [0] [1, 2] ... -> [0, 1, 2] ...

                // Case D
                // [0] [1, 2, 3] ... -> [0, 1] [2, 3] ...

                // in order to keep the borrow checker happy and do minimal
                // lookups, we first replace the to-be-merged node with an empty one.

                let mut to_merge =
                    mem::replace(&mut self.0[i], Handle::default()).into_node();

                // Is there a node before this one?
                if i > 0 {
                    // Case A/B
                    match &mut self.0[i - 1].inner_mut()? {
                        HandleMut::Node(n) => {
                            if n.0.len() == N {
                                // Case A - move from to_merge into prev node
                                let popped = to_merge
                                    .0
                                    .pop()
                                    .expect("attempt to merge empty node");
                                n.0.push(popped)
                            } else {
                                // Case B - pop from node and prepend to to_merge
                                let popped =
                                    n.0.pop().expect("len guaranteed > 0");
                                to_merge.0.insert(0, popped);
                            }
                        }
                        _ => unreachable!(),
                    }
                } else {
                    // Case C/D
                    match self.0[i + 1].inner_mut()? {
                        HandleMut::Node(ref mut n) => {
                            if n.0.len() == N {
                                // Case C
                                let popped = to_merge
                                    .0
                                    .pop()
                                    .expect("attempt to merge empty node");
                                // prepend into next node
                                n.0.insert(0, popped)
                            } else {
                                // Case D
                                let removed = n.0.remove(0);
                                to_merge.0.push(removed);
                            }
                        }
                        _ => unreachable!(),
                    }
                }

                // did we empty the to_merge node?
                if to_merge.0.len() > 0 {
                    // swap back
                    self.0[i] = Handle::new_node(to_merge);
                    Ok(RemoveResult::Removed(leaf))
                } else {
                    // remove empty node
                    self.0.remove(i);
                    if self.0.len() < N {
                        if depth > 0 {
                            Ok(RemoveResult::Merge(leaf))
                        } else {
                            // replace root
                            let singleton =
                                mem::replace(&mut self.0[0], Handle::default());
                            *self = singleton.into_node();
                            Ok(RemoveResult::Removed(leaf))
                        }
                    } else {
                        Ok(RemoveResult::Removed(leaf))
                    }
                }
            }
        }
    }
}

impl<K, V, A, S> Compound<S> for Two3Tree<K, V, A, S>
where
    S: Store,
    K: Canon<S> + Ord,
    V: Canon<S>,
    A: Annotation<KV<K, V>, S>,
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

    use kelvin::quickcheck_map;

    #[test]
    fn trivial_map() {
        let mut h = Two3Tree::<_, _, MaxKey<_>, Blake2b>::new();
        h.insert(28, 28).unwrap();
        assert_eq!(*h.get(&28).unwrap().unwrap(), 28);
    }

    #[test]
    fn bigger_map() {
        let mut h = Two3Tree::<_, _, MaxKey<_>, Blake2b>::new();
        let bigger = 1024;
        for i in 0..bigger {
            h.insert(i, i).unwrap();
        }
        for i in 0..bigger {
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn bigger_map_reverse() {
        let mut h = Two3Tree::<_, _, MaxKey<_>, Blake2b>::new();
        let bigger = 1024;
        for i in 0..bigger {
            let i = bigger - i - 1;
            h.insert(i, i).unwrap();
        }
        for i in 0..bigger {
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn insert_remove() {
        let mut h = Two3Tree::<_, _, MaxKey<_>, Blake2b>::new();
        let bigger = 1024;
        for i in 0..bigger {
            let i = bigger - i - 1;
            h.insert(i, i).unwrap();
        }
        for i in 0..bigger {
            assert_eq!(h.remove(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn insert_remove_reverse() {
        let mut h = Two3Tree::<_, _, MaxKey<_>, Blake2b>::new();
        let bigger = 4;
        for i in 0..bigger {
            h.insert(i, i).unwrap();
        }
        for i in 0..bigger {
            let i = bigger - i - 1;
            assert_eq!(h.remove(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn insert_get_reverse() {
        let mut h = Two3Tree::<_, _, MaxKey<_>, Blake2b>::new();
        let bigger = 4;
        for i in 0..bigger {
            h.insert(i, i).unwrap();
        }
        for i in 0..bigger {
            let i = bigger - i - 1;
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn borrowed_keys() {
        let mut map = Two3Tree::<String, u8, MaxKey<_>, Blake2b>::new();
        map.insert("hello".into(), 8).unwrap();
        assert_eq!(*map.get("hello").unwrap().unwrap(), 8);
        assert_eq!(map.remove("hello").unwrap().unwrap(), 8);
    }

    #[test]
    fn nested_maps() {
        let mut map_a = Two3Tree::<_, _, MaxKey<_>, Blake2b>::new();
        for i in 0..100 {
            let mut map_b = Two3Tree::<_, _, MaxKey<_>, Blake2b>::new();

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

    quickcheck_map!(|| {
        Two3Tree::<_, _, Two3TreeAnnotation<_, u64>, Blake2b>::new()
    });
}
