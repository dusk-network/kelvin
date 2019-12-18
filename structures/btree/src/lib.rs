use std::borrow::Borrow;
use std::io;
use std::iter::Iterator;
use std::mem;

use arrayvec::ArrayVec;

use kelvin::{
    annotation,
    annotations::{Cardinality, Counter, MaxKey, MaxKeyType},
    ByteHash, Compound, Content, Handle, HandleMut, Method, Sink, Source,
    ValPath, ValPathMut, ValRef, ValRefMut,
};

const N: usize = 2;
const M: usize = 3;

/// A hash array mapped trie
#[derive(Clone)]
pub struct BTree<K, V, H: ByteHash>(ArrayVec<[Handle<Self, H>; M]>)
where
    Self: Compound<H>;

impl<K: Content<H> + Ord, V: Content<H>, H: ByteHash> Default
    for BTree<K, V, H>
{
    fn default() -> Self {
        BTree(Default::default())
    }
}

annotation! {
    pub struct BTreeAnnotation<K, U> {
        key: MaxKey<K>,
        count: Cardinality<U>,
    }
    where
        K: MaxKeyType,
        U: Counter
}

#[derive(Clone)]
pub struct BTreeSearch<'a, K>(&'a K);

impl<'a, K> From<&'a K> for BTreeSearch<'a, K> {
    fn from(k: &'a K) -> Self {
        BTreeSearch(k)
    }
}

impl<'a, K, C, H> Method<C, H> for BTreeSearch<'a, K>
where
    C: Compound<H>,
    C::Annotation: Borrow<MaxKey<K>>,
    H: ByteHash,
    K: Clone + Ord + std::fmt::Debug,
{
    fn select(&mut self, handles: &[Handle<C, H>]) -> Option<usize> {
        for (i, h) in handles.iter().enumerate() {
            if let Some(ann) = h.annotation() {
                let handle_key: &MaxKey<K> = (*ann).borrow();
                if *self.0 <= **handle_key {
                    return Some(i);
                }
            }
        }
        None
    }
}

enum InsertResult<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Ok,
    Replaced(C::Leaf),
    Split(Handle<C, H>),
}

enum RemoveResult<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Noop,
    Removed(C::Leaf),
    Merge(C, C::Leaf),
}

impl<K, V, H> BTree<K, V, H>
where
    K: Content<H> + Ord + Eq + std::fmt::Debug,
    V: Content<H>,
    H: ByteHash,
{
    /// Creates a new BTree
    pub fn new() -> Self {
        BTree(Default::default())
    }

    /// Insert key-value pair into the BTree, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> io::Result<Option<V>> {
        match self._insert(Handle::new_leaf((k, v)), 0)? {
            InsertResult::Ok => Ok(None),
            InsertResult::Replaced((_, v)) => Ok(Some(v)),
            InsertResult::Split(_) => unreachable!(),
        }
    }

    fn _insert(
        &mut self,
        mut handle: Handle<Self, H>,
        depth: usize,
    ) -> io::Result<InsertResult<Self, H>> {
        println!("entry");

        /// Use an enum to get around borrow issues
        #[derive(Debug)]
        enum Action {
            Append,
            Insert(usize),
            Replace(usize),
            Split(usize),
        }
        // The default action
        let mut action = Action::Append;

        let annotation = &*handle.annotation().expect("invalid handle");
        let borrow: &MaxKey<K> = annotation.borrow();
        let ann_key: &K = &**borrow;

        match BTreeSearch(ann_key).select(self.children()) {
            Some(i) => match &mut *self.0[i].inner_mut()? {
                HandleMut::None => unreachable!(),
                HandleMut::Leaf((key, _)) => {
                    if key == ann_key {
                        action = Action::Replace(i);
                    } else if *key > *ann_key {
                        action = Action::Insert(i);
                    }
                }
                HandleMut::Node(n) => {
                    let node_ann =
                        n.annotation().expect("node without annotation");

                    let node_key: &MaxKey<K> = node_ann.borrow();

                    // Recurse
                    if **node_key >= *ann_key {
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
            None => (),
        }

        loop {
            println!("ACTION {:?}", action);
            match action {
                Action::Append => {
                    if !self.0.is_full() {
                        self.0.push(handle);
                        return Ok(InsertResult::Ok);
                    } else {
                        action = Action::Split(M - 1);
                    }
                }
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
                    // [ 1 ] [ 3, 5 ]
                    // [ 1, 2 ] [ 3, 5 ]

                    // # SPLIT CASE B

                    // [ 1, 3, 5 ]
                    // [ 1, 3 ] [ 5 ]
                    // [ 1, 3 ] [ 4, 5 ]

                    // The new (second) node is returned down the stack for merging

                    debug_assert!(self.0.len() == M);

                    let mut new_node = Self::new();
                    let popped =
                        self.0.pop().expect("attempt to split empty node");

                    let result = if i < N {
                        // CASE A
                        let second =
                            self.0.pop().expect("attempt to split empty node");
                        new_node.0.push(second);
                        new_node.0.push(popped);
                        self._insert(handle, depth + 1)?
                    } else {
                        // CASE B
                        new_node.0.push(popped);
                        new_node._insert(handle, depth + 1)?
                    };

                    // recursive insert should always succeed in this case
                    debug_assert!(if let InsertResult::Ok = result {
                        true
                    } else {
                        false
                    });

                    debug_assert!(self.0.len() == N);
                    debug_assert!(new_node.0.len() == N);

                    let new_handle = Handle::new_node(new_node);

                    if depth == 0 {
                        println!("new root");
                        // if we're on the top level, we create a new root.
                        let old_root = mem::replace(self, Self::new());
                        self.0.push(Handle::new_node(old_root));
                        self.0.push(new_handle);

                        return Ok(InsertResult::Ok);
                    } else {
                        return Ok(InsertResult::Split(new_handle));
                    }

                    // make sure `self` has space by popping and storing in new node
                }
            }
        }
    }

    /// Remove element with given key, returning it.
    pub fn remove(&mut self, k: &K) -> io::Result<Option<V>> {
        match self._remove(k, 0)? {
            RemoveResult::Removed((_, v)) => Ok(Some(v)),
            RemoveResult::Noop => Ok(None),
            _ => unreachable!(),
        }
    }

    fn _remove(
        &mut self,
        k: &K,
        depth: usize,
    ) -> io::Result<RemoveResult<Self, H>> {
        enum Action<K: Content<H> + Ord, V: Content<H>, H: ByteHash> {
            Noop,
            Remove(usize),
            Merge(BTree<K, V, H>, (K, V), usize),
        }
        // The default action
        let mut action = Action::Noop;

        match BTreeSearch(k).select(self.children()) {
            Some(i) => {
                match &mut *self.0[i].inner_mut()? {
                    HandleMut::None => unreachable!(),
                    HandleMut::Leaf((key, _)) => {
                        if key == k {
                            action = Action::Remove(i);
                        }
                    }
                    HandleMut::Node(n) => {
                        let ann =
                            n.annotation().expect("node without annotation");
                        let max_key: &MaxKey<K> = ann.borrow();

                        // Recurse
                        if **max_key >= *k {
                            match n._remove(&k, depth + 1)? {
                                RemoveResult::Noop => (),
                                RemoveResult::Removed(leaf) => {
                                    return Ok(RemoveResult::Removed(leaf))
                                }
                                RemoveResult::Merge(handle, removed) => {
                                    action = Action::Merge(handle, removed, i)
                                }
                            }
                        }
                    }
                }
            }
            None => return Ok(RemoveResult::Noop),
        }

        match action {
            Action::Remove(i) => {
                let removed = self.0.remove(i);

                if self.0.len() < N && depth > 0 {
                    let to_merge = mem::replace(self, Self::new());
                    Ok(RemoveResult::Merge(to_merge, removed.into_leaf()))
                } else {
                    Ok(RemoveResult::Removed(removed.into_leaf()))
                }
            }
            Action::Noop => Ok(RemoveResult::Noop),
            Action::Merge(to_merge, leaf, i) => {
                self.0.remove(i);
                match self._insert(Handle::new_node(to_merge), depth)? {
                    InsertResult::Split(_) => unreachable!("invalid split"),
                    _ => (),
                }
                Ok(RemoveResult::Removed(leaf))
            }
        }
    }

    /// Returns a reference to a value in the map, if any
    pub fn get(&self, k: &K) -> io::Result<Option<impl ValRef<V>>> {
        ValPath::new(self, &mut BTreeSearch::from(k), k)
    }

    /// Returns a reference to a mutable value in the map, if any
    pub fn get_mut<'a>(
        &'a mut self,
        k: &K,
    ) -> io::Result<Option<impl ValRefMut<V>>> {
        ValPathMut::new(self, &mut BTreeSearch::from(k), k)
    }
}

impl<K, V, H> Content<H> for BTree<K, V, H>
where
    K: Content<H> + Ord,
    V: Content<H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        (self.0.len() as u8).persist(sink)?;
        for h in &mut self.0 {
            h.persist(sink)?
        }
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut b = BTree::default();
        let len = u8::restore(source)?;
        for _ in 0..len {
            b.0.push(Handle::restore(source)?);
        }
        Ok(b)
    }
}

impl<K, V, H> Compound<H> for BTree<K, V, H>
where
    H: ByteHash,
    K: Content<H> + Ord,
    V: Content<H>,
{
    type Leaf = (K, V);
    type Annotation = BTreeAnnotation<K, u64>;

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
        let mut h = BTree::<_, _, Blake2b>::new();
        h.insert(28, 28).unwrap();
        assert_eq!(*h.get(&28).unwrap().unwrap(), 28);
    }

    #[test]
    fn bigger_map() {
        let mut h = BTree::<_, _, Blake2b>::new();
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
        let mut h = BTree::<_, _, Blake2b>::new();
        let bigger = 1024;
        for i in 0..bigger {
            let i = bigger - i - 1;
            println!("insert {}", i);
            h.insert(i, i).unwrap();
        }
        println!("inserting done");
        for i in 0..bigger {
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn insert_remove() {
        let mut h = BTree::<_, _, Blake2b>::new();
        let bigger = 4;
        for i in 0..bigger {
            let i = bigger - i - 1;
            println!("insert {}", i);
            h.insert(i, i).unwrap();
        }
        println!("inserting done");
        for i in 0..bigger {
            assert_eq!(h.remove(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn nested_maps() {
        let mut map_a = BTree::<_, _, Blake2b>::new();
        for i in 0..100 {
            let mut map_b = BTree::<_, _, Blake2b>::new();

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

    quickcheck_map!(|| BTree::new());
}
