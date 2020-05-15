use std::borrow::Cow;
use std::fmt;
use std::io::{self, Read, Write};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use bytehash::ByteHash;
use cache::Cached;

use crate::annotations::ErasedAnnotation;
use crate::compound::Compound;
use crate::content::Content;
use crate::debug_draw::{DebugDraw, DrawState};
use crate::sink::Sink;
use crate::source::Source;
use crate::store::Snapshot;

pub trait RcExt<T> {
    fn unwrap_or_clone(self) -> T;
}

impl<T: Clone> RcExt<T> for Rc<T> {
    fn unwrap_or_clone(self) -> T {
        match Rc::try_unwrap(self) {
            Ok(t) => t,
            Err(rc) => (*rc).clone(),
        }
    }
}

enum HandleInner<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Leaf(C::Leaf),
    Node(Rc<C>, C::Annotation, Option<H::Digest>),
    Persisted(Snapshot<C, H>, C::Annotation),
    None,
}

impl<C, H> From<HandleOwned<C, H>> for HandleInner<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn from(owned: HandleOwned<C, H>) -> Self {
        match owned {
            HandleOwned::None => HandleInner::None,
            HandleOwned::Leaf(l) => HandleInner::Leaf(l),
            HandleOwned::Node(c) => {
                let ann = c.annotation().expect("Invalid empty owned node");
                HandleInner::Node(Rc::new(c), ann, None)
            }
        }
    }
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
#[derive(Clone)]
pub struct Handle<C, H>(HandleInner<C, H>)
where
    C: Compound<H>,
    H: ByteHash;

impl<C, H> fmt::Debug for Handle<C, H>
where
    C: Compound<H>,
    H: ByteHash,
    C::Leaf: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            HandleInner::None => write!(f, "None"),
            HandleInner::Leaf(ref l) => write!(f, "Leaf({:?})", l),
            _ => write!(f, "Node"),
        }
    }
}

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

/// Struct that wraps a user-facing representation of the mutable handle
/// while still keeping track of a raw pointer to the original handle.
/// This is to ensure that the associated annotations are automatically
/// updated on mutable access.
pub struct HandleMutWrap<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    annotation: Option<&'a mut C::Annotation>,
    inner: HandleMut<'a, C, H>,
}

impl<'a, C, H> Deref for HandleMutWrap<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    type Target = HandleMut<'a, C, H>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, C, H> DerefMut for HandleMutWrap<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a, C, H> Drop for HandleMutWrap<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn drop(&mut self) {
        if let (Some(ref mut ann), HandleMut::Node(ref node)) =
            (self.annotation.as_mut(), &self.inner)
        {
            if let Some(annotation) = node.annotation() {
                ***ann = annotation
            }
        }
    }
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
            HandleInner::Node(ref n, ref ann, ref cached) => {
                HandleInner::Node(n.clone(), ann.clone(), *cached)
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
            HandleInner::Persisted(ref hash, ref mut ann) => {
                sink.write_all(&[2])?;
                sink.write_all((**hash).as_ref())?;
                ann.persist(sink)
            }
            HandleInner::Node(ref mut node, ref mut ann, ref mut cached) => {
                match sink.store() {
                    Some(store) => {
                        // We need to write the data to the backing store

                        // Create new sink sharing the store, either with the cached
                        // hash or post-hashing
                        let mut sub_sink = match *cached {
                            Some(hash) => {
                                debug_assert!({
                                    let mut sub_sink = Sink::new_dry();
                                    Rc::make_mut(node)
                                        .persist(&mut sub_sink)?;
                                    sub_sink.fin()? == hash
                                });
                                Sink::new_cached(hash, store)
                            }
                            None => Sink::new(store),
                        };

                        // Persist the node to the sub-sink
                        Rc::make_mut(node).persist(&mut sub_sink)?;
                        let hash = sub_sink.fin()?;

                        // update the handle to a persisted reference
                        let snap = Snapshot::new(hash, store);
                        self.0 = HandleInner::Persisted(snap, ann.clone());
                        // recurse to drop the borrow of ann
                        // and hit the Persisted match arm
                        self.persist(sink)
                    }
                    None => {
                        // No store, we're doing a dry run
                        let hash = match *cached {
                            Some(hash) => {
                                debug_assert!({
                                    let mut sub_sink = Sink::new_dry();
                                    Rc::make_mut(node)
                                        .persist(&mut sub_sink)?;
                                    sub_sink.fin()? == hash
                                });
                                hash
                            }
                            None => {
                                let mut sub_sink = Sink::new_dry();
                                Rc::make_mut(node).persist(&mut sub_sink)?;
                                let hash = sub_sink.fin()?;
                                // Update our hash cache
                                *cached = Some(hash);
                                hash
                            }
                        };
                        // In a dry run, we write the hash as if it was persisted
                        sink.write_all(&[2])?;
                        sink.write_all(hash.as_ref())?;
                        ann.persist(sink)
                    }
                }
            }
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
                source.read_exact(h.as_mut())?;
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
    pub fn new_node<I: Into<Rc<C>>>(n: I) -> Handle<C, H> {
        let node = n.into();
        let ann = node.annotation().expect("Empty node handles are invalid");
        Handle(HandleInner::Node(node, ann, None))
    }

    /// Constructs a new empty node Handle
    pub fn new_empty() -> Handle<C, H> {
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
    pub fn into_node(self) -> C {
        if let HandleInner::Node(n, _, _) = self.0 {
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

    pub(crate) fn node_hash(&mut self) -> Option<H::Digest> {
        match self.0 {
            HandleInner::None => None,
            HandleInner::Leaf(_) => None,
            HandleInner::Node(ref mut n, ..) => {
                Some(Rc::make_mut(n).root_hash())
            }
            HandleInner::Persisted(ref hash, ..) => Some(*hash.hash()),
        }
    }

    /// Return the annotation for the handle, unless None
    pub fn annotation(&self) -> Option<Cow<C::Annotation>> {
        match self.0 {
            HandleInner::None => None,
            HandleInner::Leaf(ref l) => {
                Some(Cow::Owned(C::Annotation::from(l)))
            }
            HandleInner::Node(_, ref ann, _)
            | HandleInner::Persisted(_, ref ann) => Some(Cow::Borrowed(ann)),
        }
    }

    /// Returns a HandleRef from the Handle
    pub fn inner(&self) -> io::Result<HandleRef<C, H>> {
        Ok(match self.0 {
            HandleInner::None => HandleRef::None,
            HandleInner::Leaf(ref l) => HandleRef::Leaf(l),
            HandleInner::Node(ref n, _, _) => {
                HandleRef::Node(Cached::Borrowed(n.as_ref()))
            }
            HandleInner::Persisted(ref snap, _) => {
                let restored = snap.restore()?;
                HandleRef::Node(Cached::Spilled(Box::new(restored)))
            }
        })
    }

    /// Replace the inner node
    pub fn replace(&mut self, with: HandleOwned<C, H>) -> HandleOwned<C, H> {
        match mem::replace(&mut self.0, with.into()) {
            HandleInner::None => HandleOwned::None,
            HandleInner::Leaf(l) => HandleOwned::Leaf(l),
            HandleInner::Node(c, _, _) => {
                HandleOwned::Node(c.unwrap_or_clone())
            }
            _ => unreachable!("Mutable handles cannot be persisted or shared"),
        }
    }

    /// Get a wrapped mutable reference to the inner node
    pub fn inner_mut(&mut self) -> io::Result<HandleMutWrap<C, H>> {
        Ok(match self.0 {
            HandleInner::None => HandleMutWrap {
                annotation: None,
                inner: HandleMut::None,
            },
            HandleInner::Leaf(ref mut l) => HandleMutWrap {
                annotation: None,
                inner: HandleMut::Leaf(l),
            },
            HandleInner::Node(ref mut n, ref mut ann, ref mut cached) => {
                // Clear cached hash
                *cached = None;
                HandleMutWrap {
                    annotation: Some(ann),
                    inner: HandleMut::Node(&mut *Rc::make_mut(n)),
                }
            }
            HandleInner::Persisted(_, _) => {
                if let HandleInner::Persisted(snap, ann) =
                    mem::replace(&mut self.0, HandleInner::None)
                {
                    let restored = snap.restore()?;
                    *self =
                        Handle(HandleInner::Node(Rc::new(restored), ann, None));
                    return self.inner_mut();
                } else {
                    unreachable!()
                }
            }
        })
    }
}

impl<C, H> ErasedAnnotation<C::Annotation> for Handle<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn annotation(&self) -> Option<Cow<C::Annotation>> {
        self.annotation()
    }
}

impl<C, H> Handle<C, H>
where
    C: Compound<H> + DebugDraw<H>,
    C::Leaf: std::fmt::Debug,
    H: ByteHash,
{
    /// Draw contents of handle, for debug use
    pub fn draw_conf(&self, state: &mut DrawState) -> String {
        match self.0 {
            HandleInner::None => "□ ".to_string(),
            HandleInner::Leaf(ref l) => format!("{:?} ", l),
            HandleInner::Node(ref n, _, _) => {
                state.recursion += 1;
                format!("\n{}{}", state.pad(), {
                    let res = n.draw_conf(state);
                    state.recursion -= 1;
                    res
                })
            }
            _ => unimplemented!(),
        }
    }
}
