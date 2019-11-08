use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};

use bytehash::ByteHash;
use cache::Cached;
use smallvec::SmallVec;

use crate::compound::Compound;
use crate::handle::{Handle, HandleMut, HandleRef};
use crate::search::Method;

// how deep the branch can be without allocating
const STACK_BRANCH_MAX_DEPTH: usize = 6;

pub enum Found {
    Leaf,
    Node,
    Nothing,
}

enum NodeRef<'a, C, H> {
    Cached(Cached<'a, C>),
    Mutable(&'a mut C),
    Owned(Box<C>),
    #[allow(unused)]
    Placeholder(PhantomData<H>),
}

pub struct Level<'a, C, H> {
    ofs: usize,
    node: NodeRef<'a, C, H>,
}

pub(crate) struct UnsafeBranch<'a, C, H>(
    SmallVec<[Level<'a, C, H>; STACK_BRANCH_MAX_DEPTH]>,
);

impl<'a, C, H> NodeRef<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    pub fn new_cached(cached: Cached<'a, C>) -> Self {
        NodeRef::Cached(cached)
    }

    pub fn new_mutable(node: &'a mut C) -> Self {
        NodeRef::Mutable(node)
    }

    pub fn handle(&self, idx: usize) -> HandleRef<C, H> {
        match self {
            NodeRef::Cached(ref c) => {
                if let Some(handle) = c.children().get(idx) {
                    handle.inner()
                } else {
                    HandleRef::None
                }
            }
            NodeRef::Mutable(m) => {
                if let Some(handle) = m.children().get(idx) {
                    handle.inner()
                } else {
                    HandleRef::None
                }
            }
            NodeRef::Owned(o) => {
                if let Some(handle) = (**o).children().get(idx) {
                    handle.inner()
                } else {
                    HandleRef::None
                }
            }
            NodeRef::Placeholder(_) => unreachable!(),
        }
    }

    pub fn handle_mut(&mut self, idx: usize) -> HandleMut<C, H> {
        match self {
            NodeRef::Cached(c) => {
                *self = NodeRef::Owned(Box::new(c.clone()));
                self.handle_mut(idx)
            }
            NodeRef::Mutable(m) => {
                if let Some(handle) = m.children_mut().get_mut(idx) {
                    handle.inner_mut()
                } else {
                    HandleMut::None
                }
            }
            NodeRef::Owned(o) => {
                if let Some(handle) = (**o).children_mut().get_mut(idx) {
                    handle.inner_mut()
                } else {
                    HandleMut::None
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn inner_immutable(&self) -> InnerImmutable<C> {
        match self {
            NodeRef::Cached(c) => InnerImmutable::Cached(c),
            NodeRef::Mutable(m) => InnerImmutable::Borrowed(m),
            NodeRef::Owned(o) => InnerImmutable::Borrowed(o),
            NodeRef::Placeholder(_) => unreachable!("Placeholder"),
        }
    }
}

impl<C, H> AsMut<C> for NodeRef<'_, C, H>
where
    C: Clone,
{
    fn as_mut(&mut self) -> &mut C {
        loop {
            match self {
                NodeRef::Mutable(ref mut m) => return m,
                NodeRef::Owned(b) => return &mut *b,
                NodeRef::Cached(c) => {
                    *self = NodeRef::Owned(Box::new((**c).clone()));
                }
                NodeRef::Placeholder(_) => unreachable!("Placeholder"),
            }
        }
    }
}

enum InnerImmutable<'a, C> {
    Cached(&'a Cached<'a, C>),
    Borrowed(&'a C),
}

impl<'a, C> Deref for InnerImmutable<'a, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            InnerImmutable::Cached(ref c) => c,
            InnerImmutable::Borrowed(b) => b,
        }
    }
}

impl<'a, C, H> Deref for NodeRef<'a, C, H> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            NodeRef::Mutable(m) => m,
            NodeRef::Owned(o) => o,
            NodeRef::Cached(c) => c,
            NodeRef::Placeholder(_) => unreachable!(),
        }
    }
}

impl<'a, C, H> DerefMut for NodeRef<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            NodeRef::Mutable(m) => m,
            NodeRef::Owned(o) => o,
            NodeRef::Cached(_) => {
                if let NodeRef::Cached(c) =
                    mem::replace(self, NodeRef::Placeholder(PhantomData))
                {
                    *self = NodeRef::Owned(Box::new(c.clone()));
                } else {
                    unreachable!()
                }

                self.deref_mut()
            }
            NodeRef::Placeholder(_) => unreachable!(),
        }
    }
}

impl<'a, C, H> Level<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    pub fn new_cached(cached: Cached<'a, C>) -> Self {
        Level {
            ofs: 0,
            node: NodeRef::new_cached(cached),
        }
    }

    pub fn insert_child(&mut self, node: C) {
        match &mut self.node {
            NodeRef::Cached(c) => {
                self.node = NodeRef::Owned(Box::new((*c).clone()));
                self.insert_child(node)
            }
            NodeRef::Owned(o) => {
                (**o).children_mut()[self.ofs] = Handle::new_node(node)
            }
            NodeRef::Placeholder(_) => unreachable!(),
            NodeRef::Mutable(ref mut m) => {
                m.children_mut()[self.ofs] = Handle::new_node(node)
            }
        }
    }

    pub fn new_mutable(node: &'a mut C) -> Self {
        Level {
            ofs: 0,
            node: NodeRef::new_mutable(node),
        }
    }

    fn inner_immutable(&self) -> InnerImmutable<C> {
        self.node.inner_immutable()
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        self.node
            .children()
            .get(self.ofs)
            .and_then(|handle| handle.leaf())
    }

    pub fn leaf_mut(&'a mut self) -> Option<&'a mut C::Leaf> {
        self.node
            .children_mut()
            .get_mut(self.ofs)
            .and_then(|handle| handle.leaf_mut())
    }

    pub fn referencing(&self) -> HandleRef<C, H> {
        self.node.handle(self.ofs)
    }

    pub fn referencing_mut(&mut self) -> HandleMut<C, H> {
        self.node.handle_mut(self.ofs)
    }

    fn search<M: Method>(&mut self, method: &mut M) -> Found {
        let node = self.inner_immutable();
        let children = node.children();
        if self.ofs > children.len() - 1 {
            return Found::Nothing;
        } else {
            match method.select(&children[self.ofs..]) {
                Some(i) => {
                    self.ofs += i;
                    match self.referencing() {
                        HandleRef::Leaf(_) => Found::Leaf,
                        HandleRef::Node(_) => Found::Node,
                        HandleRef::None => Found::Nothing,
                    }
                }
                None => Found::Nothing,
            }
        }
    }

    pub fn ofs(&self) -> usize {
        self.ofs
    }
}

impl<C, H> AsMut<C> for Level<'_, C, H>
where
    C: Clone,
{
    fn as_mut(&mut self) -> &mut C {
        self.node.as_mut()
    }
}

impl<'a, C, H> UnsafeBranch<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    // Only used as temporary replacement
    pub(crate) fn empty() -> Self {
        UnsafeBranch(SmallVec::new())
    }

    pub fn new_cached(node: Cached<'a, C>) -> Self {
        let mut vec = SmallVec::new();
        vec.push(Level::new_cached(node));
        UnsafeBranch(vec)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn new_mutable(node: &'a mut C) -> Self {
        let mut vec = SmallVec::new();
        vec.push(Level::new_mutable(node));
        UnsafeBranch(vec)
    }

    pub fn search<M: Method>(&mut self, method: &mut M) {
        loop {
            if let Some(last) = self.0.last_mut() {
                let mut push = None;
                match last.search(method) {
                    Found::Leaf => {
                        break;
                    }
                    Found::Node => match last.referencing() {
                        HandleRef::Node(cached) => {
                            let level: Level<'a, _, _> = unsafe {
                                mem::transmute(Level::new_cached(cached))
                            };
                            push = Some(level);
                        }
                        _ => unreachable!(),
                    },
                    Found::Nothing => {
                        if self.0.len() > 1 {
                            self.pop_level();
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                if let Some(level) = push.take() {
                    self.0.push(level);
                }
            } else {
                break;
            }
        }
    }

    pub fn advance(&mut self) {
        self.0.last_mut().map(|level| level.ofs += 1);
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        if let Some(last) = self.0.last() {
            last.leaf()
        } else {
            None
        }
    }

    pub fn last_node_mut(&mut self) -> Option<&mut C> {
        self.0.last_mut().map(|last| &mut *last.node)
    }

    pub(crate) fn leaf_mut(&mut self) -> Option<&'a mut C::Leaf> {
        unsafe {
            let unsafe_self: &'a mut Self = mem::transmute(self);
            if let Some(last) = unsafe_self.0.last_mut() {
                last.leaf_mut()
            } else {
                None
            }
        }
    }

    fn pop_level(&mut self) -> bool {
        if let Some(popped) = self.0.pop() {
            if self.0.len() > 0 {
                match popped.node {
                    NodeRef::Owned(o) => {
                        let last = self.0.last_mut().expect("length < 1");
                        last.insert_child(*o);
                    }
                    _ => (),
                }
            }
            true
        } else {
            false
        }
    }

    /// Makes sure all owned nodes in the branch are inserted into the tree.
    pub(crate) fn relink(&mut self) {
        while self.pop_level() {}
    }
}
