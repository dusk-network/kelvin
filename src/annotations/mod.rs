use std::borrow::{Borrow, Cow};
use std::io;

use bytehash::ByteHash;

pub use cardinality::{Cardinality, Count, Counter};

pub use max_key::{MaxKey, MaxKeyType};

use crate::{Content, Sink, Source};

mod annotation_macro;
mod cardinality;

mod max_key;

/// Wrapper trait for hiding generics when working on select functions
pub trait Annotation<A: Clone> {
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
        E: Annotation<A>;
}

impl<A, T> Combine<A> for T
where
    Self: Associative + Clone,
{
    fn combine<E>(elements: &[E]) -> Option<T>
    where
        A: Borrow<Self> + Clone,
        E: Annotation<A>,
    {
        let mut iter = elements.iter().filter_map(Annotation::annotation);

        iter.next().map(|first| {
            //let a: &A = first.borrow();
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
#[derive(Clone, PartialEq, Eq)]
pub struct VoidAnnotation;

impl<T> From<&T> for VoidAnnotation {
    fn from(_: &T) -> Self {
        VoidAnnotation
    }
}

impl Associative for VoidAnnotation {
    fn op(&mut self, _: &Self) {}
}

impl<H: ByteHash> Content<H> for VoidAnnotation {
    fn persist(&mut self, _: &mut Sink<H>) -> io::Result<()> {
        Ok(())
    }
    fn restore(_: &mut Source<H>) -> io::Result<Self> {
        Ok(VoidAnnotation)
    }
}
