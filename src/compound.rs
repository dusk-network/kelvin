use crate::content::Content;
use crate::handle::Handle;
use bytehash::ByteHash;

pub enum InsertResult<L> {
    Ok,
    Split(L),
}

pub trait Compound<H>: Content<H> + Default
where
    H: ByteHash,
{
    type Leaf: Content<H>;

    fn children_mut(&mut self) -> &mut [Handle<Self, H>];

    fn children(&self) -> &[Handle<Self, H>];
}
