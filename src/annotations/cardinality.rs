// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::borrow::Borrow;
use std::cmp::Ord;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use canonical::{Canon, Store};
use canonical_derive::Canon;
use num::{One, Zero};

use super::Associative;
use crate::branch::Branch;
use crate::handle::HandleType;
use crate::search::{Method, SearchResult};
use crate::Compound;

/// Trait group for Cardinality inner type
pub trait Counter:
    Add + Sub + SubAssign + AddAssign + Copy + Zero + One + Ord
{
}
impl<T> Counter for T where
    T: Add + AddAssign + Sub + SubAssign + Copy + Zero + One + Ord
{
}

/// Annotation that keeps track of total number of leaves
#[derive(PartialEq, Eq, Clone, Canon, Debug)]
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

/// Trait for counting the number of elements in the collection
pub trait Count<U, S> {
    /// Returns the number of elements in collection
    fn count(&self) -> U;
}

impl<U, C, S> Count<U, S> for C
where
    U: Counter,
    S: Store,
    C: Compound<S>,
    C::Annotation: Borrow<Cardinality<U>>,
{
    fn count(&self) -> U {
        self.annotation()
            .map(|ann| ann.borrow().0)
            .unwrap_or_else(U::zero)
    }
}

/// A search method to find the Nth element of a compound type
pub struct Nth<U>(U);

impl<U> Nth<U> {
    /// Creates a new search instance searching for the `n`:th element
    pub fn new(n: U) -> Self {
        Nth(n)
    }
}

impl<'a, C, U, S> Method<C, S> for Nth<U>
where
    C: Compound<S>,
    C::Annotation: Borrow<Cardinality<U>>,
    S: Store,
    U: Counter,
{
    fn select(&mut self, compound: &C, _: usize) -> SearchResult {
        for (i, child) in compound.children().iter().enumerate() {
            match child.handle_type() {
                HandleType::Leaf => {
                    if self.0 == U::zero() {
                        return SearchResult::Leaf(i);
                    } else {
                        self.0 -= U::one();
                    }
                }
                HandleType::Node => {
                    if let Some(annotation) = child.annotation() {
                        let c: &Cardinality<U> = (*annotation).borrow();
                        if self.0 >= c.0 {
                            self.0 -= c.0
                        } else {
                            return SearchResult::Path(i);
                        }
                    }
                }
                HandleType::None => (),
            }
        }
        // found nothing
        SearchResult::None
    }
}

/// Trait for finding the nth element of a collection
pub trait GetNth<U, S>: Sized + Clone
where
    S: Store,
{
    /// Returns a branch to the n:th element, if any
    fn nth(&self, i: U) -> Result<Option<Branch<Self, S>>, S::Error>;
}

impl<C, U, S> GetNth<U, S> for C
where
    C: Compound<S>,
    C::Annotation: Borrow<Cardinality<U>>,
    U: Counter,
    S: Store,
{
    fn nth(&self, i: U) -> Result<Option<Branch<Self, S>>, S::Error> {
        Branch::new(self.clone(), &mut Nth::new(i))
    }
}
