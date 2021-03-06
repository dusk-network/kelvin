// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io::{self, Read};

use bytehash::ByteHash;

mod mem;

#[cfg(feature = "filesystem")]
mod disk;
#[cfg(feature = "web")]
mod localstorage;

#[cfg(feature = "web")]
pub use self::localstorage::WebBackend as Persistant;

#[cfg(feature = "filesystem")]
pub use disk::DiskBackend as Persistant;

pub use self::mem::MemBackend as Ephemeral;

pub enum PutResult {
    Ok,
    AlreadyThere,
}

/// Trait to implement custom backends
pub trait Backend<H: ByteHash> {
    /// Get a reader from a hash
    fn get<'a>(&'a self, digest: &H::Digest) -> io::Result<Box<dyn Read + 'a>>;

    /// Put the serialized value in the backend.
    fn put(
        &mut self,
        digest: H::Digest,
        bytes: Vec<u8>,
    ) -> io::Result<PutResult>;

    /// Flush changes to underlying medium
    fn flush(&mut self) -> io::Result<()>;

    /// Return approximate size in bytes (optional)
    fn size(&self) -> usize {
        0
    }
}
