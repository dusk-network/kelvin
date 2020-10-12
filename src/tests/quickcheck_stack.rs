// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

/// Model test suite for stacks
///
/// Usage example: `quickcheck_stack!(|| HAMT::new());`
#[macro_export]
macro_rules! quickcheck_stack {
    ($new_stack:expr) => {
        use canonical_host::MemStore as __MemStore;

        use $crate::tests::tempfile::tempdir;

        #[allow(unused)]
        use $crate::tests::CorrectEmptyState as _;

        use $crate::tests::quickcheck::{quickcheck, Arbitrary, Gen};
        #[allow(unused)]
        use $crate::{annotations::Count, LeafIterable, ValIterable};

        use $crate::tests::rand::Rng;

        const KEY_SPACE: usize = 64;

        #[derive(Clone, Debug)]
        pub enum Op {
            Push(u8),
            Get(usize),
            GetMut(usize),
            Pop,
            RemoveAll,
            Iter,
            IterMut,
            Persist,
            Hash,
            HashPersist,
            PersistRestore,
            Count,
        }

        impl Arbitrary for Op {
            fn arbitrary<G: Gen>(g: &mut G) -> Op {
                let k = g.gen_range(0, KEY_SPACE);
                let op = g.gen_range(0, 12);
                match op {
                    0 => Op::Push(g.gen()),
                    1 => Op::Get(k),
                    2 => Op::GetMut(k),
                    3 => Op::Pop,
                    4 => Op::RemoveAll,
                    5 => Op::Iter,
                    6 => Op::IterMut,
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
            let store = __MemStore::new();

            let mut test = $new_stack();
            let mut model = Vec::new();

            for op in ops {
                match op {
                    Op::Push(v) => {
                        let a = test.push(v);
                        let b = model.push(v);
                    }
                    Op::Get(k) => {
                        let a = test.get(k as u64).unwrap();
                        let b = model.get(k);

                        match (a, b) {
                            (Some(a), Some(b)) => {
                                assert!(*a == *b);
                            }
                            (None, None) => (),
                            (Some(_), None) => {
                                panic!("test has item, model not")
                            }
                            (None, Some(_)) => {
                                panic!("model has item, test not")
                            }
                        };
                    }
                    Op::GetMut(k) => {
                        let a = test
                            .get_mut(k as u64)
                            .unwrap()
                            .map(|mut val| *val = val.wrapping_add(1));
                        let b = model
                            .get_mut(k)
                            .map(|val| *val = val.wrapping_add(1));

                        assert!(a == b)
                    }
                    Op::Pop => {
                        let a = test.pop().unwrap();
                        let c = model.pop();

                        assert!(a == c);
                    }
                    Op::RemoveAll => {
                        while let Some(_) = test.pop().unwrap() {}
                        model.clear();
                        test.assert_correct_empty_state();
                    }
                    Op::Iter => {
                        let mut a: Vec<u8> =
                            test.iter().map(|v| *v.unwrap()).collect();

                        let mut c: Vec<u8> = model.iter().map(|v| *v).collect();

                        assert!(a == c);
                    }
                    Op::IterMut => {
                        let _res = test
                            .iter_mut()
                            .map(|v: Result<&mut u8, _>| {
                                let v = v.unwrap();
                                *v = v.wrapping_add(1);
                                v
                            })
                            .collect::<Vec<_>>();

                        let _res = model
                            .iter_mut()
                            .map(|v| *v = v.wrapping_add(1))
                            .collect::<Vec<_>>();

                        let mut a: Vec<u8> =
                            test.iter().map(|v| *v.unwrap()).collect();

                        let mut c: Vec<_> = model.iter().map(|v| *v).collect();

                        assert!(a == c);
                    }
                    Op::Persist => {
                        store.put(&test).unwrap();
                    }
                    Op::Hash => {
                        let _ = test.root_hash();
                    }
                    Op::HashPersist => {
                        let root_hash = S::ident(&test);
                        let snapshot = store.put(&test).unwrap();
                        assert_eq!(&root_hash, snapshot.hash())
                    }
                    Op::PersistRestore => {
                        let snapshot = store.put(&test).unwrap();
                        test = store.get(&snapshot).unwrap();
                    }
                    Op::Count => assert_eq!(test.count() as usize, model.len()),
                };
            }
            true
        }

        quickcheck! {
            fn stack(ops: Vec<Op>) -> bool {
                run_ops(ops)
            }
        }
    };
}
