use std::mem;

use cache::Cached;

use crate::branch::Branch;
use crate::search::{First, Method};
use crate::Handle;
use crate::{ByteHash, Content};

pub struct LeafIter<'a, C, M, H>
where
    C: Content<H>,
    H: ByteHash,
{
    method: M,
    state: LeafIterState<'a, C, H>,
}

enum LeafIterState<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    Initial(Cached<'a, C>),
    Branch(Branch<'a, C, H>),
    Exhausted,
}

impl<'a, C, M, H> Iterator for LeafIter<'a, C, M, H>
where
    C: Content<H>,
    M: 'a + Method,
    H: ByteHash,
{
    type Item = &'a C::Leaf;

    fn next(&mut self) -> Option<Self::Item> {
        let old_state = mem::replace(&mut self.state, LeafIterState::Exhausted);
        match old_state {
            LeafIterState::Initial(node) => {
                match Branch::new(node, &mut self.method) {
                    Some(branch) => {
                        self.state = LeafIterState::Branch(branch);
                    }
                    None => {
                        self.state = LeafIterState::Exhausted;
                    }
                }
            }
            LeafIterState::Branch(branch) => {
                if let Some(branch) = branch.search(&mut self.method) {
                    self.state = LeafIterState::Branch(branch)
                }
            }
            LeafIterState::Exhausted => return None,
        }
        let self_unsafe: &'a mut Self = unsafe { mem::transmute(self) };
        match self_unsafe.state {
            LeafIterState::Branch(ref branch) => {
                // TODO: motivate this use of unsafe
                Some(branch.leaf())
            }
            LeafIterState::Initial(_) => unreachable!(),
            LeafIterState::Exhausted => None,
        }
    }
}

pub trait LeafIterable<H>
where
    Self: Content<H>,
    H: ByteHash,
{
    fn iter(&self) -> LeafIter<Self, First, H>;
}

impl<C, H> LeafIterable<H> for C
where
    C: Content<H>,
    H: ByteHash,
{
    fn iter(&self) -> LeafIter<Self, First, H> {
        LeafIter {
            state: LeafIterState::Initial(Cached::Borrowed(self)),
            method: First,
        }
    }
}
