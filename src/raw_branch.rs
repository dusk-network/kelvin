// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::ops::{Deref, DerefMut};

use canonical::{Repr, Store};

use crate::compound::Compound;
use crate::handle::{Handle, HandleRef};
use crate::search::{Method, SearchResult};

pub enum Found {
    Leaf,
    Path,
    None,
}

/// Represents a level in a branch
#[derive(Debug)]
pub struct Level<C, S>
where
    S: Store,
    C: Clone,
{
    ofs: usize,
    node: Repr<C, S>,
}

// impl<C, S> Deref for Level<C, S>
// where
//     C: Compound<S>,
//     S: Store,
// {
//     type Target = C;

//     fn deref(&self) -> &Self::Target {
//         &self.node
//     }
// }

// impl<C, S> DerefMut for Level<C, S>
// where
//     C: Compound<S>,
//     S: Store,
// {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.node
//     }
// }

#[derive(Debug)]
pub(crate) struct RawBranch<C, S>
where
    C: Clone,
    S: Store,
{
    levels: Vec<Level<C, S>>,
    exact: bool,
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

impl<C, S> Level<C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn new(node: Repr<C, S>) -> Self {
        Level { ofs: 0, node: node }
    }

    /// The node of the level
    pub fn node(&self) -> &Repr<C, S> {
        &self.node
    }

    /// Returs a reference to the handle pointing to the node below in the branch
    pub fn referencing(&self) -> Result<HandleRef<C, S>, S::Error> {
        todo!()
    }

    /// Returns the offset of the reference node in the level of the branch
    /// i.e the node that references the level below.
    pub fn offset(&self) -> usize {
        self.ofs
    }

    fn insert_child(&mut self, node: C) -> Result<(), S::Error> {
        self.node.val_mut()?.children_mut()[self.ofs] = Handle::new_node(node)?;
        Ok(())
    }

    // fn inner(&self) -> InnerImmutable<C> {
    //     self.node.inner()
    // }

    fn leaf(&self) -> Result<Option<&C::Leaf>, S::Error> {
        Ok(self
            .node
            .val()?
            .children()
            .get(self.ofs)
            .and_then(|handle| handle.leaf()))
    }

    fn leaf_mut(&mut self) -> Result<Option<&mut C::Leaf>, S::Error> {
        Ok(self
            .node
            .val_mut()?
            .children_mut()
            .get_mut(self.ofs)
            .and_then(|handle| handle.leaf_mut()))
    }

    fn search<M: Method<C, S>>(
        &mut self,
        method: &mut M,
    ) -> Result<Found, S::Error> {
        let node = self.node.val()?;
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

impl<C, S> RawBranch<C, S>
where
    C: Compound<S>,
    S: Store,
{
    pub fn new(node: C) -> Result<Self, S::Error> {
        let mut vec = Vec::new();
        let repr = Repr::new(node)?;
        vec.push(Level::new(repr));
        Ok(RawBranch {
            levels: vec,
            exact: false,
        })
    }

    pub(crate) fn exact(&self) -> bool {
        self.exact
    }

    pub fn search<M: Method<C, S>>(
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
                        push = Some(Level::new(shared));
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

    pub fn leaf(&self) -> Result<Option<&C::Leaf>, S::Error> {
        if self.exact {
            self.levels.last().expect("Invalid brach").leaf()
        } else {
            Ok(None)
        }
    }

    pub(crate) fn leaf_mut(
        &mut self,
    ) -> Result<Option<&mut C::Leaf>, S::Error> {
        if self.exact {
            self.levels.last_mut()?.leaf_mut()
        } else {
            None
        }
    }

    pub(crate) fn levels(&self) -> &[Level<C, S>] {
        &self.levels
    }

    pub(crate) fn levels_mut(&mut self) -> &mut [Level<C, S>] {
        &mut self.levels
    }

    fn pop_level(&mut self) -> bool {
        if let Some(popped) = self.levels.pop() {
            if !self.levels.is_empty() {
                let mut insert = popped.node.val_mut()?;
                // insert popped level as Handle::Node in Node above.
                let last = self.levels.last_mut().expect("length < 1");
                last.insert_child(insert).expect("FIXME: pop failed");
            }
            true
        } else {
            false
        }
    }

    /// Makes sure all owned nodes in the branch are inserted into the tree.
    pub(crate) fn relink(&mut self) -> Result<(), S::Error> {
        while self.pop_level()? {}
        Ok(())
    }
}
