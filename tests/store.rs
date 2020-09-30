// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

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
