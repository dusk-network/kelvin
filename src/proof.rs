// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::marker::PhantomData;

use crate::branch::BranchMut;
use crate::compound::Compound;
use crate::handle::Handle;
use crate::raw_branch::Level;
use bytehash::ByteHash;

struct ProofLevel<C: Compound<H>, H: ByteHash> {
    ofs: usize,
    node: C,
    _marker: PhantomData<H>,
}

impl<C, H> ProofLevel<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn root_hash(&mut self) -> H::Digest {
        self.node.root_hash()
    }

    fn children(&self) -> &[Handle<C, H>] {
        self.node.children()
    }

    fn children_mut(&mut self) -> &mut [Handle<C, H>] {
        self.node.children_mut()
    }
}

impl<C, H> From<&mut Level<'_, C, H>> for ProofLevel<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn from(level: &mut Level<C, H>) -> Self {
        // Make sure we compute and cache the hashes along the path
        let _ = level.root_hash();
        ProofLevel {
            ofs: level.offset(),
            node: (*level).clone(),
            _marker: PhantomData,
        }
    }
}

/// A merkle proof that a certain leaf exists in a compound collection
pub struct Proof<C: Compound<H>, H: ByteHash>(Vec<ProofLevel<C, H>>);

impl<C, H> Proof<C, H>
where
    C: Compound<H>,
    H: ByteHash,
    H::Digest: std::fmt::Debug,
{
    /// Creates a new proof from a branch
    pub fn new(from: &mut BranchMut<C, H>) -> Self {
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
    pub fn prove_member(&mut self, against: &mut C) -> Option<&C::Leaf> {
        // verify that all the hashes are correct bottom up

        let mut previous = None;

        for level in self.0.iter_mut().rev() {
            if let Some(prev) = previous {
                let ofs = level.ofs;
                if let Some(node_hash) = level.children_mut()[ofs].node_hash() {
                    if node_hash != prev {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            previous = Some(level.root_hash());
        }
        if let Some(root) = previous {
            // Verify against the structure we want to prove with
            if root == against.root_hash() {
                self.get_leaf()
            } else {
                None
            }
        } else {
            None
        }
    }
}
