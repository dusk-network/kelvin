use std::io::{self, Read, Write};
use std::mem;
use std::sync::Arc;

use bytehash::ByteHash;
use cache::Cached;

use crate::content::Content;
use crate::sink::StoreSink;
use crate::store::{Snapshot, Store};

enum HandleInner<C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    Leaf(C::Leaf),
    Node(Box<C>),
    SharedNode(Arc<C>),
    Persisted(Snapshot<C, H>),
    None,
}

#[derive(Clone)]
pub struct Handle<C, H>(HandleInner<C, H>)
where
    C: Content<H>,
    H: ByteHash;

pub enum HandleRef<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    Leaf(&'a C::Leaf),
    Node(Cached<'a, C>),
    None,
}

pub enum HandleMut<'a, C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    Leaf(&'a mut C::Leaf),
    Node(&'a mut C),
    None,
}

pub enum HandleOwned<C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    Leaf(C::Leaf),
    Node(C),
    None,
}

impl<C, H> Default for Handle<C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    fn default() -> Self {
        Handle(HandleInner::None)
    }
}

impl<C, H> Clone for HandleInner<C, H>
where
    C: Content<H>,
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
    C: Content<H>,
    H: ByteHash,
{
    type Leaf = ();

    fn persist(&mut self, sink: &mut dyn Write) -> io::Result<()> {
        self.0.persist(sink)
    }

    fn restore(source: &mut dyn Read) -> io::Result<Self> {
        Ok(Handle(HandleInner::restore(source)?))
    }
}

impl<C, H> Content<H> for HandleInner<C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    type Leaf = ();

    fn persist(&mut self, sink: &mut dyn Write) -> io::Result<()> {
        match self {
            HandleInner::Persisted(ref digest) => {
                sink.write_all(&[0])?;
                sink.write_all((**digest).as_ref())
            }
            HandleInner::Leaf(ref mut leaf) => {
                sink.write_all(&[255])?;
                leaf.persist(sink)
            }
            _ => panic!("Attempt at persisting a non-hash Handle"),
        }
    }

    fn restore(source: &mut dyn Read) -> io::Result<Self> {
        let mut tag = [0u8];
        source.read_exact(&mut tag)?;
        match tag {
            [0] => {
                let mut h = H::Digest::default();
                source.read(h.as_mut())?;
                Ok(HandleInner::Persisted(Snapshot::new(h)))
            }
            // We leave the rest of the bytes for future hash-function
            // upgrades
            [255] => Ok(HandleInner::Leaf(C::Leaf::restore(source)?)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid Handle encoding",
            )),
        }
    }
}

impl<C, H> Handle<C, H>
where
    C: Content<H>,
    H: ByteHash,
{
    pub fn leaf(l: C::Leaf) -> Handle<C, H> {
        Handle(HandleInner::Leaf(l))
    }

    pub fn node(n: C) -> Handle<C, H> {
        Handle(HandleInner::Node(Box::new(n)))
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

    pub fn inner(&self) -> HandleRef<C, H> {
        match self.0 {
            HandleInner::None => HandleRef::None,
            HandleInner::Leaf(ref l) => HandleRef::Leaf(l),
            HandleInner::Node(ref n) => {
                HandleRef::Node(Cached::Borrowed(n.as_ref()))
            }
            HandleInner::SharedNode(ref n) => {
                HandleRef::Node(Cached::Borrowed(n.as_ref()))
            }
            _ => unimplemented!(),
        }
    }

    pub fn inner_mut(&mut self) -> HandleMut<C, H> {
        match self.0 {
            HandleInner::None => HandleMut::None,
            HandleInner::Leaf(ref mut l) => HandleMut::Leaf(l),
            HandleInner::Node(ref mut n) => HandleMut::Node(n.as_mut()),
            _ => unimplemented!(),
        }
    }

    pub fn pre_persist(&mut self, store: &Store<H>) -> io::Result<()> {
        let hash = match &mut self.0 {
            HandleInner::Node(node) => {
                let mut sink = StoreSink::new(store);
                node.persist(&mut sink)?;
                sink.fin()?
            }
            HandleInner::SharedNode(ref mut arc) => {
                let mut sink = StoreSink::new(store);
                Arc::make_mut(arc).persist(&mut sink)?;
                sink.fin()?
            }
            HandleInner::Leaf(_)
            | HandleInner::None
            | HandleInner::Persisted(_) => return Ok(()),
        };
        *self = Handle(HandleInner::Persisted(Snapshot::new(hash)));
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
