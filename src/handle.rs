use std::io::{self, Read, Write};
use std::mem;
use std::sync::Arc;

use bytehash::ByteHash;
use cache::Cached;

use crate::content::Content;
use crate::sink::StoreSink;
use crate::store::{Snapshot, Store};

pub enum Handle<L, N, H: ByteHash> {
    Leaf(L),
    Node(Box<N>),
    SharedNode(Arc<N>),
    Persisted(Snapshot<N, H>),
    None,
}

impl<L, N, H: ByteHash> Default for Handle<L, N, H> {
    fn default() -> Self {
        Handle::None
    }
}

impl<L: Clone, N: Clone, H: ByteHash> Clone for Handle<L, N, H> {
    fn clone(&self) -> Self {
        match self {
            Handle::Leaf(ref l) => Handle::Leaf(l.clone()),
            Handle::Node(ref n) => Handle::Node(n.clone()),
            Handle::SharedNode(ref arc) => Handle::SharedNode(arc.clone()),
            Handle::Persisted(ref snap) => Handle::Persisted(snap.clone()),
            Handle::None => Handle::None,
        }
    }
}

impl<L: Content<H>, N: Content<H>, H: ByteHash> Content<H> for Handle<L, N, H> {
    type Leaf = L;
    type Node = N;

    fn persist(&mut self, sink: &mut dyn Write) -> io::Result<()> {
        // TODO: Refactor this
        match self {
            Handle::Persisted(ref digest) => {
                sink.write_all((**digest).as_ref())
            }
            _ => panic!("Attempt at persisting a non-hash Handle"),
        }
    }

    fn restore(source: &mut dyn Read) -> io::Result<Self> {
        let mut h = H::Digest::default();
        source.read(h.as_mut())?;
        Ok(Handle::Persisted(Snapshot::new(h)))
    }
}

impl<L: Content<H>, N: Content<H>, H: ByteHash> Handle<L, N, H> {
    pub fn leaf(t: L) -> Handle<L, N, H> {
        Handle::Leaf(t)
    }

    pub fn pre_persist(&mut self, store: &Store<H>) -> io::Result<()> {
        let hash = match self {
            Handle::Leaf(ref mut leaf) => {
                let mut sink = StoreSink::new(store);
                leaf.persist(&mut sink)?;
                sink.fin()?
            }
            Handle::Node(node) => {
                let mut sink = StoreSink::new(store);
                node.persist(&mut sink)?;
                sink.fin()?
            }
            Handle::SharedNode(ref mut arc) => unsafe {
                let mut sink = StoreSink::new(store);
                Arc::make_mut(arc).persist(&mut sink)?;
                sink.fin()?
            },
            Handle::None | Handle::Persisted(_) => return Ok(()),
        };
        *self = Handle::Persisted(Snapshot::new(hash));
        Ok(())
    }

    pub fn get<'a>(&'a self, store: &'a Store<H>) -> io::Result<Cached<'a, L>> {
        match self {
            Handle::Leaf(ref val) => Ok(Cached::Borrowed(val)),
            _ => panic!("Attempt to get non-Leaf value"),
        }
    }

    pub fn make_shared(&mut self) {
        if let Handle::Node(_) = self {
            if let Handle::Node(node) = mem::replace(self, Handle::None) {
                *self = Handle::SharedNode(Arc::new(*node))
            } else {
                unreachable!()
            }
        } else {
            // no-op
        }
    }

    pub fn get_mut(&mut self, store: &Store<H>) -> io::Result<&mut L> {
        match self {
            Handle::Leaf(ref mut val) => Ok(val),
            _ => panic!("Attempt to get_mut non-Leaf value"),
        }
    }
}
