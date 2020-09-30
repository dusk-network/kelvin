// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Model test suite for maps
///
/// Usage example: `quickcheck_map!(|| HAMT::new());`
#[macro_export]
macro_rules! quickcheck_map {
    ($new_map:expr) => {
        // mod inner_mod {
        use $crate::tests::tempfile::tempdir;

        #[allow(unused)]
        use std::collections::HashMap;
        use $crate::tests::CorrectEmptyState as _;

        use $crate::tests::quickcheck::{quickcheck, Arbitrary, Gen};
        #[allow(unused)]
        use $crate::{annotations::Count, LeafIterable, Store, ValIterable};

        use $crate::tests::rand::Rng;

        const KEY_SPACE: u8 = 20;

        #[derive(Clone, Debug)]
        pub enum Op {
            Insert(u8, u8),
            Get(u8),
            GetMut(u8),
            Remove(u8),
            RemoveAll,
            Values,
            ValuesMut,
            Persist,
            Hash,
            HashPersist,
            PersistRestore,
            Count,
        }

        impl Arbitrary for Op {
            fn arbitrary<G: Gen>(g: &mut G) -> Op {
                let k: u8 = g.gen_range(0, KEY_SPACE);
                let op = g.gen_range(0, 12);
                match op {
                    0 => Op::Insert(k, g.gen()),
                    1 => Op::Get(k),
                    2 => Op::GetMut(k),
                    3 => Op::Remove(k),
                    4 => Op::RemoveAll,
                    5 => Op::Values,
                    6 => Op::ValuesMut,
                    7 => Op::Persist,
                    8 => Op::Hash,
                    9 => Op::HashPersist,
                    10 => Op::PersistRestore,
                    11 => Op::Count,
                    _ => unreachable!(),
                }
            }
        }

        fn run_ops(ops: Vec<Op>) -> bool {
            let store = Store::<Blake2b>::ephemeral();

            let mut test = $new_map();
            let mut model = HashMap::new();

            for op in ops {
                match op {
                    Op::Insert(k, v) => {
                        let a = test.insert([k], v).unwrap();
                        let b = model.insert([k], v);
                        assert_eq!(a, b);
                    }
                    Op::Get(k) => {
                        let a = test.get(&[k]).unwrap();
                        let b = model.get(&[k]);

                        match (a, b) {
                            (Some(a), Some(b)) => {
                                assert!(*a == *b);
                            }
                            (None, None) => (),
                            (Some(_), None) => panic!("test has kv, model not"),
                            (None, Some(_)) => panic!("model has kv, test not"),
                        };
                    }
                    Op::GetMut(k) => {
                        let a = test
                            .get_mut(&[k])
                            .unwrap()
                            .map(|mut val| *val = val.wrapping_add(1));
                        let b = model
                            .get_mut(&[k])
                            .map(|val| *val = val.wrapping_add(1));

                        assert!(a == b)
                    }
                    Op::Remove(k) => {
                        let a = test.remove(&[k]).unwrap();
                        let c = model.remove(&[k]);

                        assert!(a == c);
                    }
                    Op::RemoveAll => {
                        model.clear();
                        for k in 0..KEY_SPACE {
                            test.remove(&[k]).unwrap();
                        }
                        test.assert_correct_empty_state();
                    }
                    Op::Values => {
                        let mut a: Vec<u8> =
                            test.values().map(|v| *v.unwrap()).collect();

                        let mut c: Vec<u8> =
                            model.values().map(|v| *v).collect();

                        a.sort();
                        c.sort();

                        assert!(a == c);
                    }
                    Op::ValuesMut => {
                        let _res = test
                            .values_mut()
                            .map(|v: Result<&mut u8, _>| {
                                let v = v.unwrap();
                                *v = v.wrapping_add(1);
                                v
                            })
                            .collect::<Vec<_>>();

                        let _res = model
                            .values_mut()
                            .map(|v| *v = v.wrapping_add(1))
                            .collect::<Vec<_>>();

                        let mut a: Vec<u8> =
                            test.values().map(|v| *v.unwrap()).collect();

                        let mut c: Vec<_> =
                            model.values().map(|v| *v).collect();

                        a.sort();
                        c.sort();

                        assert!(a == c);
                    }
                    Op::Persist => {
                        store.persist(&mut test).unwrap();
                    }
                    Op::Hash => {
                        let _ = test.root_hash();
                    }
                    Op::HashPersist => {
                        let root_hash = test.root_hash();
                        let snapshot = store.persist(&mut test).unwrap();
                        assert_eq!(&root_hash, snapshot.hash(),)
                    }
                    Op::PersistRestore => {
                        let snapshot = store.persist(&mut test).unwrap();
                        test = store.restore(&snapshot).unwrap();
                    }
                    Op::Count => assert_eq!(test.count() as usize, model.len()),
                };
            }
            true
        }

        quickcheck! {
            fn map(ops: Vec<Op>) -> bool {
                run_ops(ops)
            }
        }

        use Op::*;

        // regressions
        #[test]
        fn regression_pre_persist_fail() {
            assert!(run_ops(vec![Insert(6, 241), Insert(9, 147), Persist]))
        }

        #[test]
        fn regression_invalid_count_insert() {
            assert!(run_ops(vec![
                Insert(19, 240),
                Insert(1, 84),
                Insert(7, 203),
                Count
            ]))
        }

        #[test]
        fn regression_invalid_count_remove() {
            assert!(run_ops(vec![
                Insert(19, 45),
                Insert(7, 126),
                Insert(1, 198),
                Remove(7),
                Count
            ]))
        }

        #[test]
        fn regression_remove_all() {
            assert!(run_ops(vec![Insert(19, 45), Insert(7, 126), RemoveAll,]))
        }

        #[test]
        fn regression_get() {
            assert!(run_ops(vec![
                Insert(16, 114),
                Insert(6, 225),
                Insert(17, 19),
                Insert(9, 243),
                Get(9)
            ]))
        }

        #[test]
        fn regression_merge() {
            assert!(run_ops(vec![
                Insert(19, 158),
                Insert(18, 154),
                Insert(2, 97),
                Insert(13, 9),
                Remove(19)
            ]))
        }

        #[test]
        fn regression_removeall() {
            assert!(run_ops(vec![
                Insert(12, 142),
                Insert(4, 252),
                Insert(6, 47),
                Insert(0, 15),
                Insert(15, 122),
                RemoveAll
            ]))
        }

        #[test]
        fn regression_ordering() {
            assert!(run_ops(vec![
                Insert(13, 113),
                Insert(2, 209),
                Insert(9, 151),
                Insert(11, 56),
                Get(11)
            ]))
        }

        #[test]
        fn regression_insert_two_get() {
            assert!(run_ops(vec![Insert(5, 23), Insert(9, 52), Get(4)]))
        }
    };
}
