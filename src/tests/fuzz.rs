// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use arbitrary::{Arbitrary, Unstructured};
use canonical::{Canon, Store};
use canonical_host::MemStore;

const FUZZ_ITERATIONS: usize = 1024;

fn hash<T: Hash>(t: T) -> u64 {
    let mut hasher = DefaultHasher::new();
    t.hash(&mut hasher);
    hasher.finish()
}

/// Fuzzes a type with regards to its Content implementation.
/// making sure every serialization produces an Equal result when deserialized
pub fn fuzz_content<C, S>()
where
    C: Canon<MemStore> + Arbitrary + PartialEq + std::fmt::Debug,
    S: Store,
{
    fuzz_content_iterations::<C, S>(FUZZ_ITERATIONS)
}

/// Fuzzes for a set number of iterations
pub fn fuzz_content_iterations<
    C: Canon<MemStore> + Arbitrary + PartialEq + std::fmt::Debug,
    S: Store,
>(
    iterations: usize,
) {
    let store = MemStore::new();
    let mut entropy = 0;
    for _ in 0..iterations {
        let mut bytes = vec![];

        let content = {
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

        let id = store.put(&content).unwrap();
        let restored = store.get(&id).unwrap();

        assert!(content == restored);
    }
}
