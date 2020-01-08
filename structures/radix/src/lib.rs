use std::borrow::Borrow;
use std::io::{self, Write};
use std::mem;

use kelvin::{
    annotation,
    annotations::{Cardinality, MaxKey, MaxKeyType},
    ByteHash, Compound, Content, Handle, HandleMut, HandleOwned, HandleRef,
    HandleType, Map, Method, Sink, Source,
};
use smallvec::SmallVec;

const N_BUCKETS: usize = 17;

// this does not work for some reason
// const MAX_KEY_LEN: usize = 2 ^ 14;
//
// so be literal
const MAX_KEY_LEN: usize = 16_384;

// The maximum number of bytes to represent segments inline, before spilling to heap
//
// This is to make a compromize between wasting space in the memory representation,
// and avoiding unnecessary heap searches.
const SEGMENT_SIZE: usize = 4;

/// A hash array mapped trie
#[derive(Clone)]
pub struct Radix<K, V, H: ByteHash>([Handle<Self, H>; N_BUCKETS])
where
    Self: Compound<H>;

impl<K: Content<H>, V: Content<H>, H: ByteHash> Default for Radix<K, V, H> {
    fn default() -> Self {
        Radix(Default::default())
    }
}

pub struct Nibbles<'a> {
    bytes: &'a [u8],
    truncate_front: bool,
    truncate_back: bool,
}

impl<'a> Nibbles<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Nibbles {
            bytes,
            truncate_front: false,
            truncate_back: false,
        }
    }

    fn get(&self, idx: usize) -> usize {
        let byte_index = idx / 2;

        let byte = self.bytes[byte_index];

        // if we have front offset, we want the other nibble
        (if (idx % 2 == 0) ^ self.truncate_front {
            byte & 0x0F
        } else {
            (byte & 0xF0) >> 4
        }) as usize
    }

    fn split(&self, i: usize) -> Self {
        let start_byte = i / 2;

        Nibbles {
            bytes: &self.bytes[start_byte..],
            // true if i is odd
            truncate_front: i % 2 == 1,
            truncate_back: false,
        }
    }

    fn len(&self) -> usize {
        (self.bytes.len() / 2)
            - if self.truncate_front { 1 } else { 0 }
            - if self.truncate_back { 1 } else { 0 }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct NibbleBuf {
    bytes: SmallVec<[u8; SEGMENT_SIZE]>,
    truncate_front: bool,
    truncate_back: bool,
}

impl NibbleBuf {
    fn len(&self) -> usize {
        (self.bytes.len() / 2)
            - if self.truncate_front { 1 } else { 0 }
            - if self.truncate_back { 1 } else { 0 }
    }
}

impl Into<NibbleBuf> for Nibbles<'_> {
    fn into(self) -> NibbleBuf {
        NibbleBuf {
            bytes: SmallVec::from_slice(&self.bytes),
            truncate_front: self.truncate_front,
            truncate_back: self.truncate_back,
        }
    }
}

// The PathSegment is encoded as length in a `u16`, using the most significant
// bit as the bool value.
impl<H> Content<H> for NibbleBuf
where
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        let mut len = self.bytes.len() as u16;

        if self.truncate_front {
            // set most significant bit
            len |= 1 << 15
        } else {
            // clear most significant bit
            len &= !(1 << 15)
        }

        if self.truncate_back {
            // set second most significant bit
            len |= 1 << 14
        } else {
            // clear second most significant bit
            len &= !(1 << 14)
        }

        len.persist(sink)?;
        sink.write_all(&self.bytes)
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut len = u16::restore(source)?;
        let truncate_front = (len >> 15) == 1;
        let truncate_back = (len >> 14) == 1;
        // clear the two most significant bits
        len &= !(1 << 14);
        len &= !(1 << 15);
        let len = len as usize;

        let mut smallvec = SmallVec::with_capacity(len);
        for _ in 0..len {
            smallvec.push(u8::restore(source)?)
        }

        Ok(NibbleBuf {
            bytes: smallvec,
            truncate_front,
            truncate_back,
        })
    }
}

pub struct RadixSearch<'a> {
    nibbles: Nibbles<'a>,
    offset: usize,
}

impl<'a> RadixSearch<'a> {
    fn new<A: AsRef<[u8]> + ?Sized + 'a>(a: &'a A) -> Self {
        RadixSearch {
            nibbles: Nibbles::new(a.as_ref()),
            offset: 0,
        }
    }

    fn suffix_len(&self) -> usize {
        self.nibbles.len() - self.offset
    }

    fn into_suffix(&self) -> NibbleBuf {
        self.nibbles.split(self.offset).into()
    }

    fn current_nibble(&self) -> usize {
        self.nibbles.get(self.offset)
    }
}

impl<'a, T> From<&'a T> for RadixSearch<'a>
where
    T: AsRef<[u8]> + ?Sized,
{
    fn from(t: &'a T) -> Self {
        RadixSearch {
            nibbles: Nibbles {
                bytes: t.as_ref(),
                truncate_front: false,
                truncate_back: false,
            },
            offset: 0,
        }
    }
}

impl<C, H> Method<C, H> for RadixSearch<'_>
where
    C: Compound<H>,
    H: ByteHash,
    C::Meta: Borrow<NibbleBuf>,
{
    fn select(&mut self, handles: &[Handle<C, H>]) -> Option<usize> {
        // Are we att the correct leaf?
        if self.suffix_len() == 0 {
            return Some(0);
        }

        // else find the next hop
        let nibble = self.current_nibble();
        // offset past the 0 leaf node
        let i = nibble as usize + 1;
        // increment key by the length of the saved NibbleBuf shared suffix plus
        // one for the nibble we implicitly store in our choice of branch
        self.offset += handles[i].meta().borrow().len() + 1;
        Some(i)
    }
}

impl<K, V, H> Radix<K, V, H>
where
    K: Content<H> + AsRef<[u8]> + Eq,
    V: Content<H>,
    H: ByteHash,
{
    /// Creates a new Radix
    pub fn new() -> Self {
        Default::default()
    }

    /// Insert key-value pair into the Radix, optionally returning expelled value
    pub fn insert(&mut self, k: K, v: V) -> io::Result<Option<V>> {
        #[derive(Debug)]
        enum Action {
            Insert { at: usize, suffix: NibbleBuf },
            Replace(usize),
            Split(usize),
        };

        let action = {
            let mut search = RadixSearch::new(&k);
            let at =
                search.select(self.children()).expect("Always returns Some");

            match self.0[at].inner()? {
                HandleRef::None => Action::Insert {
                    at,
                    suffix: search.into_suffix(),
                },
                HandleRef::Leaf((leaf_k, leaf_v)) => {
                    if *leaf_k == k {
                        Action::Replace(at)
                    } else {
                        Action::Split(at)
                    }
                }
                HandleRef::Node(_) => unimplemented!("node {}", at),
            }
        };

        match action {
            Action::Insert { at, suffix } => {
                let mut leaf = Handle::new_leaf((k, v));
                *leaf.meta_mut() = suffix;
                self.0[at] = leaf;
                return Ok(None);
            }
            Action::Replace(at) => {
                let mut leaf = Handle::new_leaf((k, v));
                // move the suffix of the key
                let suffix = self.0[at].take_meta();
                // into the replacement node
                *leaf.meta_mut() = suffix;
                // replace node
                let replaced = mem::replace(&mut self.0[at], leaf);
                // return the old value
                Ok(Some(replaced.into_leaf().1))
            }
            Action::Split(at) => {
                unimplemented!("split {}", at);
            }
            action => unimplemented!("unimplemented action {:?}", action),
        }
    }

    /// Remove element with given key, returning it.
    pub fn remove(&mut self, k: &K) -> io::Result<Option<V>> {
        enum Action {
            Remove(usize),
            Placeholder,
        }

        unimplemented!()
    }
}

impl<K, V, H> Content<H> for Radix<K, V, H>
where
    K: Content<H>,
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

impl<'a, O, K, V, H> Map<'a, O, K, V, H> for Radix<K, V, H>
where
    K: Content<H> + AsRef<[u8]> + Eq + Borrow<O>,
    V: Content<H>,
    H: ByteHash,
    O: Eq + AsRef<[u8]> + 'a + ?Sized,
{
    type KeySearch = RadixSearch<'a>;
}

annotation! {
    struct RadixAnnotation<K> {
        cardinality: Cardinality<u64>,
        key: MaxKey<K>,
    } where K: MaxKeyType
}

impl<K, V, H> Compound<H> for Radix<K, V, H>
where
    H: ByteHash,
    K: Content<H>,
    V: Content<H>,
{
    type Leaf = (K, V);
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

    use kelvin::quickcheck_map;
    use kelvin::Blake2b;

    #[test]
    fn trivial_map() {
        let mut h = Radix::<_, _, Blake2b>::new();
        h.insert(String::from("hello"), String::from("world"))
            .unwrap();
        assert_eq!(*h.get("hello").unwrap().unwrap(), "world");
    }

    #[test]
    fn path_segment_encoding() {
        let dir = tempdir().unwrap();
        let store = Store::<Blake2b>::new(&dir.path()).unwrap();

        for len in 0..32 {
            let mut smallvec = SmallVec::new();
            for i in 0..len {
                smallvec.push(i);

                let mut a = NibbleBuf {
                    bytes: smallvec.clone(),
                    truncate_front: false,
                    truncate_back: false,
                };
                let mut b = NibbleBuf {
                    bytes: smallvec.clone(),
                    truncate_front: true,
                    truncate_back: false,
                };
                let mut c = NibbleBuf {
                    bytes: smallvec.clone(),
                    truncate_front: false,
                    truncate_back: true,
                };
                let mut d = NibbleBuf {
                    bytes: smallvec.clone(),
                    truncate_front: false,
                    truncate_back: false,
                };

                let snap_a = store.persist(&mut a).unwrap();
                let a2 = store.restore(&snap_a).unwrap();
                assert_eq!(a, a2);

                let snap_b = store.persist(&mut b).unwrap();
                let b2 = store.restore(&snap_b).unwrap();
                assert_eq!(b, b2);

                let snap_c = store.persist(&mut c).unwrap();
                let c2 = store.restore(&snap_c).unwrap();
                assert_eq!(c, c2);

                let snap_d = store.persist(&mut d).unwrap();
                let d2 = store.restore(&snap_d).unwrap();
                assert_eq!(d, d2);
            }
        }
    }

    #[test]
    fn bigger_map() {
        let mut h = Radix::<_, _, Blake2b>::new();
        for i in 0u16..1000 {
            println!("i {}", i);
            let b = i.to_be_bytes();
            h.insert(b, i).unwrap();
            assert_eq!(*h.get(&b).unwrap().unwrap(), i);
        }
    }

    quickcheck_map!(|| Radix::new());
}
