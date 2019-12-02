use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::path::PathBuf;

use base64::{decode, encode_config, encode_config_buf, STANDARD_NO_PAD};
use bytehash::ByteHash;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::Storage;

use crate::backend::{Backend, PutResult};

pub struct WebBackend<H: ByteHash> {
    storage: web_sys::Storage,
    name: String,
    _marker: PhantomData<H>,
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

unsafe impl<H: ByteHash> Send for WebBackend<H> {}
unsafe impl<H: ByteHash> Sync for WebBackend<H> {}

impl<H: ByteHash> WebBackend<H> {
    pub fn new<P: Into<PathBuf>>(name: P) -> io::Result<Self> {
        let name = name.into().to_str().expect("invalid name").to_owned();
        let window = web_sys::window().expect("Could not get local storage");
        if let Ok(Some(storage)) = window.local_storage() {
            Ok(WebBackend {
                storage,
                name,
                _marker: PhantomData,
            })
        } else {
            panic!("Could not get local storage")
        }
    }
}

impl<H: ByteHash> Backend<H> for WebBackend<H> {
    fn get<'a>(&'a self, hash: &H::Digest) -> io::Result<Box<dyn Read + 'a>> {
        let mut key = self.name.clone();
        encode_config_buf(hash.as_ref(), STANDARD_NO_PAD, &mut key);

        if let Some(value) = self.storage.get_item(&key).unwrap() {
            Ok(Box::new(io::Cursor::new(decode(&value).unwrap())))
        } else {
            panic!();
        }
    }

    fn put(&self, hash: H::Digest, bytes: Vec<u8>) -> io::Result<PutResult> {
        let mut key = self.name.clone();
        encode_config_buf(hash.as_ref(), STANDARD_NO_PAD, &mut key);

        if let Some(_) = self.storage.get_item(&key).unwrap() {
            Ok(PutResult::AlreadyThere)
        } else {
            let value = encode_config(&bytes, STANDARD_NO_PAD);
            self.storage.set_item(&key, &value).unwrap();
            Ok(PutResult::Ok)
        }
    }

    fn size(&self) -> usize {
        unimplemented!()
    }
}
