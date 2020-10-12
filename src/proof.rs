// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::marker::PhantomData;

use canonical::Store;

use crate::branch::BranchMut;
use crate::compound::Compound;
use crate::handle::Handle;
use crate::raw_branch::Level;

struct ProofLevel<C: Compound<S>, S: Store> {
    ofs: usize,
    node: C,
    _marker: PhantomData<S>,
}

impl<C, S> ProofLevel<C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn root_hash(&mut self) -> S::Ident {
        S::ident(&self.node)
    }

    fn children(&self) -> &[Handle<C, S>] {
        self.node.children()
    }

    fn children_mut(&mut self) -> &mut [Handle<C, S>] {
        self.node.children_mut()
    }
}

impl<C, S> From<&mut Level<'_, C, S>> for ProofLevel<C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn from(level: &mut Level<C, S>) -> Self {
        // Make sure we compute and cache the hashes along the path
        // let _ = S::ident(&*level);
        ProofLevel {
            ofs: level.offset(),
            node: (*level).clone(),
            _marker: PhantomData,
        }
    }
}

/// A merkle proof that a certain leaf exists in a compound collection
pub struct Proof<C: Compound<S>, S: Store>(Vec<ProofLevel<C, S>>);

impl<C, S> Proof<C, S>
where
    C: Compound<S>,
    S: Store,
    S::Ident: std::fmt::Debug,
{
    /// Creates a new proof from a branch
    pub fn new(from: &mut BranchMut<C, S>) -> Self {
        let mut branch = vec![];

        for level in from.levels_mut() {
            branch.push(ProofLevel::from(level))
        }
        Proof(branch)
    }

    fn get_leaf(&self) -> Option<&C::Leaf> {
        if let Some(level) = self.0.last() {
            level.children()[level.ofs].leaf()
        } else {
            None
        }
    }

    /// Proves the inclusion of the element and returns a reference to it
    /// or None if the proof is invalid.
    pub fn prove_member(
        &mut self,
        against: &mut C,
    ) -> Result<Option<&C::Leaf>, S::Error> {
        // verify that all the hashes are correct bottom up

        let mut previous = None;

        for level in self.0.iter_mut().rev() {
            if let Some(prev) = previous {
                let ofs = level.ofs;
                if let Some(node_hash) =
                    level.children_mut()[ofs].node_hash()?
                {
                    if node_hash != prev {
                        return Ok(None);
                    }
                } else {
                    return Ok(None);
                }
            }
            previous = Some(level.root_hash());
        }
        if let Some(root) = previous {
            // Verify against the structure we want to prove with
            if root == S::ident(against) {
                Ok(self.get_leaf())
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
