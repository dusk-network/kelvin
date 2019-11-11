use std::io;
use std::ops::{Deref, DerefMut};

use bytehash::ByteHash;
use cache::Cached;

use crate::compound::Compound;
use crate::search::Method;
use crate::unsafe_branch::UnsafeBranch;

/// The Branch wrapper is guaranteed to always point to a leaf
pub struct Branch<'a, C, H>(UnsafeBranch<'a, C, H>);

impl<'a, C, H> Branch<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    pub fn new<M: Method>(
        node: &'a C,
        method: &mut M,
    ) -> io::Result<Option<Self>>
    where
        M: Method,
    {
        let mut inner = UnsafeBranch::new_cached(Cached::Borrowed(node));
        inner.search(method)?;
        Ok(if inner.leaf().is_some() {
            Some(Branch(inner))
        } else {
            None
        })
    }

    pub fn search<M: Method>(
        mut self,
        method: &mut M,
    ) -> io::Result<Option<Self>> {
        self.0.advance();
        self.0.search(method)?;
        Ok(if self.0.leaf().is_some() {
            Some(self)
        } else {
            None
        })
    }
}

impl<'a, C, H> Deref for Branch<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid Branch")
    }
}

/// The BranchMut wrapper is guaranteed to always point to a leaf
pub struct BranchMut<'a, C, H>(UnsafeBranch<'a, C, H>)
where
    C: Compound<H>,
    H: ByteHash;

impl<'a, C, H> BranchMut<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    pub fn new<M>(node: &'a mut C, method: &mut M) -> io::Result<Option<Self>>
    where
        M: Method,
    {
        let mut inner = UnsafeBranch::new_mutable(node);
        inner.search(method)?;
        Ok(if inner.leaf().is_some() {
            Some(BranchMut(inner))
        } else {
            None
        })
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn search<M: Method>(
        mut self,
        method: &mut M,
    ) -> io::Result<Option<Self>> {
        self.0.advance();
        self.0.search(method)?;
        Ok(if self.0.leaf().is_some() {
            Some(self)
        } else {
            None
        })
    }

    pub fn last_node_mut(&mut self) -> &mut C {
        self.0.last_node_mut().expect("Invalid BranchMut")
    }
}

impl<'a, C, H> Drop for BranchMut<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn drop(&mut self) {
        self.0.relink()
    }
}

impl<'a, C, H> Deref for BranchMut<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid BranchMut")
    }
}

impl<'a, C, H> DerefMut for BranchMut<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.leaf_mut().expect("Invalid BranchMut")
    }
}
