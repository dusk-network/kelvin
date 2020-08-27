// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use kelvin::annotations::{Cardinality, GetNth};
use kelvin::{Blake2b, LeafIterable};
use kelvin_hamt::HAMT;

#[test]
fn nth_vs_iter() {
    let mut hamt = HAMT::<_, _, Cardinality<u64>, Blake2b>::new();

    let n: u64 = 1024;

    // insert n * 64
    for i in 0..n {
        hamt.insert(i, i).unwrap();
    }

    let mut leaves_by_iter = vec![];
    let mut leaves_by_nth = vec![];

    for element in hamt.iter() {
        leaves_by_iter.push(element.unwrap().clone());
    }

    for i in 0..n {
        leaves_by_nth.push((*hamt.nth(i).unwrap().unwrap()).clone())
    }

    assert_eq!(leaves_by_iter, leaves_by_nth);
}
