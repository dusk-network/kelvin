use std::borrow::Borrow;
use std::io;
use std::ops::AddAssign;

use bytehash::ByteHash;
use num::{One, Zero};

use super::Associative;
use crate::{Compound, Content, Sink, Source};

#[derive(PartialEq, Eq, Clone)]
pub struct Cardinality<T>(T);

impl<T> Associative for Cardinality<T>
where
    T: AddAssign + Copy,
{
    fn op(&mut self, b: &Self) {
        self.0 += b.0;
    }
}

// Leaves are always 1
impl<T, U> From<&T> for Cardinality<U>
where
    U: One,
{
    fn from(_: &T) -> Self {
        Cardinality(U::one())
    }
}

impl<H: ByteHash, T: Content<H>> Content<H> for Cardinality<T> {
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        self.0.persist(sink)
    }
    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok(Cardinality(T::restore(source)?))
    }
}

pub trait Count<U, H> {
    fn count(&self) -> U;
}

impl<U, C, H> Count<U, H> for C
where
    U: Zero + Clone,
    H: ByteHash,
    C: Compound<H>,
    C::Annotation: Borrow<Cardinality<U>>,
{
    fn count(&self) -> U {
        self.annotation()
            .map(|ann| ann.borrow().0.clone())
            .unwrap_or(U::zero())
    }
}
