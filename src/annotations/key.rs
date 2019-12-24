use std::io;
use std::ops::Deref;

use crate::{Associative, ByteHash, Content, Sink, Source};

/// Annotation used to keep track of minimum key in subtrees
#[derive(Clone)]
pub struct Key<K>(K);

/// Trait group for keys
pub trait KeyType: Eq + Ord + Clone {}
impl<T> KeyType for T where T: Eq + Ord + Clone {}

impl<K> Deref for Key<K>
where
    K: KeyType,
{
    type Target = K;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K> Associative for Key<K>
where
    K: KeyType,
{
    // Take the minimal key
    fn op(&mut self, b: &Self) {
        if b.0 < self.0 {
            self.0 = b.0.clone()
        }
    }
}

impl<K, V> From<&(K, V)> for Key<K>
where
    K: KeyType,
{
    fn from((k, _): &(K, V)) -> Self {
        Key(k.clone())
    }
}

impl<H: ByteHash, K: Content<H>> Content<H> for Key<K>
where
    K: KeyType,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        self.0.persist(sink)
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok(Key(K::restore(source)?))
    }
}
