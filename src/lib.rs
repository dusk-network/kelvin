//! Kelvin, a Merkle-tree tooklit and backend
#![deny(missing_docs)]

mod annotations;
mod backend;
mod branch;
mod compound;
mod content;
mod handle;
mod iter;
mod map;
mod search;
mod sink;
mod source;
mod store;
mod structures;
mod tests;
mod unsafe_branch;

pub use crate::annotations::{Associative, Combine, VoidAnnotation};
pub use crate::backend::Backend;
pub use crate::branch::{Branch, BranchMut};
pub use crate::compound::Compound;
pub use crate::content::Content;
pub use crate::handle::{
    Handle, HandleMut, HandleOwned, HandleRef, HandleType,
};
pub use crate::iter::LeafIterable;
pub use crate::map::{KeyValIterable, ValPath, ValPathMut, ValRef, ValRefMut};
pub use crate::search::Method;
pub use crate::sink::Sink;
pub use crate::source::Source;
pub use crate::store::{Shared, Store};

// Re-export
pub use bytehash::{Blake2b, ByteHash};

// structures
pub use structures::HAMT;

// default types

/// Map using HAMT and Blake2b
pub type Map<K, V> = HAMT<(K, V), Blake2b>;
/// Persistant store using Blake2b
pub type DefaultStore = Store<Blake2b>;

// test infra
#[cfg(test)]
pub use tests::quickcheck_map;
