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
        use $crate::tests::CorrectEmptyState;

        use $crate::tests::quickcheck::{quickcheck, Arbitrary, Gen};
        #[allow(unused)]
        use $crate::{annotations::Count, KeyValIterable, LeafIterable, Store};

        use $crate::tests::rand::Rng;

        const KEY_SPACE: u8 = 20;

        #[derive(Clone, Debug)]
        pub enum Op {
            Insert(u8, u8),
            Get(u8),
            GetMut(u8),
            Remove(u8),
            RemoveAll,
            Iter,
            IterMut,
            Values,
            ValuesMut,
            Keys,
            Persist,
            PersistRestore,
            PersistRestoreRoot,
            Count,
        }

        impl Arbitrary for Op {
            fn arbitrary<G: Gen>(g: &mut G) -> Op {
                let k: u8 = g.gen_range(0, KEY_SPACE);
                let op = g.gen_range(0, 14);
                match op {
                    0 => Op::Insert(k, g.gen()),
                    1 => Op::Iter,
                    2 => Op::IterMut,
                    3 => Op::Get(k),
                    4 => Op::GetMut(k),
                    5 => Op::Remove(k),
                    6 => Op::RemoveAll,
                    7 => Op::Values,
                    8 => Op::ValuesMut,
                    9 => Op::Keys,
                    10 => Op::Persist,
                    11 => Op::PersistRestore,
                    12 => Op::PersistRestoreRoot,
                    13 => Op::Count,
                    _ => unreachable!(),
                }
            }
        }

        fn run_ops(ops: Vec<Op>) -> bool {
            let dir = tempdir().unwrap();
            let store = Store::<Blake2b>::new(&dir.path()).unwrap();

            let mut test_a = $new_map();
            let mut model = HashMap::new();

            for op in ops {
                match op {
                    Op::Insert(k, v) => {
                        let a = test_a.insert(k, v).unwrap();
                        let b = model.insert(k, v);
                        assert_eq!(a, b);
                    }

                    Op::Iter => {
                        let mut a: Vec<_> = test_a
                            .iter()
                            .map(|res| res.unwrap())
                            .cloned()
                            .collect();
                        let mut b: Vec<_> = model
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();

                        a.sort();
                        b.sort();

                        assert_eq!(a, b);
                    }

                    Op::IterMut => {
                        for (_, value) in test_a.iter_mut().map(|r| r.unwrap())
                        {
                            *value = value.wrapping_add(1)
                        }

                        for (_, value) in model.iter_mut() {
                            *value = value.wrapping_add(1)
                        }
                    }

                    Op::Get(k) => {
                        let a = test_a.get(&k).unwrap();
                        let b = model.get(&k);

                        dbg!(a.is_some(), b.is_some());

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
                        let a = test_a
                            .get_mut(&k)
                            .unwrap()
                            .map(|mut val| *val = val.wrapping_add(1));
                        let b = model
                            .get_mut(&k)
                            .map(|val| *val = val.wrapping_add(1));

                        assert!(a == b)
                    }

                    Op::Remove(k) => {
                        let a = test_a.remove(&k).unwrap();
                        let c = model.remove(&k);

                        assert!(a == c);
                    }

                    Op::RemoveAll => {
                        model.clear();
                        let mut keys = vec![];
                        for (key, _) in test_a.iter().map(|res| res.unwrap()) {
                            keys.push(key.clone());
                        }
                        for key in keys {
                            test_a.remove(&key).unwrap();
                        }
                        test_a.assert_correct_empty_state();
                    }

                    Op::Values => {
                        let mut a: Vec<_> =
                            test_a.values().map(|v| *v.unwrap()).collect();

                        let mut c: Vec<_> =
                            model.values().map(|v| *v).collect();

                        a.sort();
                        c.sort();

                        assert!(a == c);
                    }

                    Op::ValuesMut => {
                        let _res = test_a
                            .values_mut()
                            .map(|v| {
                                let v = v.unwrap();
                                *v = v.wrapping_add(1);
                                ()
                            })
                            .collect::<Vec<_>>();

                        let _res = model
                            .values_mut()
                            .map(|v| *v = v.wrapping_add(1))
                            .collect::<Vec<_>>();

                        let mut a: Vec<_> =
                            test_a.values().map(|v| *v.unwrap()).collect();

                        let mut c: Vec<_> =
                            model.values().map(|v| *v).collect();

                        a.sort();
                        c.sort();

                        assert!(a == c);
                    }
                    Op::Keys => {
                        let mut a: Vec<_> =
                            test_a.keys().map(|v| *v.unwrap()).collect();

                        let mut c: Vec<_> = model.keys().map(|k| *k).collect();

                        a.sort();
                        c.sort();

                        assert!(a == c);
                    }
                    Op::Persist => {
                        store.persist(&mut test_a).unwrap();
                    }
                    Op::PersistRestore => {
                        let snapshot = store.persist(&mut test_a).unwrap();
                        test_a = store.restore(&snapshot).unwrap();
                    }
                    Op::PersistRestoreRoot => {
                        //
                    }
                    Op::Count => {
                        assert_eq!(test_a.count() as usize, model.len())
                    }
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
    };
}
