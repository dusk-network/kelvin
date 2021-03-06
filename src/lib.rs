// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

//! Kelvin, a Merkle-tree tooklit and backend
#![warn(missing_docs)]

/// Test helpers
pub mod tests;

/// A collection of tree annotations
pub mod annotations;

mod backend;
mod branch;
mod compound;
mod content;
mod debug_draw;
mod erased;
mod handle;
mod iter;
mod map;
mod proof;
mod raw_branch;
mod root;
mod search;
mod sink;
mod source;
mod store;

pub use crate::annotations::{
    Annotation, Associative, Combine, ErasedAnnotation, Void,
};
pub use crate::backend::Backend;
pub use crate::branch::{Branch, BranchMut};
pub use crate::compound::Compound;
pub use crate::content::Content;
pub use crate::debug_draw::{DebugDraw, DrawState};
pub use crate::erased::{Erased, Query, Transaction};
pub use crate::handle::{
    Handle, HandleMut, HandleMutLeaf, HandleMutNode, HandleMutNone, HandleRef,
    HandleType,
};
pub use crate::iter::LeafIterable;
pub use crate::map::{ValIterable, ValPath, ValPathMut, ValRef, ValRefMut, KV};
pub use crate::proof::Proof;
pub use crate::raw_branch::Level;
pub use crate::root::Root;
pub use crate::search::{Method, SearchResult};
pub use crate::sink::Sink;
pub use crate::source::Source;
pub use crate::store::{Snapshot, Store};

// Re-export
pub use bytehash::{Blake2b, ByteHash, State as ByteHashState};

/// Persistant store using Blake2b
pub type DefaultStore = Store<Blake2b>;
