// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{ByteHash, Content, Store};
use arbitrary::{Arbitrary, Unstructured};

const FUZZ_ITERATIONS: usize = 1024;

fn hash<T: Hash>(t: T) -> u64 {
    let mut hasher = DefaultHasher::new();
    t.hash(&mut hasher);
    hasher.finish()
}

/// Fuzzes a type with regards to its Content implementation.
/// making sure every serialization produces an Equal result when deserialized
pub fn fuzz_content<C: Content<H> + Arbitrary + PartialEq, H: ByteHash>() {
    fuzz_content_iterations::<C, H>(FUZZ_ITERATIONS)
}

/// Fuzzes for a set number of iterations
pub fn fuzz_content_iterations<
    C: Content<H> + Arbitrary + PartialEq,
    H: ByteHash,
>(
    iterations: usize,
) {
    let store = Store::ephemeral();
    let mut entropy = 0;
    for _ in 0..iterations {
        let mut bytes = vec![];

        let mut content = {
            loop {
                match C::arbitrary(&mut Unstructured::new(&bytes)) {
                    Ok(t) => break t,
                    Err(_) => {
                        entropy += 1;
                        bytes.extend_from_slice(&hash(entropy).to_be_bytes());
                    }
                }
            }
        };

        let snap = store.persist(&mut content).unwrap();
        let restored = snap.restore().unwrap();

        assert!(content == restored);
    }
}
