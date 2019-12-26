use std::borrow::Borrow;
use std::io;
use std::ops::AddAssign;

use bytehash::ByteHash;
use num::{One, Zero};

use super::Associative;
use crate::{Compound, Content, Sink, Source};

/// Trait group for Cardinality inner type
pub trait Counter: AddAssign + Copy + Zero + One {}
impl<T> Counter for T where T: AddAssign + Copy + Zero + One {}

/// Annotation that keeps track of total number of leaves
#[derive(PartialEq, Eq, Clone)]
pub struct Cardinality<T>(T);

impl<T> Associative for Cardinality<T>
where
    T: Counter,
{
    fn op(&mut self, b: &Self) {
        self.0 += b.0;
    }
}

impl<Anything, U> From<&Anything> for Cardinality<U>
where
    U: Counter,
{
    fn from(_: &Anything) -> Self {
        Cardinality(U::one())
    }
}

impl<H, U> Content<H> for Cardinality<U>
where
    H: ByteHash,
    U: Content<H> + Counter,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        self.0.persist(sink)
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok(Cardinality(U::restore(source)?))
    }
}

/// Method for counting the number of elements in the collection
pub trait Count<U, H> {
    /// Returns the number of elements in collection
    fn count(&self) -> U;
}

impl<U, C, H> Count<U, H> for C
where
    U: Counter,
    H: ByteHash,
    C: Compound<H>,
    C::Annotation: Borrow<Cardinality<U>>,
{
    fn count(&self) -> U {
        self.annotation()
            .map(|ann| ann.borrow().0)
            .unwrap_or_else(U::zero)
    }
}
