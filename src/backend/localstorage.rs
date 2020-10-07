// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::io::{self, Read, Write};
use core::marker::PhantomData;
use std::path::PathBuf;

use base64::{decode, encode_config, encode_config_buf, STANDARD_NO_PAD};
use bytehash::ByteHash;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::Storage;

use crate::backend::{Backend, PutResult};

pub struct WebBackend<S: Store> {
    storage: web_sys::Storage,
    name: String,
    _marker: PhantomData<S>,
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

unsafe impl<S: Store> Send for WebBackend<S> {}
unsafe impl<S: Store> Sync for WebBackend<S> {}

impl<S: Store> WebBackend<S> {
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

impl<S: Store> Backend<S> for WebBackend<S> {
    fn get<'a>(&'a self, hash: &S::Ident) -> io::Result<Box<dyn Read + 'a>> {
        let mut key = self.name.clone();
        encode_config_buf(hash.as_ref(), STANDARD_NO_PAD, &mut key);

        if let Some(value) = self.storage.get_item(&key).unwrap() {
            Ok(Box::new(io::Cursor::new(decode(&value).unwrap())))
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "Data not found"))
        }
    }

    fn put(&mut self, hash: S::Ident, bytes: Vec<u8>) -> io::Result<PutResult> {
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
