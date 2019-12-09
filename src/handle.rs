use std::borrow::Cow;
use std::io::{self, Read, Write};
use std::mem;
use std::sync::Arc;

use bytehash::ByteHash;
use cache::Cached;

use crate::annotations::Annotation;
use crate::compound::Compound;
use crate::content::Content;
use crate::sink::Sink;
use crate::source::Source;
use crate::store::Snapshot;

enum HandleInner<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Leaf(C::Leaf),
    Node(Box<C>, C::Annotation),
    SharedNode(Arc<C>, C::Annotation),
    Persisted(Snapshot<C, H>, C::Annotation),
    None,
}

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
#[derive(Clone)]
pub struct Handle<C, H>(HandleInner<C, H>)
where
    C: Compound<H>,
    H: ByteHash;

/// User facing reference to a handle
pub enum HandleRef<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Handle points at a Leaf
    Leaf(&'a C::Leaf),
    /// Handle points at a cached Node
    Node(Cached<'a, C>),
    /// Handle points at nothing
    None,
}

/// User facing mutable reference to a handle
pub enum HandleMut<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Handle points at a Leaf
    Leaf(&'a mut C::Leaf),
    /// Handle points at a Node
    Node(&'a mut C),
    /// Handle points at nothing
    None,
}

/// An owned handle
pub enum HandleOwned<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Owned Leaf
    Leaf(C::Leaf),
    /// Owned Node
    Node(C),
    /// None
    None,
}

impl<C, H> Default for Handle<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn default() -> Self {
        Handle(HandleInner::None)
    }
}

impl<C, H> Clone for HandleInner<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn clone(&self) -> Self {
        match self {
            HandleInner::Leaf(ref l) => HandleInner::Leaf(l.clone()),
            HandleInner::Node(ref n, ref ann) => {
                HandleInner::Node(n.clone(), ann.clone())
            }
            HandleInner::SharedNode(ref arc, ref ann) => {
                HandleInner::SharedNode(arc.clone(), ann.clone())
            }
            HandleInner::Persisted(ref snap, ref ann) => {
                HandleInner::Persisted(snap.clone(), ann.clone())
            }
            HandleInner::None => HandleInner::None,
        }
    }
}

impl<C, H> Content<H> for Handle<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        match self.0 {
            HandleInner::None => sink.write_all(&[0]),
            HandleInner::Leaf(ref mut leaf) => {
                sink.write_all(&[1])?;
                leaf.persist(sink)
            }
            HandleInner::Persisted(ref digest, ref mut ann) => {
                sink.write_all(&[2])?;
                sink.write_all((**digest).as_ref())?;
                ann.persist(sink)
            }
            HandleInner::Node(ref mut node, ref ann) => {
                let snap = sink.store().persist(&mut **node)?;
                self.0 = HandleInner::Persisted(snap, ann.clone());
                self.persist(sink)
            }
            HandleInner::SharedNode(_, _) => unimplemented!(),
        }
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut tag = [0u8];
        source.read_exact(&mut tag)?;
        match tag {
            [0] => Ok(Handle(HandleInner::None)),
            [1] => Ok(Handle(HandleInner::Leaf(C::Leaf::restore(source)?))),
            [2] => {
                let mut h = H::Digest::default();
                source.read(h.as_mut())?;
                Ok(Handle(HandleInner::Persisted(
                    Snapshot::new(h, source.store()),
                    C::Annotation::restore(source)?,
                )))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid Handle encoding",
            )),
        }
    }
}

impl<C, H> Handle<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    /// Constructs a new leaf Handle
    pub fn new_leaf(l: C::Leaf) -> Handle<C, H> {
        Handle(HandleInner::Leaf(l))
    }

    /// Constructs a new node Handle
    pub fn new_node<I: Into<Box<C>>>(n: I) -> Handle<C, H> {
        let node = n.into();
        let ann = node.annotation().expect("Empty node handles are invalid");
        Handle(HandleInner::Node(node, ann))
    }

    /// Returns a reference to contained leaf, if any
    pub(crate) fn leaf(&self) -> Option<&C::Leaf> {
        match self.0 {
            HandleInner::Leaf(ref leaf) => Some(leaf),
            _ => None,
        }
    }

    /// Returns a mutable reference to contained leaf, if any
    pub(crate) fn leaf_mut(&mut self) -> Option<&mut C::Leaf> {
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

    /// Return the annotation for the handle, unless None
    pub fn annotation(&self) -> Option<Cow<C::Annotation>> {
        match self.0 {
            HandleInner::None => None,
            HandleInner::Leaf(ref l) => {
                Some(Cow::Owned(C::Annotation::from(l)))
            }
            HandleInner::Node(_, ref ann)
            | HandleInner::SharedNode(_, ref ann)
            | HandleInner::Persisted(_, ref ann) => Some(Cow::Borrowed(ann)),
        }
    }

    pub(crate) fn update_annotation(&mut self) {
        // Only the case of Node needs an updated annotation, since it's the only case
        // that has an annotation and could have been updated
        match self.0 {
            HandleInner::Node(ref node, ref mut ann) => {
                *ann = node.annotation().expect("Invalid empty node handle")
            }
            _ => (),
        }
    }

    /// Returns a HandleRef from the Handle
    pub fn inner(&self) -> io::Result<HandleRef<C, H>> {
        Ok(match self.0 {
            HandleInner::None => HandleRef::None,
            HandleInner::Leaf(ref l) => HandleRef::Leaf(l),
            HandleInner::Node(ref n, _) => {
                HandleRef::Node(Cached::Borrowed(n.as_ref()))
            }
            HandleInner::SharedNode(ref n, _) => {
                HandleRef::Node(Cached::Borrowed(n.as_ref()))
            }
            HandleInner::Persisted(ref snap, _) => {
                let restored = snap.restore()?;
                HandleRef::Node(Cached::Spilled(Box::new(restored)))
            }
        })
    }

    pub(crate) fn replace(
        &mut self,
        with: HandleOwned<C, H>,
    ) -> Option<C::Leaf> {
        match with {
            HandleOwned::None => {
                if let HandleInner::Leaf(replaced) =
                    mem::replace(&mut self.0, HandleInner::None)
                {
                    Some(replaced)
                } else {
                    None
                }
            }
            HandleOwned::Leaf(leaf) => {
                if let HandleInner::Leaf(replaced) =
                    mem::replace(&mut self.0, HandleInner::Leaf(leaf))
                {
                    Some(replaced)
                } else {
                    None
                }
            }
            HandleOwned::Node(node) => {
                let ann =
                    node.annotation().expect("Empty node handles are invalid");
                if let HandleInner::Leaf(leaf) = mem::replace(
                    &mut self.0,
                    HandleInner::Node(Box::new(node), ann),
                ) {
                    Some(leaf)
                } else {
                    None
                }
            }
        }
    }

    // Should NOT be called directly by datastructure code
    pub(crate) fn inner_mut(&mut self) -> io::Result<HandleMut<C, H>> {
        Ok(match self.0 {
            HandleInner::None => HandleMut::None,
            HandleInner::Leaf(ref mut l) => HandleMut::Leaf(l),
            HandleInner::Node(ref mut n, _) => HandleMut::Node(&mut **n),
            HandleInner::Persisted(_, _) => {
                if let HandleInner::Persisted(snap, ann) =
                    mem::replace(&mut self.0, HandleInner::None)
                {
                    let restored = snap.restore()?;
                    *self = Handle(HandleInner::Node(Box::new(restored), ann));
                    return self.inner_mut();
                } else {
                    unreachable!()
                }
            }
            _ => unimplemented!(),
        })
    }

    #[doc(hidden)]
    pub fn make_shared(&mut self) {
        if let HandleInner::Node(_, _) = self.0 {
            if let HandleInner::Node(node, ann) =
                mem::replace(&mut self.0, HandleInner::None)
            {
                self.0 = HandleInner::SharedNode(Arc::new(*node), ann)
            } else {
                unreachable!()
            }
        } else {
            // no-op
        }
    }
}

impl<C, H> Annotation<C::Annotation> for Handle<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn annotation(&self) -> Option<Cow<C::Annotation>> {
        self.annotation()
    }
}
