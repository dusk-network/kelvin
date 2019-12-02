use std::fs::{create_dir, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use appendix::Index;
use bytehash::ByteHash;

use crate::backend::{Backend, PutResult};

/// A backend that stores its data in an `appendix` index, and a flat file
pub struct DiskBackend<H: ByteHash> {
    index: Index<H::Digest, u64>,
    data: File,
    data_path: PathBuf,
    data_offset: u64,
}

impl<H: ByteHash> DiskBackend<H> {
    /// Create a new DiskBackend at given path, creates a new directory if neccesary
    pub fn new<P: Into<PathBuf>>(path: P) -> io::Result<Self> {
        let dir = path.into();
        let index_dir = dir.join("index");
        create_dir(&index_dir)?;
        let index = Index::new(&index_dir)?;
        let data_path = dir.join("data");

        let data = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&data_path)?;

        let data_offset = data.metadata()?.len();

        Ok(DiskBackend {
            index,
            data_path,
            data,
            data_offset,
        })
    }
}

impl<H: ByteHash> Backend<H> for DiskBackend<H> {
    fn get<'a>(&'a self, hash: &H::Digest) -> io::Result<Box<dyn Read + 'a>> {
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
        hash: H::Digest,
        bytes: Vec<u8>,
    ) -> io::Result<PutResult> {
        if self.index.insert(hash, self.data_offset)? {
            // value already present
            Ok(PutResult::AlreadyThere)
        } else {
            self.data.write(&bytes)?;
            self.data_offset += bytes.len() as u64;
            Ok(PutResult::Ok)
        }
    }

    fn size(&self) -> usize {
        self.index.on_disk_size() + self.data_offset as usize
    }
}
