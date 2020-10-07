// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use core::borrow::{Borrow, BorrowMut};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use canonical::{Canon, Store};
use canonical_derive::Canon;
use owning_ref::{OwningRef, OwningRefMut, StableAddress};

use crate::branch::{Branch, BranchMut};
use crate::compound::Compound;
use crate::iter::{LeafIter, LeafIterMut};
use crate::search::{First, Method};

/// A Key-value pair type
#[derive(Clone, Debug, PartialEq, Eq, Hash, Canon)]
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
pub struct ValPath<'a, K, V, C, S, const N: usize>
where
    C: Compound<S, N>,
    S: Store,
{
    branch: Branch<'a, C, S, N>,
    _marker: PhantomData<(K, V)>,
}

/// A path to a mutable leaf in a map Compound
pub struct ValPathMut<'a, K, V, C, S, const N: usize>
where
    C: Compound<S, N>,
    S: Store,
{
    branch: BranchMut<'a, C, S, N>,
    _marker: PhantomData<(K, V)>,
}

// The following
unsafe impl<'a, K, V, C, S, const N: usize> StableAddress
    for ValPath<'a, K, V, C, S, N>
where
    C: Compound<S, N>,
    C::Leaf: Borrow<V>,
    S: Store,
{
}
unsafe impl<'a, K, V, C, S, const N: usize> StableAddress
    for ValPathMut<'a, K, V, C, S, N>
where
    C: Compound<S, N>,
    C::Leaf: Borrow<V>,
    S: Store,
{
}

impl<'a, K, V, C, S, const N: usize> ValPath<'a, K, V, C, S, N>
where
    C: Compound<S, N>,
    S: Store,
{
    /// Creates a new `ValPath`, when leaf is found and key matches
    pub fn new<M>(node: &'a C, method: &mut M) -> Result<Option<Self>, S::Error>
    where
        M: Method<C, S, N>,
    {
        Ok(Branch::new(node, method)?
            .filter(|branch| branch.exact())
            .map(|branch| ValPath {
                branch,
                _marker: PhantomData,
            }))
    }
}

impl<'a, K, V, C, S, const N: usize> ValPathMut<'a, K, V, C, S, N>
where
    C: Compound<S, N>,
    S: Store,
{
    /// Creates a new `ValPathMut`
    pub fn new<M>(
        node: &'a mut C,
        method: &mut M,
    ) -> Result<Option<Self>, S::Error>
    where
        M: Method<C, S, N>,
    {
        Ok(BranchMut::new(node, method)?
            .filter(|branch| branch.exact())
            .map(|branch| ValPathMut {
                branch,
                _marker: PhantomData,
            }))
    }
}

impl<'a, K, V, C, S, const N: usize> Deref for ValPath<'a, K, V, C, S, N>
where
    C: Compound<S, N>,
    C::Leaf: Borrow<V>,
    S: Store,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        (*self.branch).borrow()
    }
}

impl<'a, K, V, C, S, const N: usize> Deref for ValPathMut<'a, K, V, C, S, N>
where
    C: Compound<S, N>,
    C::Leaf: Borrow<V>,
    S: Store,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        (*self.branch).borrow()
    }
}

impl<'a, K, V, C, S, const N: usize> DerefMut for ValPathMut<'a, K, V, C, S, N>
where
    C: Compound<S, N>,
    C::Leaf: BorrowMut<V>,
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        (*self.branch).borrow_mut()
    }
}

pub struct ValIter<'a, C, V, M, S, const N: usize>(
    LeafIter<'a, C, M, S, N>,
    PhantomData<V>,
)
where
    C: Compound<S, N>,
    S: Store;

pub struct ValIterMut<'a, C, V, M, S, const N: usize>(
    LeafIterMut<'a, C, M, S, N>,
    PhantomData<V>,
)
where
    C: Compound<S, N>,
    S: Store;

pub struct KeyIter<'a, C, K, V, M, S, const N: usize>(
    LeafIter<'a, C, M, S, N>,
    PhantomData<(K, V)>,
)
where
    C: Compound<S, N>,
    S: Store;

/// Compound can be iterated over like a map
pub trait ValIterable<V, S, const N: usize>
where
    Self: Compound<S, N>,
    S: Store,
{
    /// Iterator over the values of the map
    fn values(&self) -> ValIter<Self, V, First, S, N>;

    /// Iterator over the mutable values of the map
    fn values_mut(&mut self) -> ValIterMut<Self, V, First, S, N>;
}

/// Compound can have its values iterated over
impl<C, V, S, const N: usize> ValIterable<V, S, N> for C
where
    C: Compound<S, N>,
    S: Store,
{
    fn values(&self) -> ValIter<Self, V, First, S, N> {
        ValIter(LeafIter::Initial(self, First), PhantomData)
    }

    fn values_mut(&mut self) -> ValIterMut<Self, V, First, S, N> {
        ValIterMut(LeafIterMut::Initial(self, First), PhantomData)
    }
}

impl<'a, C, V, M, S, const N: usize> Iterator for ValIter<'a, C, V, M, S, N>
where
    C: Compound<S, N>,
    C::Leaf: Borrow<V>,
    M: 'a + Method<C, S, N>,
    V: 'a,
    S: Store,
{
    type Item = Result<&'a V, S::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok(leaf)) => Some(Ok(leaf.borrow())),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

impl<'a, C, V, M, S, const N: usize> Iterator for ValIterMut<'a, C, V, M, S, N>
where
    C: Compound<S, N>,
    C::Leaf: BorrowMut<V>,
    M: 'a + Method<C, S, N>,
    V: 'a,
    S: Store,
{
    type Item = Result<&'a mut V, S::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok(leaf)) => Some(Ok(leaf.borrow_mut())),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

impl<'a, C, K, V, M, S, const N: usize> Iterator
    for KeyIter<'a, C, K, V, M, S, N>
where
    C: Compound<S, N>,
    C::Leaf: Borrow<K>,
    M: 'a + Method<C, S, N>,
    K: 'a,
    V: 'a,
    S: Store,
{
    type Item = Result<&'a K, S::Error>;

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
pub trait Map<'a, K, O, V, S, const N: usize>
where
    Self: Compound<S, N>,
    Self::Leaf: Borrow<V>,
    K: Borrow<O> + 'a,
    O: ?Sized + 'a,
    S: Store,
{
    /// The method used to search for keys in the structure
    type KeySearch: Method<Self, S, N> + From<&'a O>;

    /// Returns a reference to a value in the map, if any
    fn get(
        &self,
        k: &'a O,
    ) -> Result<Option<ValPath<K, V, Self, S, N>>, S::Error> {
        ValPath::new(self, &mut Self::KeySearch::from(k.borrow()))
    }

    /// Returns a reference to a mutable value in the map, if any
    fn get_mut(
        &mut self,
        k: &'a O,
    ) -> Result<Option<ValPathMut<K, V, Self, S, N>>, S::Error> {
        ValPathMut::new(self, &mut Self::KeySearch::from(k.borrow()))
    }
}
