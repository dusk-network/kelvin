// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::ops::{Deref, DerefMut};

use canonical::Store;

use crate::compound::Compound;
use crate::raw_branch::{Level, RawBranch};
use crate::search::Method;

/// A branch into a `Compound<S>`
/// The Branch is guaranteed to always point to a leaf
#[derive(Debug)]
pub struct Branch<'a, C, S>(RawBranch<'a, C, S>)
where
    C: Clone;

impl<'a, C, S> Branch<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Attempt to construct a branch with the given search method
    pub fn new<M: Method<C, S>>(
        node: &'a C,
        method: &mut M,
    ) -> Result<Option<Self>, S::Error>
    where
        C: Clone,
        M: Method<C, S>,
    {
        let mut inner = RawBranch::new_shared(node);
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
    pub fn search<M: Method<C, S>>(
        mut self,
        method: &mut M,
    ) -> Result<Option<Self>, S::Error> {
        self.0.advance();
        self.0.search(method)?;
        Ok(if self.0.leaf().is_some() {
            Some(self)
        } else {
            None
        })
    }

    /// Returns a reference to the levels of the branch
    pub fn levels(&self) -> &[Level<'a, C, S>] {
        self.0.levels()
    }
}

impl<'a, C, S> Deref for Branch<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid Branch")
    }
}

/// A mutable branch into a `Compound<S>`
/// The BranchMut is guaranteed to always point to a leaf
#[derive(Debug)]
pub struct BranchMut<'a, C, S>(RawBranch<'a, C, S>)
where
    C: Compound<S>,
    S: Store;

impl<'a, C, S> BranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    ///
    pub fn new<M>(
        node: &'a mut C,
        method: &mut M,
    ) -> Result<Option<Self>, S::Error>
    where
        M: Method<C, S>,
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
    pub fn search<M: Method<C, S>>(
        mut self,
        method: &mut M,
    ) -> Result<Option<Self>, S::Error> {
        self.0.advance();
        self.0.search(method)?;
        Ok(if self.0.leaf().is_some() {
            Some(self)
        } else {
            None
        })
    }

    /// Returns a reference to the levels of the branch
    pub fn levels(&self) -> &[Level<'a, C, S>] {
        self.0.levels()
    }

    /// Returns a reference to the levels of the branch
    pub fn levels_mut(&mut self) -> &mut [Level<'a, C, S>] {
        self.0.levels_mut()
    }
}

impl<'a, C, S> Drop for BranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn drop(&mut self) {
        self.0.relink()
    }
}

impl<'a, C, S> Deref for BranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid BranchMut")
    }
}

impl<'a, C, S> DerefMut for BranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
    C::Leaf: 'a,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.leaf_mut().expect("Invalid BranchMut")
    }
}
