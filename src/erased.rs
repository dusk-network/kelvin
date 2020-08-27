// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use bytehash::ByteHash;

use crate::content::Content;
use crate::store::Store;
use crate::{Sink, Source};

/// A type-erased snapshot of a Compound structure.
///
/// Can be used to hide type parameters in complex type definitions
/// and acts as a kind of `Any` type for other types implementing `Content`
#[derive(Clone)]
pub struct Erased<H: ByteHash> {
    hash: H::Digest,
    store: Store<H>,
}

/// Type representing a query over an `Erased` wrapper
pub struct Query<T, H> {
    inner: T,
    _marker: PhantomData<H>,
}

/// Type representing a transaction in progress over an `Erased` wrapper
pub struct Transaction<'a, T, H>
where
    H: ByteHash,
{
    inner: T,
    store: Store<H>,
    commit: &'a mut H::Digest,
}

impl<T, H> Deref for Query<T, H>
where
    H: ByteHash,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T, H> Deref for Transaction<'a, T, H>
where
    H: ByteHash,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T, H> DerefMut for Transaction<'a, T, H>
where
    H: ByteHash,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a, T, H> Transaction<'a, T, H>
where
    H: ByteHash,
    T: Content<H>,
{
    /// Commits the transaction to the underlying `Erased` wrapper
    pub fn commit(&mut self) -> io::Result<()> {
        *self.commit = self.store.persist(&mut self.inner)?.into_hash();
        Ok(())
    }
}

impl<H> Erased<H>
where
    H: ByteHash,
{
    /// Construct a new wrapper for the default value of `T`
    pub fn new<T: Content<H> + Default>(store: &Store<H>) -> io::Result<Self> {
        Self::wrap(T::default(), store)
    }

    /// Construct a new erased wrapper of T
    pub fn wrap<T: Content<H>>(mut t: T, store: &Store<H>) -> io::Result<Self> {
        let store = store.clone();
        let snap = store.persist(&mut t)?;
        Ok(Erased {
            hash: snap.into_hash(),
            store,
        })
    }

    /// Constructs a read-only query over the type in the `Erased` wrapper
    pub fn query<T>(&self) -> io::Result<Query<T, H>>
    where
        T: Content<H>,
    {
        let inner = self.store.get_hash(&self.hash)?;
        Ok(Query {
            inner,
            _marker: PhantomData,
        })
    }

    /// Constructs a transaction for the type in the `Erased` wrapper
    pub fn transaction<T>(&mut self) -> io::Result<Transaction<T, H>>
    where
        T: Content<H>,
    {
        let inner = self.store.get_hash(&self.hash)?;
        Ok(Transaction {
            inner,
            store: self.store.clone(),
            commit: &mut self.hash,
        })
    }
}

impl<H> Content<H> for Erased<H>
where
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        sink.write_all(self.hash.as_ref())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut hash = H::Digest::default();
        source.read_exact(hash.as_mut())?;
        Ok(Erased {
            hash,
            store: source.store().clone(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytehash::Blake2b;

    #[test]
    fn erased_u32() {
        let store = Store::<Blake2b>::ephemeral();

        let i = 77u32;

        let mut erased = Erased::wrap(i, &store).unwrap();

        assert_eq!(*erased.query::<u32>().unwrap(), 77);

        let mut trans = erased.transaction::<u32>().unwrap();
        *trans += 1;
        trans.commit().unwrap();

        assert_eq!(*erased.query::<u32>().unwrap(), 78);
    }

    #[derive(Clone)]
    struct Double<H>
    where
        H: ByteHash,
    {
        a: Erased<H>,
        b: Erased<H>,
    }

    impl<H> Content<H> for Double<H>
    where
        H: ByteHash,
    {
        fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
            self.a.persist(sink)?;
            self.b.persist(sink)
        }

        fn restore(source: &mut Source<H>) -> io::Result<Self> {
            Ok(Double {
                a: Erased::restore(source)?,
                b: Erased::restore(source)?,
            })
        }
    }

    #[test]
    fn double_wrapped() {
        let store = Store::<Blake2b>::ephemeral();

        let mut double = Double {
            a: Erased::wrap(13u32, &store).unwrap(),
            b: Erased::wrap(14u32, &store).unwrap(),
        };

        let snapshot = store.persist(&mut double).unwrap();

        let mut restored = store.restore(&snapshot).unwrap();

        assert_eq!(*restored.a.query::<u32>().unwrap(), 13);
        assert_eq!(*restored.b.query::<u32>().unwrap(), 14);

        let mut trans = restored.b.transaction::<u32>().unwrap();
        *trans = 12;
        trans.commit().unwrap();

        assert_eq!(*restored.b.query::<u32>().unwrap(), 12);
    }
}
