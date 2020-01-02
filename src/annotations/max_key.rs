use std::io;
use std::ops::Deref;

use crate::{Associative, ByteHash, Content, Sink, Source};

/// Annotation used to keep track of minimum key in subtrees
#[derive(Clone, Debug)]
pub struct MaxKey<K>(K);

/// Trait group for keys
pub trait MaxKeyType: Ord + Clone {}
impl<T> MaxKeyType for T where T: Ord + Clone {}

impl<K> Deref for MaxKey<K> {
    type Target = K;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K> Associative for MaxKey<K>
where
    K: MaxKeyType,
{
    // Take the minimal key
    fn op(&mut self, b: &Self) {
        if b.0 > self.0 {
            self.0 = b.0.clone()
        }
    }
}

impl<K, V> From<&(K, V)> for MaxKey<K>
where
    K: MaxKeyType,
{
    fn from((k, _): &(K, V)) -> Self {
        MaxKey(k.clone())
    }
}

impl<H: ByteHash, K: Content<H>> Content<H> for MaxKey<K>
where
    K: MaxKeyType,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        self.0.persist(sink)
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok(MaxKey(K::restore(source)?))
    }
}
