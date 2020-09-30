// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

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
