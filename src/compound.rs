use bytehash::ByteHash;
use std::io;

use crate::annotations::Combine;
use crate::branch::Branch;
use crate::content::Content;
use crate::handle::Handle;
use crate::search::Method;
use crate::sink::Sink;

/// A trait for tree-like structures containing leaves
pub trait Compound<H>: Content<H> + Default
where
    H: ByteHash,
{
    /// The leaf type of the compound structure
    type Leaf: Content<H>;

    /// The node-annotation type
    type Annotation: Content<H>
        + Combine<Self::Annotation>
        + for<'l> From<&'l Self::Leaf>;

    /// Returns handles to the children of the node
    fn children(&self) -> &[Handle<Self, H>];

    /// Returns mutable handles to the children of the node
    fn children_mut(&mut self) -> &mut [Handle<Self, H>];

    /// Returns the annotation of Compound, if not empty
    fn annotation(&self) -> Option<Self::Annotation> {
        Self::Annotation::combine(self.children())
    }

    /// Returns the root hash of the tree,
    /// This does not write anything to disk, the hashes are simply recursively
    /// computed and cached
    fn root_hash(&mut self) -> H::Digest {
        let mut sink = Sink::new_dry();
        self.persist(&mut sink).expect("Dry run");
        sink.fin().expect("Dry run")
    }

    /// Seach in the tree structure, with the provided method.
    /// Returns None if nothing was found, otherwise a branc pointing to the element found
    fn search<M: Method<Self, H>>(
        &self,
        m: &mut M,
    ) -> io::Result<Option<Branch<Self, H>>> {
        Branch::new(self, m)
    }
}
