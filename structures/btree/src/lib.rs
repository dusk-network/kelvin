use std::borrow::Borrow;
use std::io;
use std::iter::Iterator;

use kelvin::{
    annotation,
    annotations::{Cardinality, Counter, Key, KeyType},
    Annotation, ByteHash, Combine, Compound, Content, Handle, Method, Sink,
    Source, ValPath, ValPathMut, ValRef, ValRefMut,
};

const N: usize = 2;

/// A hash array mapped trie
#[derive(Clone)]
pub struct BTree<K, V, H: ByteHash>([Handle<Self, H>; N * 2 - 1])
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
        key: Key<K>,
        count: Cardinality<U>,
    }
    where
        K: KeyType,
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
    C::Annotation: Borrow<Key<K>>,
    H: ByteHash,
    K: Clone + Ord,
{
    fn select(&mut self, handles: &[Handle<C, H>]) -> Option<usize> {
        for (i, h) in handles.iter().enumerate() {
            if let Some(ann) = h.annotation() {
                let handle_key: &Key<K> = (*ann).borrow();
                if self.0 >= handle_key {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl<K, V, H> BTree<K, V, H>
where
    K: Content<H> + Ord + Eq,
    V: Content<H>,
    H: ByteHash,
{
    /// Creates a new BTree
    pub fn new() -> Self {
        BTree(Default::default())
    }

    /// Insert key-value pair into the BTree, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> io::Result<Option<V>> {
        unimplemented!()
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

    /// Remove element with given key, returning it.
    pub fn remove(&mut self, k: &K) -> io::Result<Option<V>> {
        unimplemented!()
    }
}

impl<K, V, H> Content<H> for BTree<K, V, H>
where
    K: Content<H> + Ord,
    V: Content<H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        for h in &mut self.0 {
            h.persist(sink)?
        }
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut b = BTree::default();
        for i in 0..N {
            b.0[i] = Handle::restore(source)?;
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
        for i in 0..1000 {
            h.insert(i, i).unwrap();
            assert_eq!(*h.get(&i).unwrap().unwrap(), i);
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
