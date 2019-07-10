use crate::Handle;
use crate::{ByteHash, Content};

pub enum LeafIter<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    Initial(&'a [Handle<C, H>]),
}

impl<'a, C, H> Iterator for LeafIter<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    type Item = &'a C::Leaf;

    fn next(&mut self) -> Option<Self::Item> {
        unimplemented!()
    }
}

pub trait LeafIterable<H>
where
    Self: Content<H>,
    H: ByteHash,
{
    fn iter(&self) -> LeafIter<Self, H>;
}

impl<C, H> LeafIterable<H> for C
where
    C: Content<H>,
    H: ByteHash,
{
    fn iter(&self) -> LeafIter<Self, H> {
        LeafIter::Initial(self.children())
    }
}
