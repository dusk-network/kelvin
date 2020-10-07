// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use kelvin::{Compound, Void};
use kelvin_hamt::HAMT;

use canonical_host::{Canon, MemStore, Store};

#[test]
fn root_hash() {
    type Hamt = HAMT<u32, u32, Void, MemStore, 1024>;

    let mut hamt = Hamt::new();

    for i in 0..1024 {
        hamt.insert(i, i).unwrap();
    }

    // Calculating the root hash should not write anything to any store

    let root_hash: <MemStore as Store>::Ident = hamt.root_hash();

    let store = MemStore::new();

    let id: <MemStore as Store>::Ident = store.put(&mut hamt).unwrap();

    assert_eq!(root_hash, id);

    let mut hamt_restored = store.get::<Hamt>(&id).unwrap();

    let restored_root_hash = hamt_restored.root_hash();

    assert_eq!(root_hash, restored_root_hash);
}
