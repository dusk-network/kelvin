use std::marker::PhantomData;

use crate::branch::BranchMut;
use crate::compound::Compound;
use crate::content::Content;
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

pub struct Proof<C: Compound<H>, H: ByteHash>(Vec<ProofLevel<C, H>>);

impl<C, H> Proof<C, H>
where
    C: Compound<H>,
    H: ByteHash,
    H::Digest: std::fmt::Debug,
{
    pub fn new(from: &mut BranchMut<C, H>) -> Self {
        let mut branch = vec![];

        for level in from.levels_mut() {
            branch.push(ProofLevel::from(level))
        }
        Proof(branch)
    }

    pub fn valid(&mut self, against: &mut C) -> bool {
        // verify that all the hashes are correct bottom up

        let mut previous = None;

        for level in self.0.iter_mut().rev() {
            if let Some(prev) = previous {
                let ofs = level.ofs;
                if level.children_mut()[ofs].root_hash() != prev {
                    assert_eq!(level.children_mut()[ofs].root_hash(), prev);
                    return false;
                }
            }
            previous = Some(level.root_hash());
        }

        previous.expect("zero length proof invalid") == against.root_hash()
    }
}
