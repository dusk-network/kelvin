// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use kelvin::{Blake2b, Store};
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn should_create_directory() {
    let dir = tempdir().unwrap();

    let mut sub_dir: PathBuf = dir.path().into();
    sub_dir.push("sub_directory");

    let _store = Store::<Blake2b>::new(&sub_dir).unwrap();

    assert!(sub_dir.exists());
}

#[test]
fn should_allow_two() {
    let dir = tempdir().unwrap();

    {
        let _store = Store::<Blake2b>::new(dir.path()).unwrap();
    }
    let _store = Store::<Blake2b>::new(dir.path()).unwrap();
}
