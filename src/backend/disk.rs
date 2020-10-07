// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::fs::{create_dir, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use appendix::Index;
use arrayvec::ArrayVec;
use canonical::Store;

use crate::backend::{Backend, PutResult};

struct DiskId([u8; 32]);

/// A backend that stores its data in an `appendix` index, and a flat file
pub struct DiskBackend {
    index: Index<DiskId, u64>,
    data: File,
    data_path: PathBuf,
    data_offset: u64,
}

impl<S: Store> DiskBackend<S> {
    /// Create a new DiskBackend at given path, creates a new directory if neccesary
    pub fn new<P: Into<PathBuf>>(path: P) -> io::Result<Self> {
        let dir = path.into();
        if !dir.exists() {
            create_dir(&dir)?;
        }

        let index_dir = dir.join("index");
        if !index_dir.exists() {
            create_dir(&index_dir)?;
        }

        let index = Index::new(&index_dir)?;
        let data_path = dir.join("data");

        let mut data = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&data_path)?;

        let data_offset = data.metadata()?.len();
        data.seek(SeekFrom::End(0))?;

        Ok(DiskBackend {
            index,
            data_path,
            data,
            data_offset,
        })
    }
}

impl<S: Store> Backend<S> for DiskBackend<S> {
    fn get<'a>(&'a self, hash: &S::Ident) -> io::Result<Box<dyn Read + 'a>> {
        match self.index.get(hash)? {
            Some(offset) => {
                let mut file = File::open(&self.data_path)?;
                file.seek(SeekFrom::Start(*offset))?;
                Ok(Box::new(file))
            }
            None => {
                Err(io::Error::new(io::ErrorKind::NotFound, "Data not found"))
            }
        }
    }

    fn put(
        &mut self,
        hash: S::Ident,
        bytes: ArrayVec,
    ) -> io::Result<PutResult> {
        if self.index.insert(hash, self.data_offset)? {
            // value already present
            Ok(PutResult::AlreadyThere)
        } else {
            self.data.write_all(&bytes)?;
            self.data_offset += bytes.len() as u64;
            Ok(PutResult::Ok)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.data.flush()?;
        self.index.flush()
    }

    fn size(&self) -> usize {
        self.index.on_disk_size() + self.data_offset as usize
    }
}
