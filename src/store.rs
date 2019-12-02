use std::marker::PhantomData;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fmt, io};

use arrayvec::ArrayVec;
use bytehash::ByteHash;
use cache::Cache;
use parking_lot::RwLock;

use crate::backend::{Backend, Persistant, PutResult, Volatile};
use crate::content::Content;
use crate::sink::Sink;
use crate::source::Source;

/// The main store type, wrapping backend and cache functionality
#[derive(Clone)]
pub struct Store<H: ByteHash>(Arc<StoreInner<H>>);

const GENERATIONS: usize = 8;

pub struct StoreInner<H: ByteHash> {
    generations: ArrayVec<[RwLock<Box<dyn Backend<H>>>; GENERATIONS]>,
    #[allow(unused)]
    cache: Cache<H::Digest>,
}

impl<H: ByteHash> fmt::Debug for Store<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Store")
    }
}

#[doc(hidden)]
pub struct Shared<T, H: ByteHash>(T, PhantomData<H>);

unsafe impl<T, H: ByteHash> Send for Shared<T, H> {}

#[derive(Clone, Debug)]
pub struct Snapshot<T, H: ByteHash> {
    hash: H::Digest,
    store: Store<H>,
    _marker: PhantomData<T>,
}

impl<T: Content<H>, H: ByteHash> Snapshot<T, H> {
    pub(crate) fn new(hash: H::Digest, store: &Store<H>) -> Self {
        Snapshot {
            hash,
            store: store.clone(),
            _marker: PhantomData,
        }
    }

    pub(crate) fn restore(&self) -> io::Result<T> {
        self.store.restore(self)
    }
}

impl<N, H: ByteHash> Deref for Snapshot<N, H> {
    type Target = H::Digest;
    fn deref(&self) -> &Self::Target {
        &self.hash
    }
}

impl<H: ByteHash> Store<H> {
    /// Creates a new Store at `path`
    pub fn new<P: Into<PathBuf>>(path: P) -> io::Result<Self> {
        let pers = Persistant::new(path)?;
        let mut generations = ArrayVec::new();
        generations.push(RwLock::new(Box::new(pers) as Box<dyn Backend<H>>));

        Ok(Store(Arc::new(StoreInner {
            generations,
            cache: Cache::new(32, 4096),
        })))
    }

    /// Creates a new volatile (in-memory only) Store
    pub fn volatile() -> io::Result<Self> {
        let pers = Volatile::new();
        let mut generations = ArrayVec::new();
        generations.push(RwLock::new(Box::new(pers) as Box<dyn Backend<H>>));

        Ok(Store(Arc::new(StoreInner {
            generations,
            cache: Cache::new(32, 4096),
        })))
    }

    /// Persists Content to the store, returning a Snapshot
    pub fn persist<T: Content<H>>(
        &self,
        content: &mut T,
    ) -> io::Result<Snapshot<T, H>> {
        let mut sink = Sink::new(self);
        content.persist(&mut sink)?;
        Ok(Snapshot {
            hash: sink.fin()?,
            store: self.clone(),
            _marker: PhantomData,
        })
    }

    pub(crate) fn put(
        &self,
        hash: H::Digest,
        bytes: Vec<u8>,
    ) -> io::Result<PutResult> {
        self.0.generations[0].write().put(hash, bytes)
    }

    /// Restores a snapshot from Backend
    pub fn restore<T: Content<H>>(
        &self,
        snap: &Snapshot<T, H>,
    ) -> io::Result<T> {
        for gen in self.0.generations.as_ref() {
            match gen.read().get(&snap.hash) {
                Ok(read) => {
                    let mut source = Source::new(read, self);
                    return T::restore(&mut source);
                }
                Err(_) => (),
            }
        }
        panic!("could not restore");
    }

    /// Returns the approximate size of the store
    pub fn size(&self) -> usize {
        let mut size = 0;
        for gen in self.0.generations.as_ref() {
            size += gen.read().size();
        }
        size
    }
}
