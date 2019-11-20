use std::collections::HashMap;
use std::io::{self, Read};

use bytehash::ByteHash;
use lazy_static::lazy_static;
use owning_ref::OwningRef;
use parking_lot::{RwLock, RwLockReadGuard};

use crate::backend::{Backend, PutResult};

// Used to return a `&Vec` when get fails
lazy_static! {
    static ref EMPTY: Vec<u8> = { vec![] };
}

type ByteMap<D> = HashMap<D, Vec<u8>>;

struct MemBackendInner<H: ByteHash> {
    size: usize,
    data: ByteMap<H::Digest>,
}

/// A backend that stores its data in memory
pub struct MemBackend<H: ByteHash>(RwLock<MemBackendInner<H>>);
unsafe impl<H: ByteHash> Sync for MemBackend<H> {}

impl<H: ByteHash> MemBackend<H> {
    /// Creates a new `MemBackend`
    pub fn new() -> Self {
        MemBackend(RwLock::new(MemBackendInner {
            size: 0,
            data: HashMap::new(),
        }))
    }
}

struct MemSource<'a, H: ByteHash> {
    guard: OwningRef<RwLockReadGuard<'a, MemBackendInner<H>>, Vec<u8>>,
    offset: usize,
}

impl<'a, H: ByteHash> Read for MemSource<'a, H> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.guard[self.offset..]).read(buf).map(|ofs| {
            self.offset += ofs;
            ofs
        })
    }
}

impl<'a, H: ByteHash> MemSource<'a, H> {
    fn from_lock(
        guard: &'a RwLock<MemBackendInner<H>>,
        hash: &H::Digest,
    ) -> Self {
        let readguard = OwningRef::new(guard.read())
            .map(|ref map| map.data.get(hash).unwrap_or(&*EMPTY));
        MemSource {
            guard: readguard,
            offset: 0,
        }
    }
}

impl<H: ByteHash> Backend<H> for MemBackend<H> {
    fn get<'a>(&'a self, hash: &H::Digest) -> io::Result<Box<dyn Read + 'a>> {
        Ok(Box::new(MemSource::<H>::from_lock(&self.0, hash)))
    }

    fn put(&self, hash: H::Digest, bytes: Vec<u8>) -> io::Result<PutResult> {
        let mut inner = self.0.write();
        inner.size += bytes.len();
        match inner.data.insert(hash, bytes) {
            Some(_) => Ok(PutResult::AlreadyThere),
            None => Ok(PutResult::Ok),
        }
    }

    fn size(&self) -> usize {
        self.0.read().size
    }
}
