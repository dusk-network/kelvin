use std::io::{self, Write};

use bytehash::{ByteHash, State};

use crate::store::Store;

pub trait Sink<H: ByteHash>
where
    Self: io::Write,
{
    fn recur(&self) -> StoreSink<H>;
}

pub struct StoreSink<'a, H: ByteHash> {
    bytes: Vec<u8>,
    store: &'a Store<H>,
}

impl<'a, H: ByteHash> StoreSink<'a, H> {
    pub fn new(store: &'a Store<H>) -> Self {
        StoreSink {
            bytes: vec![],
            store,
        }
    }

    pub fn fin(self) -> io::Result<H::Digest> {
        let StoreSink { bytes, .. } = self;
        let mut hasher = H::state();
        hasher
            .write_all(&bytes)
            .expect("In memory write should always succeed");
        let hash = hasher.fin();
        self.store.put(hash, bytes)?;
        Ok(hash)
    }
}

impl<'a, H> Sink<H> for StoreSink<'a, H>
where
    H: ByteHash,
{
    fn recur(&self) -> StoreSink<H> {
        Self::new(self.store)
    }
}

impl<'a, H: ByteHash> io::Write for StoreSink<'a, H> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = self.bytes.write(buf);
        res
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
