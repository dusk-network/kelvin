// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::mem;

use canonical::Store;

use crate::branch::{Branch, BranchMut};
use crate::compound::Compound;
use crate::search::{First, Method};

/// An iterator over the leaves of a Compound type
pub enum LeafIter<'a, C, M, S>
where
    C: Compound<S>,
    S: Store,
{
    Initial(&'a C, M),
    Branch(Branch<'a, C, S>, M),
    Exhausted,
}

impl<'a, C, M, S> Iterator for LeafIter<'a, C, M, S>
where
    C: Compound<S>,
    M: 'a + Method<C, S>,
    S: Store,
    C::Leaf: 'a,
{
    type Item = Result<&'a C::Leaf, S::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let old = mem::replace(self, LeafIter::Exhausted);
        match old {
            LeafIter::Initial(node, mut method) => {
                match Branch::new(node, &mut method) {
                    Ok(Some(branch)) => {
                        *self = LeafIter::Branch(branch, method);
                    }
                    Ok(None) => {
                        *self = LeafIter::Exhausted;
                    }
                    Err(e) => return Some(Err(e)),
                }
            }
            LeafIter::Branch(branch, mut method) => {
                match branch.search(&mut method) {
                    Ok(Some(branch)) => {
                        *self = LeafIter::Branch(branch, method)
                    }
                    Err(e) => return Some(Err(e)),
                    _ => (),
                }
            }
            LeafIter::Exhausted => return None,
        }

        let self_unsafe: &'a mut Self = unsafe { mem::transmute(self) };

        match self_unsafe {
            LeafIter::Branch(ref branch, _) => Some(Ok(&*branch)),
            LeafIter::Initial(_, _) => unreachable!(),
            LeafIter::Exhausted => None,
        }
    }
}

pub enum LeafIterMut<'a, C, M, S>
where
    C: Compound<S>,
    S: Store,
{
    Initial(&'a mut C, M),
    Branch(BranchMut<'a, C, S>, M),
    Exhausted,
}

impl<'a, C, M, S> Iterator for LeafIterMut<'a, C, M, S>
where
    C: Compound<S>,
    M: 'a + Method<C, S>,
    S: Store,
    C::Leaf: 'a,
{
    type Item = Result<&'a mut C::Leaf, S::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let old = mem::replace(self, LeafIterMut::Exhausted);
        match old {
            LeafIterMut::Initial(node, mut method) => {
                match BranchMut::new(node, &mut method) {
                    Ok(Some(branch)) => {
                        *self = LeafIterMut::Branch(branch, method);
                    }
                    Ok(None) => {
                        *self = LeafIterMut::Exhausted;
                    }
                    Err(e) => return Some(Err(e)),
                }
            }
            LeafIterMut::Branch(branch, mut method) => {
                match branch.search(&mut method) {
                    Ok(Some(branch)) => {
                        *self = LeafIterMut::Branch(branch, method)
                    }
                    Err(e) => return Some(Err(e)),
                    _ => (),
                }
            }
            LeafIterMut::Exhausted => return None,
        }

        let self_unsafe: &'a mut Self = unsafe { mem::transmute(self) };

        match self_unsafe {
            LeafIterMut::Branch(ref mut branch, _) => Some(Ok(&mut *branch)),
            LeafIterMut::Initial(_, _) => unreachable!(),
            LeafIterMut::Exhausted => None,
        }
    }
}

/// Trait for iterating over the leaves of a Compuond
pub trait LeafIterable<S>
where
    Self: Compound<S>,
    S: Store,
{
    /// Returns an iterator over the leaves of the Compound
    fn iter(&self) -> LeafIter<Self, First, S>;
    /// Returns an iterator over the mutable leaves of the Compound
    fn iter_mut(&mut self) -> LeafIterMut<Self, First, S>;
}

impl<C, S> LeafIterable<S> for C
where
    C: Compound<S>,
    S: Store,
{
    fn iter(&self) -> LeafIter<Self, First, S> {
        LeafIter::Initial(self, First)
    }

    fn iter_mut(&mut self) -> LeafIterMut<Self, First, S> {
        LeafIterMut::Initial(self, First)
    }
}
