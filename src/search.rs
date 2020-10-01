// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::compound::Compound;
use crate::handle::HandleType;
use crate::ByteHash;

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
pub trait Method<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Select among the handles of the node, indexed from `offset`
    fn select(&mut self, compound: &C, offset: usize) -> SearchResult;
}

#[derive(Clone)]
pub struct First;

impl<C, H> Method<C, H> for First
where
    H: ByteHash,
    C: Compound<H>,
{
    fn select(&mut self, compound: &C, offset: usize) -> SearchResult
    where
        C: Compound<H>,
        H: ByteHash,
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
