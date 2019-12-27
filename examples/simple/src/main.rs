use std::io;

use kelvin::{Blake2b, ByteHash, Content, Map, Root, Sink, Source};
use kelvin_btree::BTree;

#[derive(Clone)]
struct State<H: ByteHash> {
    map_a: BTree<String, String, H>,
    map_b: BTree<u64, u64, H>,
    counter: u64,
}

// The initial root state
impl<H: ByteHash> Default for State<H> {
    fn default() -> Self {
        // Set up a default kv for map_a:
        let mut map_a = BTree::new();
        map_a
            .insert("Hello".into(), "World".into())
            .expect("in memory");
        State {
            map_a,
            map_b: BTree::default(),
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
            map_a: BTree::restore(source)?,
            map_b: BTree::restore(source)?,
            counter: u64::restore(source)?,
        })
    }
}

fn main() -> io::Result<()> {
    let mut root = Root::<_, Blake2b>::new("/tmp/kelvin-example")?;

    let mut state: State<_> = root.restore()?;

    match state.map_a.get(&"Foo".to_owned())? {
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
