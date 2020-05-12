use std::io;
use std::ops::{Deref, DerefMut};

use bytehash::ByteHash;
use cache::Cached;

use crate::compound::Compound;
use crate::raw_branch::{Level, RawBranch};
use crate::search::Method;

/// A branch into a `Compound<H>`
/// The Branch is guaranteed to always point to a leaf
#[derive(Debug)]
pub struct Branch<'a, C, H>(RawBranch<'a, C, H>);

impl<'a, C, H> Branch<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Attempt to construct a branch with the given search method
    pub fn new<M: Method<C, H>>(
        node: &'a C,
        method: &mut M,
    ) -> io::Result<Option<Self>>
    where
        M: Method<C, H>,
    {
        let mut inner = RawBranch::new_cached(Cached::Borrowed(node));
        inner.search(method)?;
        Ok(if inner.leaf().is_some() {
            Some(Branch(inner))
        } else {
            None
        })
    }

    pub(crate) fn exact(&self) -> bool {
        self.0.exact()
    }

    /// Search for the next value in the branch, using `method`
    ///
    /// Takes self by value, and returns the updated branch or `None`
    pub fn search<M: Method<C, H>>(
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

    /// Returns a reference to the levels of the branch
    pub fn levels(&self) -> &[Level<'a, C, H>] {
        self.0.levels()
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

/// A mutable branch into a `Compound<H>`
/// The BranchMut is guaranteed to always point to a leaf
#[derive(Debug)]
pub struct BranchMut<'a, C, H>(RawBranch<'a, C, H>)
where
    C: Compound<H>,
    H: ByteHash;

impl<'a, C, H> BranchMut<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    ///
    pub fn new<M>(node: &'a mut C, method: &mut M) -> io::Result<Option<Self>>
    where
        M: Method<C, H>,
    {
        let mut inner = RawBranch::new_mutable(node);
        inner.search(method)?;
        Ok(if inner.leaf().is_some() {
            Some(BranchMut(inner))
        } else {
            None
        })
    }

    pub(crate) fn exact(&self) -> bool {
        self.0.exact()
    }

    /// Search for the next value in the branch, using `method`
    ///
    /// Takes self by value, and returns the updated branch or `None`
    pub fn search<M: Method<C, H>>(
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

    /// Returns a reference to the levels of the branch
    pub fn levels(&self) -> &[Level<'a, C, H>] {
        self.0.levels()
    }

    /// Returns a reference to the levels of the branch
    pub fn levels_mut(&mut self) -> &mut [Level<'a, C, H>] {
        self.0.levels_mut()
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
