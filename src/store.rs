use std::marker::PhantomData;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fmt, io};

use arrayvec::ArrayVec;
use bytehash::ByteHash;
use cache::Cache;
use parking_lot::RwLock;

use crate::backend::{Backend, MemBackend, PutResult};
use crate::content::Content;
use crate::handle::Handle;
use crate::sink::StoreSink;
use crate::source::Source;

#[derive(Clone)]
pub struct Store<H: ByteHash>(Arc<StoreInner<H>>);

const GENERATIONS: usize = 8;

pub struct StoreInner<H: ByteHash> {
    generations: ArrayVec<[RwLock<Box<dyn Backend<H>>>; GENERATIONS]>,
    cache: Cache<H::Digest>,
}

impl<H: ByteHash> fmt::Debug for Store<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Store")
    }
}

pub struct Shared<T, H: ByteHash>(T, PhantomData<H>);

unsafe impl<T, H: ByteHash> Send for Shared<T, H> {}

// impl<T: Persist<H>, H: ByteHash> Shared<T, H> {
//     pub fn make_local(self) -> T {
//         self.0
//     }
// }

// pub trait Persist<H: ByteHash>
// where
//     Self: Sized + Clone,
// {
//     type Inner: Content<H>;

//     fn store(&self) -> &Store<H>;
//     fn from_inner_store(inner: Self::Inner, store: &Store<H>) -> Self;
//     fn inner_mut(&mut self) -> &mut Self::Inner;

//     fn snapshot(
//         &mut self,
//         store: &Store<H>,
//     ) -> io::Result<Snapshot<Self::Inner, H>> {
//         // let mut sink = StoreSink::new(store);
//         // self.inner_mut().persist(&mut sink)?;
//         // Ok(Snapshot {
//         //     hash: sink.fin()?,
//         //     store: store.clone(),
//         //     _marker: PhantomData,
//         // })
//         unimplemented!()
//     }

//     fn share(&mut self) -> Shared<Self, H> {
//         for c in self.inner_mut().children_mut() {
//             c.make_shared()
//         }
//         Shared(self.clone(), PhantomData)
//     }

//     // fn from_snapshot(snap: &Snapshot<Self::Inner, H>) -> io::Result<Self> {
//     //     unimplemented!()
//     //     // let inner = snap.store.restore(&snap.hash)?;
//     //     // Ok(Self::from_inner_store(inner, &snap.store))
//     // }

//     // Writes out the datastructure as the root, possibly deleting nodes
//     // not reachable from this root.
//     // fn commit(&self, store: &Store<H>, level: usize) -> io::Result<()> {
//     //     unimplemented!()
//     // }
// }

// pub struct Snapshot<T, H: ByteHash> {
//     inner: Handle,
//     store: Store<H>,
//     _marker: PhantomData<T>,
// }

// impl<T, H: ByteHash> Clone for Snapshot<T, H> {
//     fn clone(&self) -> Self {
//         Snapshot {
//             hash: self.hash.clone(),
//             store: self.store.clone(),
//             _marker: PhantomData,
//         }
//     }
// }

#[derive(Clone)]
pub struct Snapshot<T, H: ByteHash> {
    hash: H::Digest,
    _marker: PhantomData<T>,
}

impl<T: Content<H>, H: ByteHash> Snapshot<T, H> {
    pub(crate) fn new(hash: H::Digest) -> Self {
        Snapshot {
            hash,
            _marker: PhantomData,
        }
    }

    pub fn restore(&self, store: &Store<H>) -> io::Result<T> {
        unimplemented!()
        //Ok(store.get::<T, C>(&self.hash)?.clone())
    }
}

impl<N, H: ByteHash> Deref for Snapshot<N, H> {
    type Target = H::Digest;
    fn deref(&self) -> &Self::Target {
        &self.hash
    }
}

impl<H: ByteHash> Store<H> {
    pub fn new<P: Into<PathBuf>>(_path: &P) -> Self {
        let mem = MemBackend::new();
        let mut generations = ArrayVec::new();
        generations.push(RwLock::new(Box::new(mem) as Box<dyn Backend<H>>));

        Store(Arc::new(StoreInner {
            generations,
            cache: Cache::new(32, 4096),
        }))
    }

    pub fn persist<T: Content<H>>(
        &self,
        content: &mut T,
    ) -> io::Result<Snapshot<T, H>> {
        let children: &mut [Handle<T::Leaf, T::Node, H>] =
            content.children_mut::<T>();
        for c in children {
            c.pre_persist(self)?;
        }

        let mut sink = StoreSink::new(self);
        content.persist(&mut sink)?;
        Ok(Snapshot {
            hash: sink.fin()?,
            _marker: PhantomData,
        })
    }

    // pub(crate) fn get<T: Content<T, H>, N: Content<T, H>>(
    //     &self,
    //     hash: &H::Digest,
    // ) -> io::Result<Cached<T>> {
    //     let t = self.restore(hash)?;
    //     Ok(self.0.cache.insert(hash.clone(), t))
    // }

    pub(crate) fn put(
        &self,
        hash: H::Digest,
        bytes: Vec<u8>,
    ) -> io::Result<PutResult> {
        self.0.generations[0].write().put(hash, bytes)
    }

    pub fn restore<T: Content<H>, N: Content<H>>(
        &self,
        hash: &H::Digest,
    ) -> io::Result<T> {
        for gen in self.0.generations.as_ref() {
            match gen.read().get(hash) {
                Ok(read) => {
                    let mut source = Source::new(read, self);
                    return T::restore(&mut source);
                }
                Err(_) => (),
            }
        }
        panic!("could not restore");
    }

    // pub fn backend(&self) -> &Backend<H> {
    //     &*self.0.backend
    // }

    pub fn size(&self) -> usize {
        let mut size = 0;
        for gen in self.0.generations.as_ref() {
            size += gen.read().size();
        }
        size
    }
}
