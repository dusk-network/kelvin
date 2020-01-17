use crate::compound::Compound;
use crate::handle::{Handle, HandleType};
use crate::ByteHash;

use std::ops::Deref;

#[derive(Debug)]
/// Result of searching through a node
pub enum SearchResult {
    /// Found exact match
    Leaf(usize),
    /// Found a Node/Leaf or None in path
    Path(usize),
    /// Early Abort
    None,
}

/// The type passed to the `select` method on `Method`
/// contains slices of handles and metadata
pub struct SearchIn<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    handles: &'a [Handle<C, H>],
    meta: &'a [C::Meta],
}

impl<'a, C, H> SearchIn<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    pub(crate) fn new_with_offset(
        offset: usize,
        handles: &'a [Handle<C, H>],
        mut meta: &'a [C::Meta],
    ) -> Self {
        // we might have default empty metadata
        if meta.len() != 0 {
            meta = &meta[offset..]
        }
        SearchIn {
            handles: &handles[offset..],
            meta,
        }
    }

    /// Returns the metadata array for the search
    pub fn meta(&self) -> &[C::Meta] {
        self.meta
    }
}

impl<'a, C, H> Deref for SearchIn<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    type Target = [Handle<C, H>];

    fn deref(&self) -> &Self::Target {
        &self.handles
    }
}

/// Trait for searching through tree structured data
pub trait Method<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Select among the handles of the node
    fn select(&mut self, handles: SearchIn<C, H>) -> SearchResult;
}

#[derive(Clone)]
pub struct First;

impl<C, H> Method<C, H> for First
where
    H: ByteHash,
    C: Compound<H>,
{
    fn select(&mut self, handles: SearchIn<C, H>) -> SearchResult
    where
        C: Compound<H>,
        H: ByteHash,
    {
        for (i, h) in handles.iter().enumerate() {
            match h.handle_type() {
                HandleType::Leaf => return SearchResult::Leaf(i),
                HandleType::Node => return SearchResult::Path(i),
                HandleType::None => (),
            }
        }
        SearchResult::None
    }
}
