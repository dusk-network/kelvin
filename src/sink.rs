// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::hash::Hasher;
use std::io;

use bytehash::{ByteHash, State};

use crate::store::Store;

/// A sink for bytes, used in implementing `Content`
pub enum Sink<'a, H: ByteHash> {
    /// Sink is only hashing
    DryRun(H::State),
    /// Sink is writing to storage and hashing
    Writing(Vec<u8>, &'a Store<H>),
    /// Sink is writing to storage with cached hash
    WritingCached(Vec<u8>, H::Digest, &'a Store<H>),
}

impl<'a, H: ByteHash> Sink<'a, H> {
    pub(crate) fn new(store: &'a Store<H>) -> Self {
        Sink::Writing(vec![], store)
    }

    pub(crate) fn new_dry() -> Self {
        Sink::DryRun(H::state())
    }

    pub(crate) fn new_cached(hash: H::Digest, store: &'a Store<H>) -> Self {
        Sink::WritingCached(vec![], hash, store)
    }

    pub(crate) fn store(&self) -> Option<&Store<H>> {
        match self {
            Sink::Writing(_, ref store)
            | Sink::WritingCached(_, _, ref store) => Some(store),
            Sink::DryRun(_) => None,
        }
    }

    pub(crate) fn fin(self) -> io::Result<H::Digest> {
        match self {
            Sink::DryRun(state) => Ok(state.fin()),
            Sink::Writing(bytes, store) => {
                let mut hasher = H::state();
                hasher.write(&bytes);
                let hash = hasher.fin();
                store.put(hash, bytes)?;
                Ok(hash)
            }
            Sink::WritingCached(bytes, hash, store) => {
                store.put(hash, bytes)?;
                Ok(hash)
            }
        }
    }
}

impl<'a, H: ByteHash> io::Write for Sink<'a, H> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Sink::DryRun(state) => {
                // Note, this write is from Hasher, not from io;
                state.write(buf);
                Ok(buf.len())
            }
            Sink::Writing(bytes, ..) | Sink::WritingCached(bytes, ..) => {
                bytes.write(buf)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
