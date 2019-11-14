use std::io::{self, Write};

use bytehash::{ByteHash, State};

use crate::store::Store;

pub trait SinkTrait<H: ByteHash>
where
    Self: io::Write,
{
    fn recur(&self) -> Sink<H>;
}

pub struct Sink<'a, H: ByteHash> {
    bytes: Vec<u8>,
    store: &'a Store<H>,
}

impl<'a, H: ByteHash> Sink<'a, H> {
    pub fn new(store: &'a Store<H>) -> Self {
        Sink {
            bytes: vec![],
            store,
        }
    }

    pub fn fin(self) -> io::Result<H::Digest> {
        let Sink { bytes, .. } = self;
        let mut hasher = H::state();
        hasher
            .write_all(&bytes)
            .expect("In memory write should always succeed");
        let hash = hasher.fin();
        self.store.put(hash, bytes)?;
        Ok(hash)
    }
}

impl<'a, H> SinkTrait<H> for Sink<'a, H>
where
    H: ByteHash,
{
    fn recur(&self) -> Sink<H> {
        Self::new(self.store)
    }
}

impl<'a, H: ByteHash> io::Write for Sink<'a, H> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = self.bytes.write(buf);
        res
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
