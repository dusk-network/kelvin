#[macro_use]
extern crate criterion;

use criterion::Criterion;

use std::collections::BTreeMap;

fn insert() {
    let mut map = BTreeMap::new();
    for i in 0..10_000 {
        map.insert(i, i);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("insert_btree", |b| b.iter(|| insert()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
