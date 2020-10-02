// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use canonical::Store;

use crate::compound::Compound;
use crate::handle::HandleType;

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

/// Trait for searching through tree structured data
pub trait Method<C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Select among the handles of the node, indexed from `offset`
    fn select(&mut self, compound: &C, offset: usize) -> SearchResult;
}

#[derive(Clone)]
pub struct First;

impl<C, S> Method<C, S> for First
where
    S: Store,
    C: Compound<S>,
{
    fn select(&mut self, compound: &C, offset: usize) -> SearchResult
    where
        C: Compound<S>,
        S: Store,
    {
        for (i, h) in compound.children()[offset..].iter().enumerate() {
            match h.handle_type() {
                HandleType::Leaf => return SearchResult::Leaf(i),
                HandleType::Node => return SearchResult::Path(i),
                HandleType::None => (),
            }
        }
        SearchResult::None
    }
}
