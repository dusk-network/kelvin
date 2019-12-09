use std::{io, mem};

use crate::branch::{Branch, BranchMut};
use crate::compound::Compound;
use crate::search::{First, Method};
use crate::ByteHash;

/// An iterator over the leaves of a Compound type
pub enum LeafIter<'a, C, M, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Initial(&'a C, M),
    Branch(Branch<'a, C, H>, M),
    Exhausted,
}

impl<'a, C, M, H> Iterator for LeafIter<'a, C, M, H>
where
    C: Compound<H>,
    M: 'a + Method<C, H>,
    H: ByteHash,
{
    type Item = io::Result<&'a C::Leaf>;

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

pub enum LeafIterMut<'a, C, M, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Initial(&'a mut C, M),
    Branch(BranchMut<'a, C, H>, M),
    Exhausted,
}

impl<'a, C, M, H> Iterator for LeafIterMut<'a, C, M, H>
where
    C: Compound<H>,
    M: 'a + Method<C, H>,
    H: ByteHash,
{
    type Item = io::Result<&'a mut C::Leaf>;

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
pub trait LeafIterable<H>
where
    Self: Compound<H>,
    H: ByteHash,
{
    /// Returns an iterator over the leaves of the Compound
    fn iter(&self) -> LeafIter<Self, First, H>;
    /// Returns an iterator over the mutable leaves of the Compound
    fn iter_mut(&mut self) -> LeafIterMut<Self, First, H>;
}

impl<C, H> LeafIterable<H> for C
where
    C: Compound<H>,
    H: ByteHash,
{
    fn iter(&self) -> LeafIter<Self, First, H> {
        LeafIter::Initial(self, First)
    }

    fn iter_mut(&mut self) -> LeafIterMut<Self, First, H> {
        LeafIterMut::Initial(self, First)
    }
}
