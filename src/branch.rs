// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use canonical::{Repr, Store};

use crate::compound::Compound;
use crate::raw_branch::{Level, RawBranch};
use crate::search::Method;

/// A branch into a `Compound<S>`
/// The Branch is guaranteed to always point to a leaf
#[derive(Debug)]
pub struct Branch<C, S>(RawBranch<C, S>)
where
    C: Clone,
    S: Store;

impl<C, S> Branch<C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Attempt to construct a branch with the given search method
    pub fn new<M: Method<C, S>>(
        node: C,
        method: &mut M,
    ) -> Result<Option<Self>, S::Error>
    where
        C: Clone,
        M: Method<C, S>,
    {
        let mut inner = RawBranch::new(node)?;
        inner.search(method)?;

        Ok(if inner.leaf()?.is_some() {
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
        Ok(if self.0.leaf()?.is_some() {
            Some(self)
        } else {
            None
        })
    }

    /// Returns a reference to the levels of the branch
    pub fn levels(&self) -> &[Level<C, S>] {
        self.0.levels()
    }
}

// impl<C, S> Deref for Branch<C, S>
// where
//     C: Compound<S>,
//     S: Store,
// {
//     type Target = C::Leaf;

//     fn deref(&self) -> &Self::Target {
//         self.0
//             .leaf()
//             .expect("Invalid Branch")
//             .expect("Invalid brach")
//     }
// }

/// A mutable branch into a `Compound<S>`
/// The BranchMut is guaranteed to always point to a leaf
#[derive(Debug)]
pub struct BranchMut<C, S>(RawBranch<C, S>)
where
    C: Compound<S>,
    S: Store;

impl<C, S> BranchMut<C, S>
where
    C: Compound<S>,
    S: Store,
{
    ///
    pub fn new<M>(
        node: &mut C,
        method: &mut M,
    ) -> Result<Option<Self>, S::Error>
    where
        M: Method<C, S>,
    {
        let mut inner = RawBranch::new(node.clone())?;
        inner.search(method)?;
        Ok(if inner.leaf()?.is_some() {
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
        Ok(if self.0.leaf()?.is_some() {
            Some(self)
        } else {
            None
        })
    }

    /// Returns a reference to the levels of the branch
    pub fn levels(&self) -> &[Level<C, S>] {
        self.0.levels()
    }

    /// Returns a reference to the levels of the branch
    pub fn levels_mut(&mut self) -> &mut [Level<C, S>] {
        self.0.levels_mut()
    }

    pub fn commit(self) -> Result<(), S::Error> {
        self.0.relink()
    }
}
