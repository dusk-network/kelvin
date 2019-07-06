use std::cell::UnsafeCell;
use std::fs::{create_dir, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use appendix::Index;
use bytehash::ByteHash;
use parking_lot::Mutex;

use crate::backend::{Backend, PutResult};

pub struct DiskBackend<H: ByteHash> {
    index: Index<H::Digest, u64>,
    data: UnsafeCell<File>,
    data_path: PathBuf,
    data_offset: Mutex<u64>,
}

unsafe impl<H: ByteHash> Send for DiskBackend<H> {}
unsafe impl<H: ByteHash> Sync for DiskBackend<H> {}

impl<H: ByteHash> DiskBackend<H> {
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
            data: UnsafeCell::new(data),
            data_offset: Mutex::new(data_offset),
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
            None => unimplemented!(),
        }
    }

    fn put(&self, hash: H::Digest, bytes: Vec<u8>) -> io::Result<PutResult> {
        let mut offset_lock = self.data_offset.lock();
        let offset = *offset_lock;

        if self.index.insert(hash, offset)? {
            // value already present
            Ok(PutResult::AlreadyThere)
        } else {
            unsafe {
                (*self.data.get()).write(&bytes)?;
            }
            *offset_lock += bytes.len() as u64;
            Ok(PutResult::Ok)
        }
    }

    fn size(&self) -> usize {
        self.index.on_disk_size() + *self.data_offset.lock() as usize
    }
}
