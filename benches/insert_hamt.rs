#[macro_use]
extern crate criterion;

use criterion::Criterion;

use kelvin::Map;

fn insert() {
    let mut map = Map::new();
    for i in 0..10_000 {
        let _ = map.insert(i, i);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("insert_hamt", |b| b.iter(|| insert()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
