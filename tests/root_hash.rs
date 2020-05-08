// Example on how to use extra hash annotations to do merkle proofs
use std::borrow::Borrow;

use kelvin::{
    Blake2b, Compound, Content, ErasedAnnotation, Store, VoidAnnotation,
};
use kelvin_hamt::{HAMTSearch, HAMT};
use std::collections::hash_map::DefaultHasher;

#[test]
fn root_hash() {
    let mut hamt = HAMT::<_, _, VoidAnnotation, Blake2b>::new();

    for i in 0..1024 {
        hamt.insert(i, i).unwrap();
    }

    // Calculating the root hash should not write anything to any store

    let root_hash = hamt.root_hash();

    let store = Store::ephemeral();

    let snap = store.persist(&mut hamt).unwrap();

    assert_eq!(&root_hash, snap.hash());

    let mut hamt_restored = store.restore(&snap).unwrap();

    let restored_root_hash = hamt_restored.root_hash();

    assert_eq!(root_hash, restored_root_hash);
}
