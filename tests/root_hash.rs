// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use kelvin::Void;
use kelvin_hamt::HAMT;

use canonical::Store;
use canonical_host::MemStore;

#[test]
fn root_hash() {
    type Hamt = HAMT<u32, u32, Void, MemStore>;

    let mut hamt = Hamt::new();

    for i in 0..1024 {
        hamt.insert(i, i).unwrap();
    }

    // Calculating the root hash should not write anything to any store

    let root_hash = MemStore::ident(&hamt);

    let store = MemStore::new();

    let id: <MemStore as Store>::Ident = store.put(&mut hamt).unwrap();

    assert_eq!(root_hash, id);

    let hamt_restored = store.get::<Hamt>(&id).unwrap();

    let restored_root_hash = MemStore::ident(&hamt_restored);

    assert_eq!(root_hash, restored_root_hash);
}
