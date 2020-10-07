// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use canonical::{Canon, Store};

use crate::annotations::Combine;
use crate::branch::{Branch, BranchMut};
use crate::handle::Handle;
use crate::search::Method;

/// A trait for tree-like structures containing leaves
pub trait Compound<S, const N: usize>: Canon<S> + Clone + Default
where
    S: Store,
{
    /// The leaf type of the compound structure
    type Leaf: Canon<S> + Clone;

    /// The node-annotation type
    type Annotation: Canon<S>
        + Combine<Self::Annotation>
        + for<'l> From<&'l Self::Leaf>;

    /// Returns handles to the children of the node
    fn children(&self) -> &[Handle<Self, S, N>];

    /// Returns mutable handles to the children of the node
    fn children_mut(&mut self) -> &mut [Handle<Self, S, N>];

    /// Returns the annotation of Compound, if not empty
    fn annotation(&self) -> Option<Self::Annotation> {
        Self::Annotation::combine(self.children())
    }

    /// Seach in the tree structure, with the provided method.
    /// Returns None if nothing was found, otherwise a branc pointing to the element found
    fn search<M: Method<Self, S, N>>(
        &self,
        m: &mut M,
    ) -> Result<Option<Branch<Self, S, N>>, S::Error> {
        Branch::new(self, m)
    }

    /// Seach in the tree structure, with the provided method.
    /// Returns None if nothing was found, otherwise a mutable branch pointing to
    /// the element found
    fn search_mut<M: Method<Self, S, N>>(
        &mut self,
        m: &mut M,
    ) -> Result<Option<BranchMut<Self, S, N>>, S::Error> {
        BranchMut::new(self, m)
    }

    /// Returns the hash of the type.
    /// This does not write anything to disk, the hashes are simply recursively
    /// computed and cached
    fn root_hash(&mut self) -> S::Ident {
        // let mut sink = Sink::new_dry();
        // self.persist(&mut sink).expect("Dry run");
        // sink.fin().expect("Dry run")
        unimplemented!()
    }
}
