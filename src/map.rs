use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use bytehash::ByteHash;

use crate::branch::{Branch, BranchMut};
use crate::compound::Compound;
use crate::iter::{LeafIter, LeafIterMut};
use crate::search::{First, Method};

pub trait KVPair<K, V>: Into<(K, V)> + From<(K, V)> {
    fn key(&self) -> &K;
    fn val(&self) -> &V;
    fn val_mut(&mut self) -> &mut V;
    fn into_val(self) -> V;
}

impl<K, V> KVPair<K, V> for (K, V) {
    fn key(&self) -> &K {
        &self.0
    }

    fn val(&self) -> &V {
        &self.1
    }

    fn val_mut(&mut self) -> &mut V {
        &mut self.1
    }

    fn into_val(self) -> V {
        self.1
    }
}

pub struct ValRef<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    H: ByteHash,
{
    branch: Branch<'a, C, H>,
    _marker: PhantomData<(K, V)>,
}

pub struct ValRefMut<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    H: ByteHash,
{
    branch: BranchMut<'a, C, H>,
    _marker: PhantomData<(K, V)>,
}

impl<'a, K, V, C, H> ValRef<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    H: ByteHash,
    K: PartialEq + Eq,
{
    pub fn new<'m, M>(node: &'a C, method: &mut M, key: &K) -> Option<Self>
    where
        M: Method,
    {
        Branch::new(node, method)
            .filter(|b| b.key() == key)
            .map(|branch| ValRef {
                branch,
                _marker: PhantomData,
            })
    }
}

impl<'a, K, V, C, H> ValRefMut<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    H: ByteHash,
    K: PartialEq + Eq,
{
    pub fn new<'m, M>(node: &'a mut C, method: &mut M, key: &K) -> Option<Self>
    where
        M: Method,
    {
        BranchMut::new(node, method)
            .filter(|b| b.key() == key)
            .map(|branch| ValRefMut {
                branch,
                _marker: PhantomData,
            })
    }
}

impl<'a, K, V, C, H> Deref for ValRef<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    H: ByteHash,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        self.branch.val()
    }
}

impl<'a, K, V, C, H> Deref for ValRefMut<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    H: ByteHash,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        self.branch.val()
    }
}

impl<'a, K, V, C, H> DerefMut for ValRefMut<'a, K, V, C, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    H: ByteHash,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.branch.val_mut()
    }
}

pub struct ValIter<'a, C, K, V, M, H>(
    LeafIter<'a, C, M, H>,
    PhantomData<(K, V)>,
)
where
    C: Compound<H>,
    H: ByteHash;

pub struct ValIterMut<'a, C, K, V, M, H>(
    LeafIterMut<'a, C, M, H>,
    PhantomData<(K, V)>,
)
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

pub trait KeyValIterable<K, V, H>
where
    Self: Compound<H>,
    Self::Leaf: KVPair<K, V>,
    H: ByteHash,
{
    fn values(&self) -> ValIter<Self, K, V, First, H>;
    fn values_mut(&mut self) -> ValIterMut<Self, K, V, First, H>;
    fn keys(&mut self) -> KeyIter<Self, K, V, First, H>;
}

impl<C, K, V, H> KeyValIterable<K, V, H> for C
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    H: ByteHash,
{
    fn values(&self) -> ValIter<Self, K, V, First, H> {
        ValIter(LeafIter::Initial(self, First), PhantomData)
    }

    fn values_mut(&mut self) -> ValIterMut<Self, K, V, First, H> {
        ValIterMut(LeafIterMut::Initial(self, First), PhantomData)
    }

    fn keys(&mut self) -> KeyIter<Self, K, V, First, H> {
        KeyIter(LeafIter::Initial(self, First), PhantomData)
    }
}

impl<'a, C, K, V, M, H> Iterator for ValIter<'a, C, K, V, M, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    M: 'a + Method,
    K: 'a,
    V: 'a,
    H: ByteHash,
{
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(KVPair::val)
    }
}

impl<'a, C, K, V, M, H> Iterator for ValIterMut<'a, C, K, V, M, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    M: 'a + Method,
    K: 'a,
    V: 'a,
    H: ByteHash,
{
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(KVPair::val_mut)
    }
}

impl<'a, C, K, V, M, H> Iterator for KeyIter<'a, C, K, V, M, H>
where
    C: Compound<H>,
    C::Leaf: KVPair<K, V>,
    M: 'a + Method,
    K: 'a,
    V: 'a,
    H: ByteHash,
{
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(KVPair::key)
    }
}
