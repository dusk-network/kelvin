// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use canonical::Canon;
use canonical_derive::Canon;

use kelvin::{Blake2b, ByteHash, Map, Root};
use kelvin_hamt::DefaultHAMTMap as HAMT;
use kelvin_two3::DefaultTwo3Map as Two3;

#[derive(Clone, Canon)]
struct State<S: Store> {
    map_a: HAMT<String, String, S>,
    map_b: Two3<u64, u64, S>,
    counter: u64,
}

// The initial root state
impl<S: Store> Default for State<S> {
    fn default() -> Self {
        // Set up a default kv for map_a:
        let mut map_a = HAMT::new();
        map_a
            .insert("Hello".into(), "World".into())
            .expect("in memory");
        State {
            map_a,
            map_b: Two3::default(),
            counter: 0,
        }
    }
}

fn main() -> Result<(), S::Error> {
    let mut root = Root::<_, Blake2b>::new("/tmp/kelvin-example")?;

    let mut state: State<_> = root.restore()?;

    match state.map_a.get("Foo")? {
        Some(path) => println!("Foo is {}", *path),
        None => println!("Foo is `None`"),
    }

    println!("Counter is {}", state.counter);

    state.counter += 1;
    state.map_a.insert(
        "Foo".into(),
        format!("Bar {}", state.counter * state.counter),
    )?;

    root.set_root(&mut state)?;

    Ok(())
}
