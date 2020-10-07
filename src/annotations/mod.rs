// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use core::borrow::Borrow;

use canonical::{Canon, Store};
use canonical_derive::Canon;

pub use cardinality::{Cardinality, Count, Counter, GetNth, Nth};
pub use max_key::{MaxKey, MaxKeyType};

mod annotation_macro;
mod cardinality;

mod max_key;

/// Helper group-trait for annotations
pub trait Annotation<L, S>:
    'static + Combine<Self> + for<'l> From<&'l L> + Canon<S>
where
    S: Store,
{
}

impl<A, L, S> Annotation<L, S> for A
where
    A: 'static + Combine<Self> + for<'any> From<&'any L> + Canon<S>,
    S: Store,
{
}

/// Wrapper trait for hiding generics when working on select functions
pub trait ErasedAnnotation<A: Clone>: Clone {
    /// Returns the annotation of &self, if any.
    fn annotation(&self) -> Option<A>;
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
            let t: &T = first.borrow();
            let mut s: T = t.clone();
            for next in iter {
                s.op(next.borrow())
            }
            s
        })
    }
}

/// Empty annotation
#[derive(Clone, PartialEq, Eq, Debug, Canon)]
pub struct Void;

impl<T> From<&T> for Void {
    fn from(_: &T) -> Self {
        Void
    }
}

impl Associative for Void {
    fn op(&mut self, _: &Self) {}
}
