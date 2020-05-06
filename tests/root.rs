use kelvin::{Blake2b, Root, Store};
use std::path::PathBuf;
use tempfile::tempdir;

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
