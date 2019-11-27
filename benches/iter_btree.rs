#[macro_use]
extern crate criterion;

use criterion::Criterion;

use std::collections::BTreeMap;

fn iter(map: &BTreeMap<u32, u32>) {
    for (k, v) in map.iter() {
        assert!(k == v)
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut map = BTreeMap::new();
    for i in 0..10_000 {
        let _ = map.insert(i, i);
    }
    c.bench_function("iter_btree", move |b| b.iter(|| iter(&map)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
