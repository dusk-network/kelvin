use std::borrow::{Borrow, Cow};
use std::io;

use bytehash::ByteHash;

pub use cardinality::{Cardinality, Count, Counter, GetNth, Nth};
pub use max_key::{MaxKey, MaxKeyType};

use crate::{Content, Sink, Source};

mod annotation_macro;
mod cardinality;

mod max_key;

/// Helper group-trait for annotations
pub trait Annotation<L, H>:
    'static + Combine<Self> + for<'l> From<&'l L> + Content<H>
where
    H: ByteHash,
{
}

impl<A, L, H> Annotation<L, H> for A
where
    A: 'static + Combine<Self> + for<'any> From<&'any L> + Content<H>,
    H: ByteHash,
{
}

/// Wrapper trait for hiding generics when working on select functions
pub trait ErasedAnnotation<A: Clone>: Clone {
    /// Returns the annotation of &self, if any.
    fn annotation(&self) -> Option<Cow<A>>;
}

/// Defines the associative operation for the annotation type
pub trait Associative {
    /// Perform the associative operation on self
    fn op(&mut self, b: &Self);
}

/// Defines how annotation types are combined. Prefer using `Associative` when possible.
pub trait Combine<A>: Sized + Clone {
    /// Combines n annotation elements into one
    fn combine<E>(elements: &[E]) -> Option<Self>
    where
        A: Borrow<Self> + Clone,
        E: ErasedAnnotation<A>;
}

impl<A, T> Combine<A> for T
where
    Self: Associative + Clone,
{
    fn combine<E>(elements: &[E]) -> Option<T>
    where
        A: Borrow<Self> + Clone,
        E: ErasedAnnotation<A>,
    {
        let mut iter = elements.iter().filter_map(ErasedAnnotation::annotation);

        iter.next().map(|first| {
            let t: &T = (*first).borrow().borrow();
            let mut s: T = t.clone();
            for next in iter {
                let a: &A = next.borrow();
                s.op(a.borrow())
            }
            s
        })
    }
}

/// Empty annotation
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Void;

impl<T> From<&T> for Void {
    fn from(_: &T) -> Self {
        Void
    }
}

impl Associative for Void {
    fn op(&mut self, _: &Self) {}
}

impl<H: ByteHash> Content<H> for Void {
    fn persist(&mut self, _: &mut Sink<H>) -> io::Result<()> {
        Ok(())
    }
    fn restore(_: &mut Source<H>) -> io::Result<Self> {
        Ok(Void)
    }
}
