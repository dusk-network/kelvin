// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::borrow::Cow;
use std::fmt;
use std::mem;
use std::ops::{Deref, DerefMut};

use canonical::{Canon, Repr, Store};
use canonical_derive::Canon;

use crate::annotations::ErasedAnnotation;
use crate::compound::Compound;

#[derive(Canon, Clone)]
enum HandleInner<C, S>
where
    C: Compound<S>,
    S: Store,
{
    Leaf(C::Leaf),
    Node(Repr<C, S>, C::Annotation),
    None,
}

#[derive(Debug, PartialEq, Eq)]
/// Represents the type of the handle
pub enum HandleType {
    /// Empty handle
    None,
    /// Leaf handle
    Leaf,
    /// Node handle
    Node,
}

/// The user-facing type for handles, the main type to build trees
#[derive(Clone, Canon)]
pub struct Handle<C, S>(HandleInner<C, S>)
where
    C: Compound<S>,
    S: Store;

impl<C, S> fmt::Debug for Handle<C, S>
where
    C: Compound<S> + fmt::Debug,
    S: Store,
    C::Leaf: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            HandleInner::None => write!(f, "None"),
            HandleInner::Leaf(ref l) => write!(f, "Leaf({:?})", l),
            HandleInner::Node(ref n, _) => write!(f, "Node({:?})", n.val()),
        }
    }
}

/// User facing reference to a handle
pub enum HandleRef<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Handle points at a Leaf
    Leaf(&'a C::Leaf),
    /// Handle points at a cached Node
    Node(Cow<'a, C>),
    /// Handle points at nothing
    None,
}

impl<'a, C, S> Drop for HandleMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn drop(&mut self) {
        if let HandleMut::Node(nodewrap) = self {
            if let HandleInner::Node(repr, ann) = nodewrap.inner {
                if let Ok(Some(annotation)) = repr.val().map(|v| v.annotation())
                {
                    *ann = annotation
                }
            }
        }
    }
}

impl<C, S> Default for Handle<C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn default() -> Self {
        Handle(HandleInner::None)
    }
}

/// A mutable reference to an empty `Handle`
pub struct HandleMutNone<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    inner: &'a mut HandleInner<C, S>,
}

/// A mutable reference to a `Handle` containing a leaf
pub struct HandleMutLeaf<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    inner: &'a mut HandleInner<C, S>,
}

/// A mutable reference to a `Handle` containing a node
pub struct HandleMutNode<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    inner: &'a mut HandleInner<C, S>,
}

impl<'a, C, S> Deref for HandleMutLeaf<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        match self.inner {
            HandleInner::Leaf(ref leaf) => leaf,
            _ => panic!("invalid deref after replace"),
        }
    }
}

impl<'a, C, S> DerefMut for HandleMutLeaf<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.inner {
            HandleInner::Leaf(ref mut leaf) => leaf,
            _ => panic!("invalid deref after replace"),
        }
    }
}

impl<'a, C, S> HandleMutNode<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Replaces the node with `handle`
    /// Invalidates the `HandleMutNone` if `handle` is not a node.
    pub fn replace(&mut self, handle: Handle<C, S>) -> Repr<C, S> {
        match mem::replace(self.inner, handle.0) {
            HandleInner::Node(n, _) => n,
            _ => panic!("multiple incompatible replaces"),
        }
    }

    /// Get a mutable reference to the underlying node in a closure
    pub fn val_mut<R, F>(&mut self, f: F) -> Result<R, S::Error>
    where
        F: Fn(&mut C) -> Result<R, S::Error>,
    {
        match self.inner {
            HandleInner::Node(ref mut n, _) => n.val_mut(f),
            _ => panic!("multiple incompatible replaces"),
        }
    }
}

impl<'a, C, S> HandleMutLeaf<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Replaces the leaf with `handle`
    /// Invalidates the `HandleMutNone` if `handle` is not None.
    pub fn replace(&mut self, handle: Handle<C, S>) -> C::Leaf {
        match mem::replace(self.inner, handle.0) {
            HandleInner::Leaf(l) => l,
            _ => panic!("multiple incompatible replaces"),
        }
    }
}

impl<'a, C, S> HandleMutNone<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Replaces the empty node with `handle`
    /// Invalidates the `HandleMutNone` if `handle` is not None.
    pub fn replace(&mut self, handle: Handle<C, S>) {
        *self.inner = handle.0
    }
}

/// A mutable reference to a handle
pub enum HandleMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Mutable handle pointing at a leaf
    Leaf(HandleMutLeaf<'a, C, S>),
    /// Mutable handle pointing at a node
    Node(HandleMutNode<'a, C, S>),
    /// Mutable handle pointing at an empty slot
    None(HandleMutNone<'a, C, S>),
}

impl<C, S> Handle<C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Constructs a new leaf Handle
    pub fn new_leaf(l: C::Leaf) -> Handle<C, S> {
        Handle(HandleInner::Leaf(l))
    }

    /// Constructs a new node Handle
    pub fn new_node(node: C) -> Result<Handle<C, S>, S::Error> {
        let ann = node.annotation().expect("Empty node handles are invalid");
        Ok(Handle(HandleInner::Node(Repr::new(node)?, ann)))
    }

    /// Constructs a new empty node Handle
    pub fn new_empty() -> Handle<C, S> {
        Handle(HandleInner::None)
    }

    /// Converts handle into leaf, panics on mismatching type
    pub fn into_leaf(self) -> C::Leaf {
        self.try_into_leaf().expect("Not a leaf")
    }

    /// Converts a leaf handle into its contained leaf, if any
    pub fn try_into_leaf(self) -> Option<C::Leaf> {
        if let HandleInner::Leaf(l) = self.0 {
            Some(l)
        } else {
            None
        }
    }

    /// Converts handle into leaf, panics on mismatching type
    pub fn into_node(self) -> Result<C, S::Error> {
        if let HandleInner::Node(n, _) = self.0 {
            n.unwrap_or_clone()
        } else {
            panic!("Not a node")
        }
    }

    /// Returns a reference to contained leaf, if any
    pub fn leaf(&self) -> Option<&C::Leaf> {
        match self.0 {
            HandleInner::Leaf(ref leaf) => Some(leaf),
            _ => None,
        }
    }

    /// Returns a mutable reference to contained leaf, if any
    pub fn leaf_mut(&mut self) -> Option<&mut C::Leaf> {
        match self.0 {
            HandleInner::Leaf(ref mut leaf) => Some(leaf),
            _ => None,
        }
    }

    /// Returns true if the Handle is pointing to nothing
    pub fn is_none(&self) -> bool {
        match self.0 {
            HandleInner::None => true,
            _ => false,
        }
    }

    /// Returns the type of the Handle
    pub fn handle_type(&self) -> HandleType {
        match self.0 {
            HandleInner::None => HandleType::None,
            HandleInner::Leaf(_) => HandleType::Leaf,
            _ => HandleType::Node,
        }
    }

    pub(crate) fn node_hash(&mut self) -> Result<Option<S::Ident>, S::Error> {
        Ok(match self.0 {
            HandleInner::None => None,
            HandleInner::Leaf(_) => None,
            HandleInner::Node(ref mut n, ..) => Some(n.get_id()?),
        })
    }

    /// Return the annotation for the handle, unless None
    pub fn annotation(&self) -> Option<Cow<C::Annotation>> {
        match self.0 {
            HandleInner::None => None,
            HandleInner::Leaf(ref l) => {
                Some(Cow::Owned(C::Annotation::from(l)))
            }
            HandleInner::Node(_, ref ann) => Some(Cow::Borrowed(ann)),
        }
    }

    /// Returns a HandleRef from the Handle
    pub fn inner(&self) -> Result<HandleRef<C, S>, S::Error> {
        Ok(match self.0 {
            HandleInner::None => HandleRef::None,
            HandleInner::Leaf(ref l) => HandleRef::Leaf(l),
            HandleInner::Node(ref n, _) => HandleRef::Node(n.val()?),
        })
    }

    /// Returns a mutable reference to the `Handle` as `HandleMut`
    pub fn inner_mut(&mut self) -> Result<HandleMut<C, S>, S::Error> {
        match self.0 {
            HandleInner::None => {
                Ok(HandleMut::None(HandleMutNone { inner: &mut self.0 }))
            }
            HandleInner::Leaf(_) => {
                Ok(HandleMut::Leaf(HandleMutLeaf { inner: &mut self.0 }))
            }
            HandleInner::Node(..) => {
                Ok(HandleMut::Node(HandleMutNode { inner: &mut self.0 }))
            }
        }
    }
}

impl<C, S> ErasedAnnotation<C::Annotation> for Handle<C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn annotation(&self) -> Option<Cow<C::Annotation>> {
        self.annotation()
    }
}
