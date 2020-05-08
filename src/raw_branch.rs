use std::io;
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};

use bytehash::ByteHash;
use cache::Cached;

use crate::compound::Compound;
use crate::handle::{Handle, HandleRef};
use crate::search::{Method, SearchResult};

pub enum Found {
    Leaf,
    Path,
    None,
}

enum NodeRef<'a, C, H> {
    Cached(Cached<'a, C>),
    Mutable(&'a mut C),
    Owned(Box<C>),
    Placeholder(PhantomData<H>),
}

/// Represents a level in a branch
pub struct Level<'a, C, H> {
    ofs: usize,
    node: NodeRef<'a, C, H>,
}

impl<'a, C, H> Deref for Level<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

pub(crate) struct RawBranch<'a, C, H> {
    levels: Vec<Level<'a, C, H>>,
    exact: bool,
}

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

    pub fn handle(&self, idx: usize) -> io::Result<HandleRef<C, H>> {
        Ok(match self {
            NodeRef::Cached(ref c) => {
                if let Some(handle) = c.children().get(idx) {
                    handle.inner()?
                } else {
                    HandleRef::None
                }
            }
            NodeRef::Mutable(m) => {
                if let Some(handle) = m.children().get(idx) {
                    handle.inner()?
                } else {
                    HandleRef::None
                }
            }
            NodeRef::Owned(o) => {
                if let Some(handle) = (**o).children().get(idx) {
                    handle.inner()?
                } else {
                    HandleRef::None
                }
            }
            NodeRef::Placeholder(_) => unreachable!(),
        })
    }

    pub fn inner(&self) -> InnerImmutable<C> {
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

/// A reference to a cached or borrowed node
pub enum InnerImmutable<'a, C> {
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
    /// Returs a reference to the handle pointing to the node below in the branch
    pub fn referencing(&self) -> io::Result<HandleRef<C, H>> {
        self.node.handle(self.ofs)
    }

    /// Returns the offset of the reference node in the level of the branch
    /// i.e the node that references the level below.
    pub fn offset(&self) -> usize {
        self.ofs
    }

    fn new_cached(cached: Cached<'a, C>) -> Self {
        Level {
            ofs: 0,
            node: NodeRef::new_cached(cached),
        }
    }

    fn insert_child(&mut self, node: C) {
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

    fn new_mutable(node: &'a mut C) -> Self {
        Level {
            ofs: 0,
            node: NodeRef::new_mutable(node),
        }
    }

    fn inner(&self) -> InnerImmutable<C> {
        self.node.inner()
    }

    fn leaf(&self) -> Option<&C::Leaf> {
        self.node
            .children()
            .get(self.ofs)
            .and_then(|handle| handle.leaf())
    }

    fn leaf_mut(&'a mut self) -> Option<&'a mut C::Leaf> {
        self.node
            .children_mut()
            .get_mut(self.ofs)
            .and_then(|handle| handle.leaf_mut())
    }

    fn search<M: Method<C, H>>(&mut self, method: &mut M) -> io::Result<Found> {
        let node = self.inner();
        let children_len = node.children().len();
        if self.ofs + 1 > children_len {
            Ok(Found::None)
        } else {
            Ok(match method.select(&*node, self.ofs) {
                SearchResult::Leaf(i) => {
                    self.ofs += i;
                    Found::Leaf
                }
                SearchResult::Path(i) => {
                    self.ofs += i;
                    Found::Path
                }
                SearchResult::None => Found::None,
            })
        }
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

impl<'a, C, H> RawBranch<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    pub fn new_cached(node: Cached<'a, C>) -> Self {
        let mut vec = Vec::new();
        vec.push(Level::new_cached(node));
        RawBranch {
            levels: vec,
            exact: false,
        }
    }

    pub fn new_mutable(node: &'a mut C) -> Self {
        let mut vec = Vec::new();
        vec.push(Level::new_mutable(node));
        RawBranch {
            levels: vec,
            exact: false,
        }
    }

    pub(crate) fn exact(&self) -> bool {
        self.exact
    }

    pub fn search<M: Method<C, H>>(
        &mut self,
        method: &mut M,
    ) -> io::Result<()> {
        self.exact = false;
        while let Some(last) = self.levels.last_mut() {
            let mut push = None;
            match last.search(method)? {
                Found::Leaf => {
                    self.exact = true;
                    break;
                }
                Found::Path => match last.referencing()? {
                    HandleRef::Node(cached) => {
                        let level: Level<'a, _, _> = unsafe {
                            mem::transmute(Level::new_cached(cached))
                        };
                        push = Some(level);
                    }
                    _ => break,
                },
                Found::None => {
                    if self.levels.len() > 1 {
                        self.pop_level();
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            if let Some(level) = push.take() {
                self.levels.push(level);
            }
        }
        Ok(())
    }

    pub fn advance(&mut self) {
        if let Some(level) = self.levels.last_mut() {
            level.ofs += 1;
        }
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        if let Some(last) = self.levels.last() {
            last.leaf()
        } else {
            None
        }
    }

    pub(crate) fn leaf_mut(&mut self) -> Option<&'a mut C::Leaf> {
        unsafe {
            let unsafe_self: &'a mut Self = mem::transmute(self);
            if let Some(last) = unsafe_self.levels.last_mut() {
                last.leaf_mut()
            } else {
                None
            }
        }
    }

    pub fn levels(&self) -> &[Level<'a, C, H>] {
        &self.levels
    }

    fn pop_level(&mut self) -> bool {
        if let Some(popped) = self.levels.pop() {
            if !self.levels.is_empty() {
                if let NodeRef::Owned(o) = popped.node {
                    let last = self.levels.last_mut().expect("length < 1");
                    last.insert_child(*o);
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
