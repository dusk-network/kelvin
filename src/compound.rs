use bytehash::ByteHash;

use crate::annotations::Combine;
use crate::content::Content;
use crate::handle::Handle;

/// A trait for tree-like structures containing leaves
pub trait Compound<H>: Content<H> + Default
where
    H: ByteHash,
{
    /// The leaf type of the compound structure
    type Leaf: Content<H>;
    /// The node-annotation type
    type Annotation: Content<H> + Combine + for<'l> From<&'l Self::Leaf>;

    /// Returns handles to the children of the node
    fn children(&self) -> &[Handle<Self, H>];

    /// Returns mutable handles to the children of the node
    fn children_mut(&mut self) -> &mut [Handle<Self, H>];

    /// Calculates the annotation for the node
    fn annotation(&self) -> Option<Self::Annotation> {
        Self::Annotation::combine(
            self.children().iter().filter_map(Handle::annotation),
        )
    }
}
