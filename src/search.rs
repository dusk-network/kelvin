use crate::compound::Compound;
use crate::handle::{Handle, HandleType};
use crate::ByteHash;

/// Trait for searching through tree structured data
pub trait Method<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Select among the handles of the node
    fn select(&mut self, handles: &[Handle<C, H>]) -> Option<usize>;
}

#[derive(Clone)]
pub struct First;

impl<C, H> Method<C, H> for First
where
    H: ByteHash,
    C: Compound<H>,
{
    fn select(&mut self, handles: &[Handle<C, H>]) -> Option<usize>
    where
        C: Compound<H>,
        H: ByteHash,
    {
        for (i, h) in handles.iter().enumerate() {
            match h.handle_type() {
                HandleType::Leaf | HandleType::Node => return Some(i),
                HandleType::None => (),
            }
        }
        None
    }
}
