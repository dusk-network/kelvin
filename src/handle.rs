use std::io::{self, Read, Write};
use std::mem;
use std::sync::Arc;

use bytehash::ByteHash;
use cache::Cached;

use crate::compound::Compound;
use crate::content::Content;
use crate::sink::Sink;
use crate::source::Source;
use crate::store::{Snapshot, Store};

enum HandleInner<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Leaf(C::Leaf),
    Node(Box<C>),
    SharedNode(Arc<C>),
    Persisted(Snapshot<C, H>),
    None,
}

pub enum HandleType {
    None,
    Leaf,
    Node,
}

#[derive(Clone)]
pub struct Handle<C, H>(HandleInner<C, H>)
where
    C: Compound<H>,
    H: ByteHash;

pub enum HandleRef<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Leaf(&'a C::Leaf),
    Node(Cached<'a, C>),
    None,
}

pub enum HandleMut<'a, C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Leaf(&'a mut C::Leaf),
    Node(&'a mut C),
    None,
}

pub enum HandleOwned<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    Leaf(C::Leaf),
    Node(C),
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

impl<C, H> PartialEq for Handle<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (HandleInner::None, HandleInner::None) => true,
            (HandleInner::Leaf(a), HandleInner::Leaf(b)) => a == b,
            _ => false,
        }
    }
}

impl<C, H> Eq for Handle<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
}

impl<C, H> Clone for HandleInner<C, H>
where
    C: Compound<H>,
    H: ByteHash,
{
    fn clone(&self) -> Self {
        match self {
            HandleInner::Leaf(ref l) => HandleInner::Leaf(l.clone()),
            HandleInner::Node(ref n) => HandleInner::Node(n.clone()),
            HandleInner::SharedNode(ref arc) => {
                HandleInner::SharedNode(arc.clone())
            }
            HandleInner::Persisted(ref snap) => {
                HandleInner::Persisted(snap.clone())
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
            HandleInner::Persisted(ref digest) => {
                sink.write_all(&[2])?;
                sink.write_all((**digest).as_ref())
            }
            HandleInner::SharedNode(_) => unimplemented!(),
            // Pre-persist handles this case
            HandleInner::Node(_) => unreachable!(),
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
                Ok(Handle(HandleInner::Persisted(Snapshot::new(
                    h,
                    source.store(),
                ))))
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
    pub fn new_leaf(l: C::Leaf) -> Handle<C, H> {
        Handle(HandleInner::Leaf(l))
    }

    pub fn new_node<I: Into<Box<C>>>(n: I) -> Handle<C, H> {
        Handle(HandleInner::Node(n.into()))
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        match self.0 {
            HandleInner::Leaf(ref leaf) => Some(leaf),
            _ => None,
        }
    }

    pub fn leaf_mut(&mut self) -> Option<&mut C::Leaf> {
        match self.0 {
            HandleInner::Leaf(ref mut leaf) => Some(leaf),
            _ => None,
        }
    }

    pub fn is_none(&self) -> bool {
        match self.0 {
            HandleInner::None => true,
            _ => false,
        }
    }

    pub fn handle_type(&self) -> HandleType {
        match self.0 {
            HandleInner::None => HandleType::None,
            HandleInner::Leaf(_) => HandleType::Leaf,
            _ => HandleType::Node,
        }
    }

    pub fn replace(&mut self, with: HandleOwned<C, H>) -> Option<C::Leaf> {
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
                if let HandleInner::Leaf(leaf) =
                    mem::replace(&mut self.0, HandleInner::Node(Box::new(node)))
                {
                    Some(leaf)
                } else {
                    None
                }
            }
        }
    }

    pub fn inner(&self) -> io::Result<HandleRef<C, H>> {
        Ok(match self.0 {
            HandleInner::None => HandleRef::None,
            HandleInner::Leaf(ref l) => HandleRef::Leaf(l),
            HandleInner::Node(ref n) => {
                HandleRef::Node(Cached::Borrowed(n.as_ref()))
            }
            HandleInner::SharedNode(ref n) => {
                HandleRef::Node(Cached::Borrowed(n.as_ref()))
            }
            HandleInner::Persisted(ref snap) => {
                let restored = snap.restore()?;
                HandleRef::Node(Cached::Spilled(Box::new(restored)))
            }
        })
    }

    pub fn inner_mut(&mut self) -> io::Result<HandleMut<C, H>> {
        Ok(match self.0 {
            HandleInner::None => HandleMut::None,
            HandleInner::Leaf(ref mut l) => HandleMut::Leaf(l),
            HandleInner::Node(ref mut n) => HandleMut::Node(n.as_mut()),
            HandleInner::Persisted(ref snapshot) => {
                let restored = snapshot.restore()?;
                *self = Handle(HandleInner::Node(Box::new(restored)));
                self.inner_mut()?
            }
            _ => unimplemented!(),
        })
    }

    pub(crate) fn pre_persist(&mut self, store: &Store<H>) -> io::Result<()> {
        let hash = match &mut self.0 {
            HandleInner::Node(node) => {
                for child in node.children_mut() {
                    child.pre_persist(store)?;
                }
                let mut sink = Sink::new(store);
                node.persist(&mut sink)?;
                sink.fin()?
            }
            HandleInner::SharedNode(_) => unimplemented!(),
            HandleInner::Leaf(_)
            | HandleInner::None
            | HandleInner::Persisted(_) => {
                // no pre-persist needed
                return Ok(());
            }
        };
        *self = Handle(HandleInner::Persisted(Snapshot::new(hash, store)));
        Ok(())
    }

    pub fn make_shared(&mut self) {
        if let HandleInner::Node(_) = self.0 {
            if let HandleInner::Node(node) =
                mem::replace(&mut self.0, HandleInner::None)
            {
                self.0 = HandleInner::SharedNode(Arc::new(*node))
            } else {
                unreachable!()
            }
        } else {
            // no-op
        }
    }
}
