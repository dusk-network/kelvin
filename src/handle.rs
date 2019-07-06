use std::cell::UnsafeCell;
use std::io::{self, Read, Write};
use std::mem;
use std::sync::Arc;

use bytehash::ByteHash;
use cache::Cached;

use crate::content::Content;
use crate::sink::StoreSink;
use crate::store::{Snapshot, Store};

pub enum Handle<T, C, H: ByteHash> {
    Val(T),
    Shared(Arc<UnsafeCell<T>>),
    Compound(Box<C>),
    Persisted(Snapshot<C, H>),
    None,
}

impl<T, C, H: ByteHash> Default for Handle<T, C, H> {
    fn default() -> Self {
        Handle::None
    }
}

impl<T: Clone, C: Clone, H: ByteHash> Clone for Handle<T, C, H> {
    fn clone(&self) -> Self {
        match self {
            Handle::Val(ref t) => Handle::Val(t.clone()),
            Handle::Compound(ref c) => Handle::Compound(c.clone()),
            Handle::Persisted(ref snap) => Handle::Persisted(snap.clone()),
            Handle::Shared(ref arc) => Handle::Shared(arc.clone()),
            Handle::None => Handle::None,
        }
    }
}

impl<T: Content<H>, C: Content<H>, H: ByteHash> Content<H> for Handle<T, C, H> {
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

impl<T: Content<H>, C: Content<H>, H: ByteHash> Handle<T, C, H> {
    pub fn val(t: T) -> Handle<T, C, H> {
        Handle::Val(t)
    }

    pub fn pre_persist(&mut self, store: &Store<H>) -> io::Result<()> {
        let hash = match self {
            Handle::Val(ref mut val) => {
                let mut sink = StoreSink::new(store);
                val.persist(&mut sink)?;
                sink.fin()?
            }
            Handle::Shared(arc) => unsafe {
                let mut sink = StoreSink::new(store);
                (*arc.get()).persist(&mut sink)?;
                sink.fin()?
            },
            Handle::Compound(_c) => unimplemented!(),
            Handle::None | Handle::Persisted(_) => return Ok(()),
        };
        *self = Handle::Persisted(Snapshot::new(hash));
        Ok(())
    }

    pub fn get<'a>(&'a self, store: &'a Store<H>) -> io::Result<Cached<'a, T>> {
        match self {
            Handle::Val(ref val) => Ok(Cached::Borrowed(val)),
            Handle::Shared(ref arc) => unsafe {
                Ok(Cached::Borrowed(&*arc.get()))
            },
            Handle::Persisted(snapshot) => store.get(&snapshot),
            Handle::Compound(_) => panic!("Attempt to get Compound value"),
            Handle::None => panic!("Attempt to get None value"),
        }
    }

    pub fn make_shared(&mut self) {
        if let Handle::Val(_) = self {
            if let Handle::Val(t) = mem::replace(self, Handle::None) {
                *self = Handle::Shared(Arc::new(UnsafeCell::new(t)))
            } else {
                unreachable!()
            }
        } else {
            // no-op
        }
    }

    pub fn get_mut(&mut self, store: &Store<H>) -> io::Result<&mut T>
    where
        T: Clone,
    {
        match self {
            Handle::Val(ref mut val) => Ok(val),
            Handle::Shared(arc) => {
                *self = unsafe { Handle::Val((*arc.get()).clone()) };
                self.get_mut(store)
            }
            Handle::Persisted(snap) => {
                *self = Handle::Compound(Box::new(snap.restore(store)?));
                self.get_mut(store)
            }
            Handle::Compound(_) => panic!("Attempt to get_mut Compound value"),
            Handle::None => panic!("Attempt to get_mut None value"),
        }
    }
}
