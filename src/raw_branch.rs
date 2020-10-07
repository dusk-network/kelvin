// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};

use canonical::Store;

use crate::compound::Compound;
use crate::handle::{Handle, HandleRef};
use crate::search::{Method, SearchResult};

use const_arrayvec::ArrayVec;

pub enum Found {
    Leaf,
    Path,
    None,
}

#[derive(Debug)]
enum NodeRef<'a, C, S, const N: usize>
where
    C: Clone,
{
    Shared(&'a C),
    Mutable(&'a mut C),
    Owned(C),
    Placeholder(PhantomData<S>),
}

/// Represents a level in a branch
#[derive(Debug)]
pub struct Level<'a, C, S, const N: usize>
where
    C: Clone,
{
    ofs: usize,
    node: NodeRef<'a, C, S, N>,
}

impl<'a, C, S, const N: usize> Deref for Level<'a, C, S, N>
where
    C: Compound<S, N>,
    S: Store,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

impl<'a, C, S, const N: usize> DerefMut for Level<'a, C, S, N>
where
    C: Compound<S, N>,
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node
    }
}

#[derive(Debug)]
pub(crate) struct RawBranch<'a, C, S, const N: usize>
where
    C: Clone,
{
    levels: ArrayVec<Level<'a, C, S, N>, N>,
    exact: bool,
}

impl<'a, C, S, const N: usize> NodeRef<'a, C, S, N>
where
    C: Compound<S, N>,
    S: Store,
{
    pub fn new_owned(owned: C) -> Self {
        NodeRef::Owned(owned)
    }

    pub fn new_shared(shared: &'a C) -> Self {
        NodeRef::Shared(shared)
    }

    pub fn new_mutable(node: &'a mut C) -> Self {
        NodeRef::Mutable(node)
    }

    pub fn handle(&self, idx: usize) -> Result<HandleRef<C, S, N>, S::Error> {
        Ok(match self {
            NodeRef::Shared(ref c) => {
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
                if let Some(handle) = (o).children().get(idx) {
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
            NodeRef::Shared(c) => InnerImmutable::Shared(c),
            NodeRef::Mutable(m) => InnerImmutable::Borrowed(m),
            NodeRef::Owned(o) => InnerImmutable::Borrowed(o),
            NodeRef::Placeholder(_) => unreachable!("Placeholder"),
        }
    }
}

impl<C, S, const N: usize> AsMut<C> for NodeRef<'_, C, S, N>
where
    C: Clone,
{
    fn as_mut(&mut self) -> &mut C {
        loop {
            match self {
                NodeRef::Mutable(ref mut m) => return m,
                NodeRef::Owned(b) => return &mut *b,
                NodeRef::Shared(c) => {
                    *self = NodeRef::Owned(c.clone());
                }
                NodeRef::Placeholder(_) => unreachable!("Placeholder"),
            }
        }
    }
}

/// A reference to a shared or borrowed node
pub enum InnerImmutable<'a, C>
where
    C: Clone,
{
    Shared(&'a C),
    Borrowed(&'a C),
}

impl<'a, C> Deref for InnerImmutable<'a, C>
where
    C: Clone,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            InnerImmutable::Shared(ref c) => c,
            InnerImmutable::Borrowed(b) => b,
        }
    }
}

impl<'a, C, S, const N: usize> Deref for NodeRef<'a, C, S, N>
where
    C: Clone,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            NodeRef::Mutable(m) => m,
            NodeRef::Owned(o) => o,
            NodeRef::Shared(c) => c,
            NodeRef::Placeholder(_) => unreachable!(),
        }
    }
}

impl<'a, C, S, const N: usize> DerefMut for NodeRef<'a, C, S, N>
where
    C: Compound<S, N>,
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            NodeRef::Mutable(m) => m,
            NodeRef::Owned(o) => o,
            NodeRef::Shared(_) => {
                if let NodeRef::Shared(c) =
                    mem::replace(self, NodeRef::Placeholder(PhantomData))
                {
                    *self = NodeRef::Owned(c.clone());
                } else {
                    unreachable!()
                }

                self.deref_mut()
            }
            NodeRef::Placeholder(_) => unreachable!(),
        }
    }
}

impl<'a, C, S, const N: usize> Level<'a, C, S, N>
where
    C: Compound<S, N>,
    S: Store,
{
    /// Returs a reference to the handle pointing to the node below in the branch
    pub fn referencing(&self) -> Result<HandleRef<C, S, N>, S::Error> {
        self.node.handle(self.ofs)
    }

    /// Returns the offset of the reference node in the level of the branch
    /// i.e the node that references the level below.
    pub fn offset(&self) -> usize {
        self.ofs
    }

    fn new_owned(owned: C) -> Self {
        Level {
            ofs: 0,
            node: NodeRef::new_owned(owned),
        }
    }

    fn new_shared(shared: &'a C) -> Self {
        Level {
            ofs: 0,
            node: NodeRef::new_shared(shared),
        }
    }

    fn insert_child(&mut self, node: C) -> Result<(), S::Error> {
        match &mut self.node {
            NodeRef::Shared(c) => {
                self.node = NodeRef::Owned((c).clone());
                self.insert_child(node)?
            }
            NodeRef::Owned(o) => {
                o.children_mut()[self.ofs] = Handle::new_node(node)?
            }
            NodeRef::Placeholder(_) => unreachable!(),
            NodeRef::Mutable(ref mut m) => {
                m.children_mut()[self.ofs] = Handle::new_node(node)?
            }
        }
        Ok(())
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

    fn search<M: Method<C, S, N>>(
        &mut self,
        method: &mut M,
    ) -> Result<Found, S::Error> {
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

impl<C, S, const N: usize> AsMut<C> for Level<'_, C, S, N>
where
    C: Clone,
{
    fn as_mut(&mut self) -> &mut C {
        self.node.as_mut()
    }
}

impl<'a, C, S, const N: usize> RawBranch<'a, C, S, N>
where
    C: Compound<S, N>,
    S: Store,
{
    pub fn new_shared(node: &'a C) -> Self {
        let mut vec: ArrayVec<Level<'a, C, S, N>, N> = ArrayVec::new();
        vec.push(Level::new_shared(node));
        RawBranch {
            levels: vec,
            exact: false,
        }
    }

    pub fn new_mutable(node: &'a mut C) -> Self {
        let mut vec = ArrayVec::<_, N>::new();
        vec.push(Level::new_mutable(node));
        RawBranch {
            levels: vec,
            exact: false,
        }
    }

    pub(crate) fn exact(&self) -> bool {
        self.exact
    }

    pub fn search<M: Method<C, S, N>>(
        &mut self,
        method: &mut M,
    ) -> Result<(), S::Error> {
        self.exact = false;
        while let Some(last) = self.levels.last_mut() {
            let mut push = None;
            match last.search(method)? {
                Found::Leaf => {
                    self.exact = true;
                    break;
                }
                Found::Path => match last.referencing()? {
                    HandleRef::Node(shared) => {
                        let level: Level<'a, C, S, N> =
                            Level::new_owned(shared.clone());
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
        if self.exact {
            self.levels.last()?.leaf()
        } else {
            None
        }
    }

    pub(crate) fn leaf_mut(&mut self) -> Option<&'a mut C::Leaf> {
        if self.exact {
            unsafe {
                let unsafe_self: &'a mut Self = mem::transmute(self);
                unsafe_self.levels.last_mut()?.leaf_mut()
            }
        } else {
            None
        }
    }

    pub(crate) fn levels(&self) -> &[Level<'a, C, S, N>] {
        &self.levels
    }

    pub(crate) fn levels_mut(&mut self) -> &mut [Level<'a, C, S, N>] {
        &mut self.levels
    }

    fn pop_level(&mut self) -> bool {
        if let Some(popped) = self.levels.pop() {
            if !self.levels.is_empty() {
                if let NodeRef::Owned(o) = popped.node {
                    let last = self.levels.last_mut().expect("length < 1");
                    last.insert_child(o);
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
