// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use std::ops::Deref;

use canonical::Canon;
use canonical_derive::Canon;

use crate::Associative;

/// Annotation used to keep track of minimum key in subtrees
#[derive(Clone, Debug, Canon)]
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

impl<T, K> From<&T> for MaxKey<K>
where
    T: AsRef<K>,
    K: MaxKeyType,
{
    fn from(t: &T) -> Self {
        MaxKey(t.as_ref().clone())
    }
}
