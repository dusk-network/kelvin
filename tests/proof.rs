// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use kelvin::{Compound, Void, KV};
use kelvin_hamt::{HAMTSearch, NarrowHAMT};

use canonical_host::MemStore;

#[test]
fn merkle_proof() {
    use kelvin::Proof;

    let mut hamt = NarrowHAMT::<_, _, Void, MemStore, 1024>::new();

    let n = 16;

    // insert n * 64
    for i in 0..n * 64 {
        hamt.insert(i, i).unwrap();
    }

    // make and check proof that 0..n is in the hamt
    for i in 0..n {
        let mut cloned = hamt.clone();

        let mut proof = {
            let mut branch = cloned
                .search_mut(&mut HAMTSearch::from(&i))
                .unwrap()
                .unwrap();
            Proof::new(&mut branch)
        };

        assert_eq!(
            proof.prove_member(&mut cloned).unwrap(),
            Some(&KV { key: i, val: i })
        );

        cloned.insert(3, 8).unwrap();

        assert_eq!(proof.prove_member(&mut cloned).unwrap(), None);
    }
}
