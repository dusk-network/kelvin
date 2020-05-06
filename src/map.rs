use std::borrow::{Borrow, BorrowMut};
use std::io;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use bytehash::ByteHash;
use owning_ref::{OwningRef, OwningRefMut, StableAddress};

use crate::branch::{Branch, BranchMut};
use crate::compound::Compound;
use crate::content::Content;
use crate::iter::{LeafIter, LeafIterMut};
use crate::search::{First, Method};
use crate::sink::Sink;
use crate::source::Source;

/// A Key-value pair type
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KV<K, V> {
    /// the key of the pair
    pub key: K,
    /// the value of the pair
    pub val: V,
}

impl<K, V> KV<K, V> {
    /// Create a new key-value pair
    pub fn new(key: K, val: V) -> Self {
        KV { key, val }
    }
}

impl<K, V> Into<(K, V)> for KV<K, V> {
    fn into(self) -> (K, V) {
        (self.key, self.val)
    }
}

impl<K, V, H> Content<H> for KV<K, V>
where
    K: Content<H>,
    V: Content<H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        self.key.persist(sink)?;
        self.val.persist(sink)
    }
    /// Restore the type from a `Source`
    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok(KV {
            key: K::restore(source)?,
            val: V::restore(source)?,
        })
    }
}

impl<K, V> Borrow<V> for KV<K, V> {
    fn borrow(&self) -> &V {
        &self.val
    }
}

impl<K, V> AsRef<K> for KV<K, V> {
    fn as_ref(&self) -> &K {
        &self.key
    }
}

impl<K, V> BorrowMut<V> for KV<K, V> {
    fn borrow_mut(&mut self) -> &mut V {
        &mut self.val
    }
}

/// A path to a leaf in a map Compound
pub struct ValPath<'a, K, V, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    branch: Branch<'a, C, H>,
    _marker: PhantomData<(K, V)>,
}

/// A path to a mutable leaf in a map Compound
pub struct ValPathMut<'a, K, V, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    branch: BranchMut<'a, C, H>,
    _marker: PhantomData<(K, V)>,
}

// The following
unsafe impl<'a, K, V, C, H> StableAddress for ValPath<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: Borrow<V>,
    H: ByteHash,
{
}
unsafe impl<'a, K, V, C, H> StableAddress for ValPathMut<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: Borrow<V>,
    H: ByteHash,
{
}

impl<'a, K, V, C, H> ValPath<'a, K, V, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Creates a new `ValPath`, when leaf is found and key matches
    pub fn new<M>(node: &'a C, method: &mut M) -> io::Result<Option<Self>>
    where
        M: Method<C, H>,
    {
        Ok(Branch::new(node, method)?
            .filter(|branch| branch.exact())
            .map(|branch| ValPath {
                branch,
                _marker: PhantomData,
            }))
    }
}

impl<'a, K, V, C, H> ValPathMut<'a, K, V, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Creates a new `ValPathMut`
    pub fn new<M>(node: &'a mut C, method: &mut M) -> io::Result<Option<Self>>
    where
        M: Method<C, H>,
    {
        Ok(BranchMut::new(node, method)?
            .filter(|branch| branch.exact())
            .map(|branch| ValPathMut {
                branch,
                _marker: PhantomData,
            }))
    }
}

impl<'a, K, V, C, H> Deref for ValPath<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: Borrow<V>,
    H: ByteHash,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        (*self.branch).borrow()
    }
}

impl<'a, K, V, C, H> Deref for ValPathMut<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: Borrow<V>,
    H: ByteHash,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        (*self.branch).borrow()
    }
}

impl<'a, K, V, C, H> DerefMut for ValPathMut<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: BorrowMut<V>,
    H: ByteHash,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        (*self.branch).borrow_mut()
    }
}

pub struct ValIter<'a, C, V, M, H>(LeafIter<'a, C, M, H>, PhantomData<V>)
where
    C: Compound<H>,
    H: ByteHash;

pub struct ValIterMut<'a, C, V, M, H>(LeafIterMut<'a, C, M, H>, PhantomData<V>)
where
    C: Compound<H>,
    H: ByteHash;

pub struct KeyIter<'a, C, K, V, M, H>(
    LeafIter<'a, C, M, H>,
    PhantomData<(K, V)>,
)
where
    C: Compound<H>,
    H: ByteHash;

/// Compound can be iterated over like a map
pub trait ValIterable<V, H>
where
    Self: Compound<H>,
    H: ByteHash,
{
    /// Iterator over the values of the map
    fn values(&self) -> ValIter<Self, V, First, H>;

    /// Iterator over the mutable values of the map
    fn values_mut(&mut self) -> ValIterMut<Self, V, First, H>;
}

/// Compound can have its values iterated over
impl<C, V, H> ValIterable<V, H> for C
where
    C: Compound<H>,
    H: ByteHash,
{
    fn values(&self) -> ValIter<Self, V, First, H> {
        ValIter(LeafIter::Initial(self, First), PhantomData)
    }

    fn values_mut(&mut self) -> ValIterMut<Self, V, First, H> {
        ValIterMut(LeafIterMut::Initial(self, First), PhantomData)
    }
}

impl<'a, C, V, M, H> Iterator for ValIter<'a, C, V, M, H>
where
    C: Compound<H>,
    C::Leaf: Borrow<V>,
    M: 'a + Method<C, H>,
    V: 'a,
    H: ByteHash,
{
    type Item = io::Result<&'a V>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok(leaf)) => Some(Ok(leaf.borrow())),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

impl<'a, C, V, M, H> Iterator for ValIterMut<'a, C, V, M, H>
where
    C: Compound<H>,
    C::Leaf: BorrowMut<V>,
    M: 'a + Method<C, H>,
    V: 'a,
    H: ByteHash,
{
    type Item = io::Result<&'a mut V>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok(leaf)) => Some(Ok(leaf.borrow_mut())),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

impl<'a, C, K, V, M, H> Iterator for KeyIter<'a, C, K, V, M, H>
where
    C: Compound<H>,
    C::Leaf: Borrow<K>,
    M: 'a + Method<C, H>,
    K: 'a,
    V: 'a,
    H: ByteHash,
{
    type Item = io::Result<&'a K>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok(leaf)) => Some(Ok(leaf.borrow())),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

/// Value reference trait to hide generic arguments to users of the library
pub trait ValRef<'a, V: 'a>: Deref<Target = V> + 'a
where
    Self: Sized + StableAddress,
{
    /// Wrap the ValPath in an OwningRef
    fn wrap<V2, F>(self, f: F) -> OwningRef<Self, V2>
    where
        V2: 'a,
        F: for<'r> FnOnce(&'r V) -> &'r V2,
    {
        OwningRef::new(self).map(f)
    }
}
impl<'a, T, V: 'a> ValRef<'a, V> for T where
    T: StableAddress + Deref<Target = V> + 'a
{
}

/// Mutable value reference trait to hide generic arguments to users of the library
pub trait ValRefMut<'a, V>: DerefMut<Target = V> + 'a
where
    Self: Sized + StableAddress,
{
    /// Wrap the ValPathMut in an OwningRef
    fn wrap_mut<V2, F>(self, f: F) -> OwningRefMut<Self, V2>
    where
        V2: 'a,
        F: for<'r> FnOnce(&'r mut V) -> &'r mut V2,
    {
        OwningRefMut::new(self).map_mut(f)
    }
}

impl<'a, T, V> ValRefMut<'a, V> for T where
    T: StableAddress + Deref<Target = V> + 'a + DerefMut + 'a
{
}

/// Collection can be read as a map
pub trait Map<'a, K, O, V, H>
where
    Self: Compound<H>,
    Self::Leaf: Borrow<V>,
    K: Borrow<O> + 'a,
    O: ?Sized + 'a,
    H: ByteHash,
{
    /// The method used to search for keys in the structure
    type KeySearch: Method<Self, H> + From<&'a O>;

    /// Returns a reference to a value in the map, if any
    fn get(&self, k: &'a O) -> io::Result<Option<ValPath<K, V, Self, H>>> {
        ValPath::new(self, &mut Self::KeySearch::from(k.borrow()))
    }

    /// Returns a reference to a mutable value in the map, if any
    fn get_mut(
        &mut self,
        k: &'a O,
    ) -> io::Result<Option<ValPathMut<K, V, Self, H>>> {
        ValPathMut::new(self, &mut Self::KeySearch::from(k.borrow()))
    }
}
