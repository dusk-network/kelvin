use std::mem;

use crate::branch::{Branch, BranchMut};
use crate::compound::Compound;
use crate::search::{First, Method};
use crate::ByteHash;

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
    M: 'a + Method,
    H: ByteHash,
{
    type Item = &'a C::Leaf;

    fn next(&mut self) -> Option<Self::Item> {
        let old = mem::replace(self, LeafIter::Exhausted);
        match old {
            LeafIter::Initial(node, mut method) => {
                match Branch::new(node, &mut method) {
                    Some(branch) => {
                        *self = LeafIter::Branch(branch, method);
                    }
                    None => {
                        *self = LeafIter::Exhausted;
                    }
                }
            }
            LeafIter::Branch(branch, mut method) => {
                if let Some(branch) = branch.search(&mut method) {
                    *self = LeafIter::Branch(branch, method)
                }
            }
            LeafIter::Exhausted => return None,
        }

        let self_unsafe: &'a mut Self = unsafe { mem::transmute(self) };

        match self_unsafe {
            LeafIter::Branch(ref branch, _) => Some(&*branch),
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
    M: 'a + Method,
    H: ByteHash,
{
    type Item = &'a mut C::Leaf;

    fn next(&mut self) -> Option<Self::Item> {
        let old = mem::replace(self, LeafIterMut::Exhausted);
        match old {
            LeafIterMut::Initial(node, mut method) => {
                match BranchMut::new(node, &mut method) {
                    Some(branch) => {
                        *self = LeafIterMut::Branch(branch, method);
                    }
                    None => {
                        *self = LeafIterMut::Exhausted;
                    }
                }
            }
            LeafIterMut::Branch(branch, mut method) => {
                if let Some(branch) = branch.search(&mut method) {
                    *self = LeafIterMut::Branch(branch, method)
                }
            }
            LeafIterMut::Exhausted => return None,
        }

        let self_unsafe: &'a mut Self = unsafe { mem::transmute(self) };

        match self_unsafe {
            LeafIterMut::Branch(ref mut branch, _) => Some(&mut *branch),
            LeafIterMut::Initial(_, _) => unreachable!(),
            LeafIterMut::Exhausted => None,
        }
    }
}

pub trait LeafIterable<H>
where
    Self: Compound<H>,
    H: ByteHash,
{
    fn iter(&self) -> LeafIter<Self, First, H>;
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
