// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::io::{self, Read};

use bytehash::ByteHash;

use crate::store::Store;

/// A source of bytes, used in implementing `Content`
pub struct Source<'a, H: ByteHash> {
    read: Box<dyn Read + 'a>,
    store: &'a Store<H>,
}

impl<'a, H: ByteHash> Source<'a, H> {
    pub(crate) fn new(read: Box<dyn Read + 'a>, store: &'a Store<H>) -> Self {
        Source { read, store }
    }

    pub(crate) fn store(&self) -> &Store<H> {
        &self.store
    }
}

impl<'a, H: ByteHash> Read for Source<'a, H> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read.read(buf)
    }
}
