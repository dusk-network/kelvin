// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use kelvin::{Blake2b, Compound, Void, KV};
use kelvin_hamt::{HAMTSearch, NarrowHAMT};

#[test]
fn merkle_proof() {
    use kelvin::Proof;

    let mut hamt = NarrowHAMT::<_, _, Void, Blake2b>::new();

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
            proof.prove_member(&mut cloned),
            Some(&KV { key: i, val: i })
        );

        cloned.insert(3, 8).unwrap();

        assert_eq!(proof.prove_member(&mut cloned), None);
    }
}
