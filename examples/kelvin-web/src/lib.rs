mod utils;

use wasm_bindgen::prelude::*;
use kelvin::{Map, DefaultStore};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn entry() {
    let mut map = Map::new();

    for i in 0..1000 {
        map.insert(i, i).unwrap();
    }

    let store = DefaultStore::new("kelvin").unwrap();

    let snap = store.persist(&mut map).unwrap();

    let restored = store.restore(&snap).unwrap();

    let s1 = format!("orig index {} is {}", 32, *map.get(&32).unwrap().unwrap());
    alert(&s1);

    let s2 = format!("restored index {} is {}", 32, *restored.get(&32).unwrap().unwrap());
    alert(&s2);
}
