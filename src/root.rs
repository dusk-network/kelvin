use std::fs::File;
use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::path::PathBuf;

use atomicwrites::{AllowOverwrite, AtomicFile};

use crate::{content::Content, ByteHash, Snapshot, Store};

/// Type to keep track of the root of a state tree.
///
/// The latest snapshot is saved between program runs.
pub struct Root<T: Content<H>, H: ByteHash> {
    path: PathBuf,
    store: Store<H>,
    _marker: PhantomData<T>,
}

impl<T, H> Root<T, H>
where
    T: Content<H> + Default,
    H: ByteHash,
{
    /// Given a path, create a new `Root`
    pub fn new<P: Into<PathBuf>>(path: P) -> io::Result<Self> {
        let path = path.into();
        let store = Store::new(&path)?;

        Ok(Root {
            path,
            store,
            _marker: PhantomData,
        })
    }

    /// Restore the latest state of the Root.
    pub fn restore(&self) -> io::Result<T> {
        let root_file_path = self.path.join("root");
        if root_file_path.exists() {
            let mut file = File::open(root_file_path)?;
            let mut hash = H::Digest::default();
            file.read_exact(hash.as_mut())?;
            self.store.get_hash(&hash)
        } else {
            Ok(T::default())
        }
    }

    /// Set the latest state of the Root. Anything not reachable from this node
    /// will be lost, and eventually garbage collected.
    pub fn set_root(&mut self, t: &mut T) -> io::Result<Snapshot<T, H>> {
        let snapshot = self.store.persist(t)?;
        self.store.flush()?;
        let root_file_path = self.path.join("root");
        let af = AtomicFile::new(root_file_path, AllowOverwrite);
        af.write(|f| f.write_all(snapshot.as_bytes()))?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tests::tempfile::tempdir;
    use crate::Blake2b;

    #[test]
    fn root_integer() {
        let dir = tempdir().unwrap();

        // Default
        {
            let ver = Root::<u8, Blake2b>::new(dir.path()).unwrap();
            let def = ver.restore().unwrap();

            assert_eq!(def, 0u8);
        }

        // Set new root state
        {
            let mut ver = Root::<u8, Blake2b>::new(dir.path()).unwrap();
            ver.set_root(&mut 42).unwrap();
        }

        // Restore state
        {
            let ver = Root::<u8, Blake2b>::new(dir.path()).unwrap();
            let restored = ver.restore().unwrap();

            assert_eq!(restored, 42);
        }
    }
}
