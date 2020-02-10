use std::io;

use kelvin::{Blake2b, ByteHash, Content, Map, Root, Sink, Source};
use kelvin_hamt::DefaultHAMTMap as HAMT;
use kelvin_two3::DefaultTwo3Map as Two3;

#[derive(Clone)]
struct State<H: ByteHash> {
    map_a: HAMT<String, String, H>,
    map_b: Two3<u64, u64, H>,
    counter: u64,
}

// The initial root state
impl<H: ByteHash> Default for State<H> {
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

impl<H: ByteHash> Content<H> for State<H> {
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        self.map_a.persist(sink)?;
        self.map_b.persist(sink)?;
        self.counter.persist(sink)
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok(State {
            map_a: HAMT::restore(source)?,
            map_b: Two3::restore(source)?,
            counter: u64::restore(source)?,
        })
    }
}

fn main() -> io::Result<()> {
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
