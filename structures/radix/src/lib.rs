use std::borrow::Borrow;
use std::io::{self};
use std::mem;

mod nibbles;

use nibbles::{AsNibbles, NibbleBuf, Nibbles};

use kelvin::{
    annotation,
    annotations::{Cardinality, MaxKey, MaxKeyType},
    ByteHash, Compound, Content, DebugDraw, Handle, HandleMut, HandleType, Map,
    Method, SearchResult, Sink, Source,
};

const N_BUCKETS: usize = 17;

// this does not work for some reason
// const MAX_KEY_LEN: usize = 2 ^ 14;
//
// so be literal
const MAX_KEY_LEN: usize = 16_384;

/// A hash array mapped trie
pub struct Radix<K, V, H: ByteHash>([Handle<Self, H>; N_BUCKETS])
where
    Self: Compound<H>;

impl<K: 'static, V: Content<H>, H: ByteHash> Clone for Radix<K, V, H> {
    fn clone(&self) -> Self {
        Radix(self.0.clone())
    }
}

impl<K, V, H> Default for Radix<K, V, H>
where
    K: 'static,
    V: Content<H>,
    H: ByteHash,
{
    fn default() -> Self {
        Radix(Default::default())
    }
}

impl<C, H> Method<C, H> for Nibbles<'_>
where
    C: Compound<H>,
    H: ByteHash,
    C::Meta: AsNibbles,
{
    fn select(&mut self, handles: &[Handle<C, H>]) -> SearchResult {
        println!("SELECT {:?}", self);

        let nibble = self.pop_nibble();

        let i = nibble + 1;

        let nibbles = handles[i].meta().as_nibbles();

        let common_len = dbg!(nibbles.common_prefix(self)).len();
        let search_len = self.len();

        let result = {
            if common_len == search_len {
                SearchResult::Leaf(i)
            } else if common_len <= search_len {
                // if we're descending, we'll trim the search
                if handles[i].handle_type() == HandleType::Node {
                    self.trim_front(common_len)
                }
                SearchResult::Path(i)
            } else {
                SearchResult::None
            }
        };
        println!("result {:?}", &result);
        result
    }
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
            match self.0[0].handle_type() {
                HandleType::None => {
                    self.0[0] = Handle::new_leaf(v);
                    return Ok(None);
                }
                HandleType::Leaf => {
                    return Ok(Some(
                        mem::replace(&mut self.0[0], Handle::new_leaf(v))
                            .into_leaf(),
                    ))
                }
                HandleType::Node => unreachable!("Invalid in Leaf position"),
            }
        }

        let i = search.pop_nibble() + 1;

        match self.0[i].handle_type() {
            HandleType::None => {
                let mut handle = Handle::new_leaf(v);
                *handle.meta_mut() = (*search).into();
                self.0[i] = handle;
                return Ok(None);
            }
            HandleType::Leaf => {
                let old_leaf_meta = self.0[i].take_meta();
                let mut old_leaf_suffix = old_leaf_meta.as_nibbles();
                let common: NibbleBuf =
                    search.common_prefix(&old_leaf_suffix).into();

                let mut new_node = Self::new();

                search.trim_front(common.len());
                new_node._insert(search, v)?;

                let old_leaf = mem::take(&mut self.0[i]).into_leaf();
                old_leaf_suffix.trim_front(common.len());
                new_node._insert(&mut old_leaf_suffix, old_leaf)?;

                let mut new_handle = Handle::new_node(new_node);
                *new_handle.meta_mut() = common;

                self.0[i] = new_handle;

                return Ok(None);
            }
            HandleType::Node => {
                let common_len =
                    search.common_prefix(&self.0[i].meta().as_nibbles()).len();
                if let HandleMut::Node(node) = &mut *self.0[i].inner_mut()? {
                    search.trim_front(common_len);
                    return node._insert(search, v);
                } else {
                    unreachable!()
                }
            }
            _ => unimplemented!(),
        }
    }

    /// Remove element with given key, returning it.
    pub fn remove(&mut self, _k: &K) -> io::Result<Option<V>> {
        // enum Action {
        //     Remove(usize),
        //     Noop,
        // }

        unimplemented!()
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
            self.0[i].persist(sink)?
        }
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut handles: [Handle<Self, H>; N_BUCKETS] = Default::default();
        for _ in 0..N_BUCKETS {
            handles[0] = Handle::restore(source)?;
        }
        Ok(Radix(handles))
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
    type Meta = NibbleBuf;
    type Annotation = Cardinality<u64>;

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

    use kelvin::{quickcheck_map, Blake2b, DebugDraw};

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
        for i in 0u16..16 {
            let b = i.to_be_bytes();
            println!("-- INSERTING {} --", i);
            h.insert(b, i).unwrap();

            println!("{}", h.draw());

            for test in 0u16..=i {
                println!("-- GETTING {} --", test);
                let t = test.to_be_bytes();
                assert_eq!(*h.get(&t).unwrap().unwrap(), test);
            }
        }
    }

    quickcheck_map!(|| Radix::new());
}
