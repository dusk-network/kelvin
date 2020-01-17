use std::borrow::Borrow;
use std::io::{self};
use std::mem;

mod nibbles;

use nibbles::{AsNibbles, NibbleBuf, Nibbles};

use kelvin::{
    annotation,
    annotations::{Cardinality, MaxKey, MaxKeyType},
    ByteHash, Compound, Content, Handle, HandleMut, HandleType, Map, Method,
    SearchIn, SearchResult, Sink, Source,
};

const N_BUCKETS: usize = 17;

// this does not work for some reason
// const MAX_KEY_LEN: usize = 2 ^ 14;
//
// so be literal
const MAX_KEY_LEN: usize = 16_384;

/// A Prefix tree
pub struct Radix<K, V, H>
where
    Self: Compound<H>,
    H: ByteHash,
{
    handles: [Handle<Self, H>; N_BUCKETS],
    meta: [NibbleBuf; N_BUCKETS],
}

impl<K: 'static, V: Content<H>, H: ByteHash> Clone for Radix<K, V, H> {
    fn clone(&self) -> Self {
        Radix {
            handles: self.handles.clone(),
            meta: self.meta.clone(),
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
            meta: Default::default(),
        }
    }
}

impl<C, H> Method<C, H> for Nibbles<'_>
where
    C: Compound<H>,
    H: ByteHash,
    C::Meta: AsNibbles,
{
    fn select(&mut self, handles: SearchIn<C, H>) -> SearchResult {
        println!("SELECT {:?}", self);

        for i in 0..17 {
            print!(
                "{} {:?} : {:?} ",
                i,
                handles[i].handle_type(),
                handles.meta()[i].as_nibbles()
            );
        }
        println!();

        let nibble = self.pop_nibble();

        let i = nibble + 1;

        let nibbles = handles.meta()[i].as_nibbles();

        println!("nibbles at {} {:?}", i, nibbles);

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
        println!("insert with search {:?}", search);

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

        println!(
            "looking at {} {:?} with meta {:?} and search {:?}",
            i,
            self.handles[i].handle_type(),
            self.meta()[i],
            search
        );

        let path_len = self.meta[i].len();

        let common: NibbleBuf =
            search.common_prefix(&self.meta[i].as_nibbles()).into();

        println!("common len {}", common.len());

        if path_len == 0 {
            // only empty handles has null meta.
            println!("case a");
            let leaf = Handle::new_leaf(v);
            self.meta[i] = (*search).into();
            self.handles[i] = leaf;
            return Ok(None);
        } else if common.len() == search.len() {
            println!("case b");
            let leaf = Handle::new_leaf(v);
            return Ok(Some(
                mem::replace(&mut self.handles[i], leaf).into_leaf(),
            ));
        } else if common.len() < path_len {
            println!("split");
            // we need to split
            let mut old_path = mem::take(&mut self.meta[i]);

            let mut new_node = Self::new();

            let old_handle = mem::take(&mut self.handles[i]);

            // The path to re-insert the removed handle
            let o = old_path.pop_nibble() + 1;
            old_path.trim_front(common.len());
            new_node.handles[o] = old_handle;
            new_node.meta[o] = old_path;

            println!("o a: {}", o);

            // insert into new node
            println!("recurse here");
            search.trim_front(common.len());
            new_node._insert(search, v)?;

            self.handles[i] = Handle::new_node(new_node);
            self.meta[i] = common;

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

        // match self.0[i].handle_type() {
        //     HandleType::None => {
        //         let mut handle = Handle::new_leaf(v);
        //         *handle.meta_mut() = (*search).into();
        //         self.0[i] = handle;
        //         println!(
        //             "inserted into position {} with meta {:?}",
        //             i,
        //             self.0[i].meta()
        //         );
        //         return Ok(None);
        //     }
        //     HandleType::Leaf => {
        //         let old_leaf_meta = self.0[i].take_meta();
        //         let mut old_leaf_suffix = old_leaf_meta.as_nibbles();
        //         let common: NibbleBuf =
        //             search.common_prefix(&old_leaf_suffix).into();

        //         let mut new_node = Self::new();

        //         search.trim_front(common.len());
        //         new_node._insert(search, v)?;

        //         let old_leaf = mem::take(&mut self.0[i]).into_leaf();
        //         old_leaf_suffix.trim_front(common.len());
        //         new_node._insert(&mut old_leaf_suffix, old_leaf)?;

        //         let mut new_handle = Handle::new_node(new_node);
        //         *new_handle.meta_mut() = common;

        //         self.0[i] = new_handle;

        //         return Ok(None);
        //     }
        //     HandleType::Node => {
        //         let path_len = self.0[i].meta().len();
        //         let common_len =
        //             search.common_prefix(&self.0[i].meta().as_nibbles()).len();

        //         println!("path len {}, common len {}", path_len, common_len);

        //         if path_len <= common_len {
        //             // just follow the branch and recurse
        //             if let HandleMut::Node(node) =
        //                 &mut *self.0[i].inner_mut()?
        //             {
        //                 search.trim_front(common_len);
        //                 return node._insert(search, v);
        //             } else {
        //                 unreachable!()
        //             }
        //         } else {
        //             // we cannot enter the node, since it has a prefix that
        //             // clashes with our search.

        //             // remove old meta, and calculate the common path.

        //             let mut old_meta = self.0[i].take_meta();
        //             let common = search.common_prefix(&old_meta.as_nibbles());

        //             println!("common: {:?}", common);

        //             // remove old node

        //             let mut old_node_handle = mem::take(&mut self.0[i]);

        //             let new_node = Self::new();

        //             // insert

        //             // the meta of the new_node handle will be the old_meta, with the common
        //             // nibbles trimmed off

        //             let mut new_node_handle = Handle::new_node(new_node);
        //             old_meta.trim_front(common.len());

        //             println!("meta of new_node {:?}", old_meta);

        //             *new_node_handle.meta_mut() = old_meta;

        //             self.0[i] = new_node_handle;

        //             unimplemented!()

        //             // // inserting one more node level inbetween this and the current
        //             // // continued path
        //             // let old_path = self.0[i].take_meta();

        //             // println!("old_path {:?}", old_path);

        //             // // setup the new path segments
        //             // let mut path_a = old_path.clone();
        //             // let mut path_b = old_path;

        //             // path_a.trim_front(common_len);
        //             // let keep_back = path_b.len() - common_len;

        //             // println!("path_b pre trim of {}: {:?}", keep_back, path_b);

        //             // path_b.trim_back(keep_back);

        //             // // do the swap

        //             // let mut old_node_handle =
        //             //     mem::replace(&mut self.0[i], Handle::new_empty());

        //             // let mut new_node = Self::new();

        //             // println!("path A {:?}, path B: {:?}", path_a, path_b);

        //             // let pos = path_a.pop_nibble() + 1;

        //             // *old_node_handle.meta_mut() = path_a;
        //             // new_node.0[pos] = old_node_handle;

        //             // search.trim_front(1);
        //             // println!("RECURSE WITH {:?}", search);
        //             // new_node._insert(search, v)?;

        //             // let mut new_node_handle = Handle::new_node(new_node);
        //             // *new_node_handle.meta_mut() = path_b;

        //             // self.0[i] = new_node_handle;

        //             // Ok(None)
        //         }
        //     }
        // }
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
            self.handles[i].persist(sink)?
        }
        // We don't need to store the metadata for the leaf node
        // since it will always be []
        for i in 1..N_BUCKETS - 1 {
            self.meta[i].persist(sink)?
        }
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut handles: [Handle<Self, H>; N_BUCKETS] = Default::default();
        let mut meta: [NibbleBuf; N_BUCKETS] = Default::default();
        for i in 0..N_BUCKETS {
            handles[i] = Handle::restore(source)?;
        }
        for i in 1..N_BUCKETS - 1 {
            meta[i - 1] = NibbleBuf::restore(source)?;
        }
        Ok(Radix { handles, meta })
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
        &mut self.handles
    }

    fn children(&self) -> &[Handle<Self, H>] {
        &self.handles
    }

    fn meta(&self) -> &[Self::Meta] {
        &self.meta
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
        for i in 0u16..1024 {
            let b = i.to_be_bytes();
            println!("-- INSERTING {} --", i);
            h.insert(b, i).unwrap();

            println!("{}", h.draw());

            assert_eq!(*h.get(&b).unwrap().unwrap(), i);
        }
    }

    #[test]
    fn splitting() {
        let mut h = Radix::<_, _, Blake2b>::new();
        h.insert(vec![0x00, 0x00], 0).unwrap();
        println!("{}", h.draw());
        assert_eq!(h.insert(vec![0x00, 0x00], 0).unwrap(), Some(0));
        println!("{}", h.draw());
        h.insert(vec![0x00, 0x10], 8).unwrap();
        println!("{}", h.draw());

        assert_eq!(*h.get(&vec![0x00, 0x00]).unwrap().unwrap(), 0);
        // assert_eq!(*h.get(&[0x00, 0x08]).unwrap().unwrap(), 8);
    }

    quickcheck_map!(|| Radix::new());
}
