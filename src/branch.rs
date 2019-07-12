use std::marker::PhantomData;

use cache::Cached;
use smallvec::SmallVec;

use crate::content::Content;
use crate::handle::{Handle, HandleRef};
use crate::search::Method;
use crate::ByteHash;

// how deep the branch can be without allocating
const STACK_BRANCH_MAX_DEPTH: usize = 6;

enum Found {
    Leaf,
    Node,
    Nothing,
}

pub struct Branch<'a, C, H>(
    SmallVec<[Level<'a, C, H>; STACK_BRANCH_MAX_DEPTH]>,
);

struct Level<'a, C, H> {
    ofs: usize,
    node: Cached<'a, C>,
    _marker: PhantomData<H>,
}

impl<'a, C, H> Level<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    fn new(node: Cached<'a, C>) -> Self {
        Level {
            ofs: 0,
            node,
            _marker: PhantomData,
        }
    }

    fn leaf(&'a self) -> Option<&'a C::Leaf> {
        match (*self.node).children()[self.ofs].inner() {
            HandleRef::Leaf(l) => Some(l),
            _ => None,
        }
    }

    fn referencing(&'a self) -> HandleRef<'a, C, H> {
        self.node.children()[self.ofs].inner()
    }

    fn search<M: Method>(&mut self, method: &mut M) -> Found {
        match method.select(&self.node.children()[self.ofs..]) {
            Some(i) => {
                self.ofs += i;
                match self.referencing() {
                    HandleRef::Leaf(_) => Found::Leaf,
                    HandleRef::Node(_) => Found::Node,
                    _ => unreachable!(),
                }
            }
            None => Found::Nothing,
        }
    }
}

impl<'a, C, H> Into<Option<Branch<'a, C, H>>> for PartialBranch<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    fn into(self) -> Option<Branch<'a, C, H>> {
        if self.valid() {
            Some(Branch(self.0))
        } else {
            None
        }
    }
}

impl<'a, C, H> Into<PartialBranch<'a, C, H>> for Branch<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    fn into(self) -> PartialBranch<'a, C, H> {
        PartialBranch(self.0)
    }
}

impl<'a, C, H> PartialBranch<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    fn new(node: Cached<'a, C>) -> Self {
        let mut levels = SmallVec::new();
        levels.push(Level::new(node));
        PartialBranch(levels)
    }

    fn leaf(&'a self) -> Option<&'a C::Leaf> {
        self.0.last()?.leaf()
    }

    fn valid(&self) -> bool {
        match self.0.last() {
            Some(level) => level.leaf().is_some(),
            None => false,
        }
    }

    fn advance(&mut self) {
        self.0.last_mut().map(|level| level.ofs += 1);
    }

    pub fn search<M: Method>(&mut self, method: &mut M) {
        loop {
            match self.0.last_mut() {
                Some(level) => match level.search(method) {
                    Found::Leaf => {
                        return;
                    }
                    Found::Node => unimplemented!(),
                    Found::Nothing => {
                        self.0.pop();
                    }
                },
                None => {
                    return;
                }
            }
        }
    }
}

// an as-of-yet unfinished branch
struct PartialBranch<'a, C, H>(
    SmallVec<[Level<'a, C, H>; STACK_BRANCH_MAX_DEPTH]>,
)
where
    C: Content<H>,
    H: ByteHash;

impl<'a, C, H> Branch<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    pub fn new<M: Method>(node: Cached<'a, C>, method: &mut M) -> Option<Self> {
        let mut partial = PartialBranch::new(node);
        partial.search(method);
        partial.into()
    }

    pub fn search<M: Method>(self, method: &mut M) -> Option<Branch<'a, C, H>> {
        let mut partial: PartialBranch<'a, C, H> = self.into();
        partial.advance();
        partial.search(method);
        partial.into()
    }

    pub fn leaf(&'a self) -> &'a C::Leaf {
        self.0
            .last()
            .expect("Invalid branch")
            .leaf()
            .expect("Invalid branch")
    }
}
