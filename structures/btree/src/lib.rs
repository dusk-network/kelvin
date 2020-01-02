use std::borrow::Borrow;
use std::io;
use std::iter::Iterator;
use std::marker::PhantomData;
use std::mem;

use arrayvec::ArrayVec;

use kelvin::{
    annotation,
    annotations::{Cardinality, Counter, MaxKey, MaxKeyType},
    ByteHash, Compound, Content, Handle, HandleMut, HandleType, Map, Method,
    Sink, Source,
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

pub struct BTreeSearch<'a, K, O: ?Sized>(&'a O, PhantomData<K>);

impl<'a, K, O> BTreeSearch<'a, K, O> {
    fn new(key: &'a O) -> Self {
        BTreeSearch(key, PhantomData)
    }
}

impl<'a, K, O: ?Sized> From<&'a O> for BTreeSearch<'a, K, O> {
    fn from(k: &'a O) -> Self {
        BTreeSearch(k, PhantomData)
    }
}

impl<'a, K, O, C, H> Method<C, H> for BTreeSearch<'a, K, O>
where
    C: Compound<H>,
    C::Annotation: Borrow<MaxKey<K>>,
    H: ByteHash,
    K: Ord + Borrow<O>,
    O: Ord + ?Sized,
{
    fn select(&mut self, handles: &[Handle<C, H>]) -> Option<usize> {
        for (i, h) in handles.iter().enumerate() {
            if let Some(ann) = h.annotation() {
                let handle_key: &MaxKey<K> = (*ann).borrow();
                if self.0 <= (**handle_key).borrow() {
                    return Some(i);
                }
            }
        }
        // Always select last element if node
        let len = handles.len();
        if len > 0 && handles[len - 1].handle_type() == HandleType::Node {
            Some(len - 1)
        } else {
            None
        }
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
    Merge(C::Leaf),
}

impl<K, V, H> BTree<K, V, H>
where
    K: Content<H> + Ord,
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

        match BTreeSearch::new(ann_key).select(self.children()) {
            Some(i) => match &mut *self.0[i].inner_mut()? {
                HandleMut::None => unreachable!(),
                HandleMut::Leaf((key, _)) => {
                    if key == ann_key {
                        action = Action::Replace(i);
                    } else if *key > *ann_key {
                        action = Action::Insert(i);
                    } else if i + 1 == len {
                        action = Action::Insert(i + 1);
                    }
                }
                HandleMut::Node(n) => {
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
            None => action = Action::Insert(len),
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
        enum Action<L> {
            Noop,
            Remove(usize),
            Merge(usize, L),
        }
        // The default action
        let mut action = Action::Noop;

        match BTreeSearch::new(k).select(self.children()) {
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
                                RemoveResult::Merge(removed_leaf) => {
                                    action = Action::Merge(i, removed_leaf)
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
                    match &mut *self.0[i - 1].inner_mut()? {
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
                    match &mut *self.0[i + 1].inner_mut()? {
                        HandleMut::Node(n) => {
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

impl<'a, O, K, V, H> Map<'a, O, K, V, H> for BTree<K, V, H>
where
    K: Content<H> + Ord + Borrow<O>,
    V: Content<H>,
    H: ByteHash,
    O: Ord + ?Sized + 'a,
{
    type KeySearch = BTreeSearch<'a, K, O>;
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
            h.insert(i, i).unwrap();
        }
        for i in 0..bigger {
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn insert_remove() {
        let mut h = BTree::<_, _, Blake2b>::new();
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
        let mut h = BTree::<_, _, Blake2b>::new();
        let bigger = 1024;
        for i in 0..bigger {
            h.insert(i, i).unwrap();
        }
        for i in 0..bigger {
            let i = bigger - i - 1;
            assert_eq!(h.remove(&i).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn borrowed_keys() {
        let mut map = BTree::<String, u8, Blake2b>::new();
        map.insert("hello".into(), 8).unwrap();
        assert_eq!(*map.get("hello").unwrap().unwrap(), 8);
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
