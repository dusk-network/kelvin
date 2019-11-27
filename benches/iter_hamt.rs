#[macro_use]
extern crate criterion;

use criterion::Criterion;

use kelvin::LeafIterable;
use kelvin::Map;

fn iter(map: &Map<u32, u32>) {
    for kv in map.iter() {
        let (k, v) = kv.unwrap();
        assert!(k == v)
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut map = Map::new();
    for i in 0..10_000 {
        let _ = map.insert(i, i);
    }
    c.bench_function("iter_hamt", move |b| b.iter(|| iter(&map)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
