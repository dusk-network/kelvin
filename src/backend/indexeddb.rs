use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::path::PathBuf;

use bytehash::ByteHash;

use futures::channel::oneshot::channel;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    Event,
    // IdbCursorWithValue,
    IdbDatabase,
    IdbOpenDbRequest,
    IdbRequest,
};

use crate::backend::{Backend, PutResult};

pub struct WebBackend<H: ByteHash> {
    db: IdbDatabase,
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
        let pb = name.into();
        let s = pb.to_str().expect("invalid name");

        let window = web_sys::window().expect("no window avaliable");

        let factory = window
            .indexed_db()
            .expect("no idb available")
            .expect("no idb available");

        let open = factory.open(s).unwrap();

        let (tx, mut rx) = channel();

        let success = Closure::once(move |event: &Event| {
            let target = event.target().expect("could not create db");

            let req =
                target.dyn_ref::<IdbRequest>().expect("could not create db");

            let result = req.result().expect("could not create db");

            assert!(result.is_instance_of::<IdbDatabase>());
            let db = IdbDatabase::from(result);

            tx.send(db).expect("could not create db");
            alert("sent");
        });

        open.set_onsuccess(Some(success.as_ref().unchecked_ref()));

        while rx.try_recv().unwrap().is_none() {
            alert("sleeping");
            std::thread::sleep(std::time::Duration::from_millis(100));
            alert("waking");
        }

        alert("finished sleeping");

        alert(&format!("{:?}", rx.try_recv()));

        // let db = rx
        //     .try_recv()
        //     .expect("could not create db")
        //     .expect("could not create db");

        // alert("received");

        // Ok(WebBackend {
        //     db,
        //     _marker: PhantomData,
        // })
        unimplemented!()
    }
}

impl<H: ByteHash> Backend<H> for WebBackend<H> {
    fn get<'a>(&'a self, hash: &H::Digest) -> io::Result<Box<dyn Read + 'a>> {
        alert("got to get!");
        unimplemented!()
    }

    fn put(&self, hash: H::Digest, bytes: Vec<u8>) -> io::Result<PutResult> {
        alert("got to put!");
        unimplemented!()
    }

    fn size(&self) -> usize {
        unimplemented!()
    }
}
