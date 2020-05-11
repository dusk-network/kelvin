use std::marker::PhantomData;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fmt, io};

use arrayvec::ArrayVec;
use bytehash::ByteHash;
use cache::Cache;
use parking_lot::RwLock;

use crate::backend::{Backend, Ephemeral, Persistant, PutResult};
use crate::content::Content;
use crate::sink::Sink;
use crate::source::Source;

/// The main store type, wrapping backend and cache functionality
#[derive(Clone)]
pub struct Store<H: ByteHash>(Arc<StoreInner<H>>);

unsafe impl<H: ByteHash> Send for Store<H> {}
unsafe impl<H: ByteHash> Sync for Store<H> {}

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

/// A snapshot of a structure state
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

    /// Returns a reference to the underlying snapshot hash.
    pub fn hash(&self) -> &H::Digest {
        &self.hash
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.hash.as_ref()
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

    /// Creates a new ephemeral (in-memory only) Store
    pub fn ephemeral() -> Self {
        let pers = Ephemeral::new();
        let mut generations = ArrayVec::new();
        generations.push(RwLock::new(Box::new(pers) as Box<dyn Backend<H>>));

        Store(Arc::new(StoreInner {
            generations,
            cache: Cache::new(32, 4096),
        }))
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

    pub(crate) fn flush(&self) -> io::Result<()> {
        // TODO, sync to disk
        for gen in &self.0.generations {
            gen.write().flush()?;
        }

        Ok(())
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
        self.get_hash(&snap.hash)
    }

    pub(crate) fn get_hash<T: Content<H>>(
        &self,
        hash: &H::Digest,
    ) -> io::Result<T> {
        for gen in self.0.generations.as_ref() {
            if let Ok(read) = gen.read().get(hash) {
                let mut source = Source::new(read, self);
                return T::restore(&mut source);
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "Data not found"))
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
