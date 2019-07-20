use std::ops::DerefMut;

use bytehash::ByteHash;

use crate::branch::{Branch, BranchMut};
use crate::compound::Compound;

pub trait Set<T, H>: Compound<H>
where
    H: ByteHash,
{
    fn insert(&mut self, t: T) -> Option<T>;
    fn get(&self, t: &T) -> Option<Branch<Self, H>>;
    fn get_mut(&mut self, t: &T) -> Option<BranchMut<Self, H>>;
}
