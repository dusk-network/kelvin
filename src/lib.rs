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

pub use crate::backend::{Backend, DiskBackend, MemBackend};
pub use crate::branch::{Branch, BranchMut};
pub use crate::compound::{Compound, InsertResult};
pub use crate::content::Content;
pub use crate::handle::{
    Handle, HandleMut, HandleOwned, HandleRef, HandleType,
};
pub use crate::iter::LeafIterable;
pub use crate::map::{KeyValIterable, ValRef, ValRefMut};
pub use crate::search::Method;
pub use crate::source::Source;
pub use crate::store::{Shared, Store};

// test infra
#[cfg(test)]
pub use tests::quickcheck_map;

// structures
pub use structures::HAMT;

// Re-export
pub use bytehash::{Blake2b, ByteHash};
