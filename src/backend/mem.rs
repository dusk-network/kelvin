use std::collections::HashMap;
use std::io::{self, Cursor, Read};

use bytehash::ByteHash;
use lazy_static::lazy_static;

use crate::backend::{Backend, PutResult};

// Used to return a `&Vec` when get fails
lazy_static! {
    static ref EMPTY: Vec<u8> = { vec![] };
}

type ByteMap<D> = HashMap<D, Vec<u8>>;

/// A backend that stores its data in memory
pub struct MemBackend<H: ByteHash> {
    size: usize,
    data: ByteMap<H::Digest>,
}

impl<H: ByteHash> MemBackend<H> {
    /// Creates a new `MemBackend`
    pub fn new() -> Self {
        MemBackend {
            size: 0,
            data: HashMap::new(),
        }
    }
}

impl<H: ByteHash> Backend<H> for MemBackend<H> {
    fn get<'a>(&'a self, hash: &H::Digest) -> io::Result<Box<dyn Read + 'a>> {
        Ok(Box::new(Cursor::new(
            self.data.get(hash).unwrap_or(&*EMPTY),
        )))
    }

    fn put(
        &mut self,
        hash: H::Digest,
        bytes: Vec<u8>,
    ) -> io::Result<PutResult> {
        self.size += bytes.len();
        match self.data.insert(hash, bytes) {
            Some(_) => Ok(PutResult::AlreadyThere),
            None => Ok(PutResult::Ok),
        }
    }

    fn size(&self) -> usize {
        self.size
    }
}
