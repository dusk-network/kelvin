use std::io;
use std::ops::{Deref, DerefMut};

use crate::{ByteHash, Compound, Handle, HandleMut, HandleOwned};

pub struct UpdatingWrapper<'a, C, H>(&'a mut Handle<C, H>)
where
    H: ByteHash,
    C: Compound<H>;

impl<'a, C, H> Deref for UpdatingWrapper<'a, C, H>
where
    H: ByteHash,
    C: Compound<H>,
{
    type Target = Handle<C, H>;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, C, H> DerefMut for UpdatingWrapper<'a, C, H>
where
    H: ByteHash,
    C: Compound<H>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'a, C, H> UpdatingWrapper<'a, C, H>
where
    H: ByteHash,
    C: Compound<H>,
{
    /// Returns a HandleMut from the wrapped Handle
    pub fn inner_mut(&mut self) -> io::Result<HandleMut<C, H>> {
        self.0.inner_mut()
    }

    /// Replace the handle with an owned one, returning the leaf if any
    pub fn replace(&mut self, with: HandleOwned<C, H>) -> Option<C::Leaf> {
        self.0.replace(with)
    }
}

impl<'a, C, H> Drop for UpdatingWrapper<'a, C, H>
where
    H: ByteHash,
    C: Compound<H>,
{
    fn drop(&mut self) {
        self.0.update_annotation()
    }
}

/// Trait wrapping mutable access to handles
pub trait Children<C, H>: AsMut<[Handle<C, H>]>
where
    H: ByteHash,
    C: Compound<H>,
{
    /// Gets a mutable reference to a Handle
    fn slot_mut(&mut self, i: usize) -> UpdatingWrapper<C, H>;
}

impl<C, H, T> Children<C, H> for T
where
    H: ByteHash,
    C: Compound<H>,
    T: AsMut<[Handle<C, H>]>,
{
    fn slot_mut(&mut self, i: usize) -> UpdatingWrapper<C, H> {
        UpdatingWrapper(&mut self.as_mut()[i])
    }
}
