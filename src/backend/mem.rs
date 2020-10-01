// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::HashMap;
use std::io::{self, Cursor, Read};

use bytehash::ByteHash;

use crate::backend::{Backend, PutResult};

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
        if let Some(data) = self.data.get(hash) {
            Ok(Box::new(Cursor::new(data)))
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "Data not found"))
        }
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

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> usize {
        self.size
    }
}
