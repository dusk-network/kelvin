// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

//! Kelvin, a Merkle-tree tooklit and backend
#![warn(missing_docs)]
#![cfg_attr(feature = "no_std", no_std)]
#![feature(min_const_generics)]

/// Test helpers
pub mod tests;

/// A collection of tree annotations
pub mod annotations;

mod backend;
mod branch;
mod compound;
mod debug_draw;
mod handle;
mod iter;
mod map;
mod proof;
mod raw_branch;
mod search;

pub use crate::annotations::{
    Annotation, Associative, Combine, ErasedAnnotation, Void,
};
pub use crate::branch::{Branch, BranchMut};
pub use crate::compound::Compound;
// pub use crate::debug_draw::{DebugDraw, DrawState};
pub use crate::handle::{
    Handle, HandleMut, HandleMutLeaf, HandleMutNode, HandleMutNone, HandleRef,
    HandleType,
};
pub use crate::iter::LeafIterable;
pub use crate::map::{ValIterable, ValPath, ValPathMut, ValRef, ValRefMut, KV};
pub use crate::proof::Proof;
pub use crate::raw_branch::Level;
pub use crate::search::{Method, SearchResult};

// Re-export
pub use bytehash::{Blake2b, ByteHash, State as ByteHashState};
