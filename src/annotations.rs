use std::borrow::Borrow;
use std::io;

use bytehash::ByteHash;

use crate::{Content, Sink, Source};

/// Defines the associative operation for the annotation type
pub trait Associative {
    /// Perform the associative operation on self
    fn op(&mut self, b: &Self);
}

/// Defines how multiple annotation types are combined
pub trait Combine: Sized {
    /// Combines n>1 elements into one
    fn combine(elements: impl IntoIterator<Item = impl Borrow<Self>>) -> Self;
}

impl<T> Combine for T
where
    T: Associative + Clone,
{
    fn combine(elements: impl IntoIterator<Item = impl Borrow<Self>>) -> Self {
        let mut iter = elements.into_iter();
        let mut a = iter
            .next()
            .expect("Combine called on empty iterator")
            .borrow()
            .clone();

        while let Some(next) = iter.next() {
            a.op(next.borrow())
        }
        a
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

#[cfg(test)]
mod test {
    use super::*;

    #[derive(PartialEq, Eq, Clone)]
    struct Max(usize);

    impl Associative for Max {
        fn op(&mut self, b: &Self) {
            if b.0 > self.0 {
                self.0 = b.0
            }
        }
    }

    #[test]
    fn maxes() {
        let maxes = [Max(0), Max(4), Max(32), Max(8), Max(3), Max(0)];
        assert!(Max::combine(&maxes) == Max(32));
    }

    #[test]
    fn max() {
        let mut a = Max(10);
        let b = Max(20);

        a.op(&b);
        assert!(a.0 == 20);
    }
}
