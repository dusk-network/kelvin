use std::borrow::Borrow;
use std::io::{self};
use std::mem;

mod nibbles;

use nibbles::{AsNibbles, NibbleBuf, Nibbles};

use kelvin::{
    annotation,
    annotations::{Cardinality, MaxKey, MaxKeyType},
    ByteHash, Compound, Content, Handle, HandleMut, HandleType, Map, Method,
    SearchResult, Sink, Source,
};

const N_BUCKETS: usize = 17;

const MAX_KEY_LEN: usize = core::u16::MAX as usize / 2;

/// A Prefix tree
pub struct Radix<K, V, H>
where
    Self: Compound<H>,
    H: ByteHash,
{
    handles: [Handle<Self, H>; N_BUCKETS],
    prefixes: [NibbleBuf; N_BUCKETS - 1],
}

impl<K: 'static, V: Content<H>, H: ByteHash> Clone for Radix<K, V, H> {
    fn clone(&self) -> Self {
        Radix {
            handles: self.handles.clone(),
            prefixes: self.prefixes.clone(),
        }
    }
}

impl<K, V, H> Default for Radix<K, V, H>
where
    K: 'static,
    V: Content<H>,
    H: ByteHash,
{
    fn default() -> Self {
        Radix {
            handles: Default::default(),
            prefixes: Default::default(),
        }
    }
}

impl<K, V, H> Method<Radix<K, V, H>, H> for Nibbles<'_>
where
    K: 'static,
    V: Content<H>,
    H: ByteHash,
{
    fn select(&mut self, compound: &Radix<K, V, H>, _: usize) -> SearchResult {
        let nibble = self.pop_nibble();

        let i = nibble + 1;

        let nibbles = compound.prefixes[i - 1].as_nibbles();

        let common_len = nibbles.common_prefix(self).len();
        let search_len = self.len();

        let result = {
            if common_len == search_len {
                SearchResult::Leaf(i)
            } else if common_len <= search_len {
                // if we're descending, we'll trim the search
                if compound.handles[i].handle_type() == HandleType::Node {
                    self.trim_front(common_len)
                }
                SearchResult::Path(i)
            } else {
                SearchResult::None
            }
        };
        result
    }
}

enum Removed<L> {
    None,
    Leaf(L),
    Collapse(L, L, NibbleBuf, usize),
}

impl<K, V, H> Radix<K, V, H>
where
    K: AsRef<[u8]> + Eq + 'static,
    V: Content<H>,
    H: ByteHash,
{
    /// Creates a new Radix
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, k: K, v: V) -> io::Result<Option<V>> {
        debug_assert!(k.as_ref().len() <= MAX_KEY_LEN);
        let mut search = Nibbles::new(k.as_ref());
        self._insert(&mut search, v)
    }

    /// Insert key-value pair into the Radix, optionally returning expelled value
    fn _insert(&mut self, search: &mut Nibbles, v: V) -> io::Result<Option<V>> {
        // Leaf case, for keys that are subsets of other keys
        if search.len() == 0 {
            match self.handles[0].handle_type() {
                HandleType::None => {
                    self.handles[0] = Handle::new_leaf(v);
                    return Ok(None);
                }
                HandleType::Leaf => {
                    return Ok(Some(
                        mem::replace(&mut self.handles[0], Handle::new_leaf(v))
                            .into_leaf(),
                    ));
                }
                HandleType::Node => unreachable!("Invalid in Leaf position"),
            }
        }

        let i = search.pop_nibble() + 1;

        let path_len = self.prefixes[i - 1].len();

        let common: NibbleBuf = search
            .common_prefix(&self.prefixes[i - 1].as_nibbles())
            .into();

        if self.handles[i].handle_type() == HandleType::None {
            let leaf = Handle::new_leaf(v);
            self.prefixes[i - 1] = (*search).into();
            self.handles[i] = leaf;
            return Ok(None);
        } else if common.len() == search.len() {
            // found the leaf
            let leaf = Handle::new_leaf(v);
            return Ok(Some(
                mem::replace(&mut self.handles[i], leaf).into_leaf(),
            ));
        } else if common.len() < path_len {
            // we need to split
            let mut old_path = mem::take(&mut self.prefixes[i - 1]);

            let mut new_node = Self::new();

            let old_handle = mem::take(&mut self.handles[i]);

            // The path to re-insert the removed handle
            let o = old_path.pop_nibble() + 1;
            old_path.trim_front(common.len());
            new_node.handles[o] = old_handle;
            new_node.prefixes[o - 1] = old_path;

            // insert into new node
            search.trim_front(common.len());
            new_node._insert(search, v)?;

            self.handles[i] = Handle::new_node(new_node);
            self.prefixes[i - 1] = common;

            return Ok(None);
        } else {
            // recurse
            if let HandleMut::Node(ref mut node) =
                *self.handles[i].inner_mut()?
            {
                search.trim_front(common.len());
                node._insert(search, v)
            } else {
                unreachable!()
            }
        }
    }

    /// Remove element with given key, returning it.
    pub fn remove(&mut self, k: &K) -> io::Result<Option<V>> {
        debug_assert!(k.as_ref().len() <= MAX_KEY_LEN);
        let mut search = Nibbles::new(k.as_ref());
        match self._remove(&mut search, 0)? {
            Removed::None => Ok(None),
            Removed::Leaf(l) => Ok(Some(l)),
            _ => unreachable!(),
        }
    }

    fn _remove(
        &mut self,
        search: &mut Nibbles,
        depth: usize,
    ) -> io::Result<Removed<V>> {
        let collapse = {
            if search.len() == 0 {
                match self.handles[0].handle_type() {
                    HandleType::None => {
                        return Ok(Removed::None);
                    }
                    HandleType::Leaf => {
                        return Ok(Removed::Leaf(
                            mem::take(&mut self.handles[0]).into_leaf(),
                        ));
                    }
                    HandleType::Node => {
                        unreachable!("Invalid in Leaf position")
                    }
                }
            }

            let i = search.pop_nibble() + 1;

            let path_len = self.prefixes[i - 1].len();

            let common: NibbleBuf = search
                .common_prefix(&self.prefixes[i - 1].as_nibbles())
                .into();

            if self.handles[i].handle_type() == HandleType::None {
                return Ok(Removed::None);
            } else if common.len() == search.len() {
                // found the leaf
                self.prefixes[i - 1] = Default::default();
                let removed = mem::take(&mut self.handles[i]).into_leaf();
                if depth > 0 {
                    if let Some((l, prefixes, inner_i)) =
                        self.remove_singleton()
                    {
                        return Ok(Removed::Collapse(
                            removed, l, prefixes, inner_i,
                        ));
                    }
                }
                return Ok(Removed::Leaf(removed));
            } else if common.len() < path_len {
                // nothing here
                return Ok(Removed::None);
            } else {
                // recurse
                if let HandleMut::Node(ref mut node) =
                    *self.handles[i].inner_mut()?
                {
                    search.trim_front(common.len());
                    match node._remove(search, depth + 1)? {
                        Removed::Collapse(
                            removed,
                            reinsert,
                            nibbles,
                            inner_i,
                        ) => (removed, reinsert, nibbles, i, inner_i),

                        result => return Ok(result),
                    }
                } else {
                    unreachable!()
                }
            }
        };

        // if we get here, we're in the collapse branch.

        let (removed, reinsert, nibbles, i, inner_i) = collapse;

        if inner_i > 0 {
            self.prefixes[i - 1].push(inner_i - 1);
        }

        self.prefixes[i - 1].append(&nibbles);

        self.handles[i] = Handle::new_leaf(reinsert);

        // recursion needed?

        return Ok(Removed::Leaf(removed));
    }

    fn remove_singleton(&mut self) -> Option<(V, NibbleBuf, usize)> {
        let mut singleton = None;

        for (i, child) in self.handles.iter().enumerate() {
            match (child.handle_type(), singleton) {
                (HandleType::None, _) => (),
                (HandleType::Leaf, None) => singleton = Some(i),
                (HandleType::Leaf, Some(_)) => return None,
                (HandleType::Node, _) => return None,
            }
        }
        if let Some(idx) = singleton {
            Some((
                mem::take(&mut self.handles[idx]).into_leaf(),
                mem::take(&mut self.prefixes[idx - 1]),
                idx,
            ))
        } else {
            None
        }
    }
}

impl<K, V, H> Content<H> for Radix<K, V, H>
where
    K: 'static,
    V: Content<H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        for i in 0..N_BUCKETS {
            self.handles[i].persist(sink)?
        }
        // We don't need to store the prefixesdata for the leaf node
        // since it will always be []
        for i in 0..N_BUCKETS - 1 {
            self.prefixes[i].persist(sink)?
        }
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut handles: [Handle<Self, H>; N_BUCKETS] = Default::default();
        let mut prefixes: [NibbleBuf; N_BUCKETS - 1] = Default::default();
        for i in 0..N_BUCKETS {
            handles[i] = Handle::restore(source)?;
        }
        for i in 0..N_BUCKETS - 1 {
            prefixes[i] = NibbleBuf::restore(source)?;
        }
        Ok(Radix { handles, prefixes })
    }
}

impl<'a, K, O, V, H> Map<'a, K, O, V, H> for Radix<K, V, H>
where
    K: AsRef<[u8]> + Eq + Borrow<O> + 'static,
    V: Content<H>,
    H: ByteHash,
    O: Eq + AsRef<[u8]> + 'a + ?Sized,
{
    type KeySearch = Nibbles<'a>;
}

annotation! {
    struct RadixAnnotation<K> {
        cardinality: Cardinality<u64>,
        key: MaxKey<K>,
    } where K: MaxKeyType
}

impl<K, V, H> Compound<H> for Radix<K, V, H>
where
    K: 'static,
    V: Content<H>,
    H: ByteHash,
{
    type Leaf = V;
    type Annotation = Cardinality<u64>;

    fn children_mut(&mut self) -> &mut [Handle<Self, H>] {
        &mut self.handles
    }

    fn children(&self) -> &[Handle<Self, H>] {
        &self.handles
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use kelvin::{quickcheck_map, tests::CorrectEmptyState, Blake2b};

    #[test]
    fn trivial_map() {
        let mut h = Radix::<_, _, Blake2b>::new();
        h.insert(String::from("hello"), String::from("world"))
            .unwrap();
        assert_eq!(*h.get("hello").unwrap().unwrap(), "world");
    }

    #[test]
    fn bigger_map() {
        let mut h = Radix::<_, _, Blake2b>::new();
        for i in 0u16..1024 {
            let b = i.to_be_bytes();
            assert_eq!(h.insert(b, i).unwrap(), None);
            assert_eq!(*h.get(&b).unwrap().unwrap(), i);
            assert_eq!(h.insert(b, i).unwrap(), Some(i));
        }
    }

    #[test]
    fn insert_remove() {
        let mut h = Radix::<_, _, Blake2b>::new();
        let n = 1024;
        for i in 0u16..n {
            let b = i.to_be_bytes();
            h.insert(b, i).unwrap();
        }
        for i in 0u16..n {
            let b = i.to_be_bytes();
            assert_eq!(h.remove(&b).unwrap(), Some(i));
        }
    }

    #[test]
    fn bigger_map_random() {
        const N: usize = 1024;
        let mut keys = [0u64; N];

        for i in 0..N as u64 {
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            let key = hasher.finish();
            keys[i as usize] = key;
        }

        let mut h = Radix::<_, _, Blake2b>::new();
        for key in keys.iter() {
            let b = key.to_be_bytes();
            h.insert(b, *key).unwrap();
            assert_eq!(*h.get(&b).unwrap().unwrap(), *key);
        }
    }

    #[test]
    #[ignore]
    fn different_length_keys() {
        let keys = [
            "cat",
            "mouse",
            "m",
            "dog",
            "elephant",
            "hippopotamus",
            "giraffe",
            "eel",
            "spider",
            "bald eagle",
            "",
        ];

        let mut h = Radix::<_, _, Blake2b>::new();

        for i in 0..keys.len() {
            h.insert(keys[i], i as u16).unwrap();
        }

        for i in 0..keys.len() {
            assert_eq!(*h.get(keys[i]).unwrap().unwrap(), i as u16);
        }
    }

    #[test]
    fn splitting() {
        let mut h = Radix::<_, _, Blake2b>::new();
        h.insert(vec![0x00, 0x00], 0).unwrap();
        assert_eq!(h.insert(vec![0x00, 0x00], 0).unwrap(), Some(0));
        h.insert(vec![0x00, 0x10], 8).unwrap();

        assert_eq!(*h.get(&vec![0x00, 0x00]).unwrap().unwrap(), 0);
        assert_eq!(*h.get(&vec![0x00, 0x10]).unwrap().unwrap(), 8);
    }

    #[test]
    fn collapse() {
        let mut h = Radix::<_, _, Blake2b>::new();
        h.insert(vec![0x00, 0x00], 0).unwrap();
        h.insert(vec![0x00, 0x10], 0).unwrap();

        assert_eq!(h.remove(&vec![0x00, 0x00]).unwrap(), Some(0));

        assert_eq!(h.remove(&vec![0x00, 0x10]).unwrap(), Some(0));

        h.assert_correct_empty_state();
    }

    quickcheck_map!(|| Radix::new());
}
