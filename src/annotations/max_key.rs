// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

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

impl<T, K> From<&T> for MaxKey<K>
where
    T: AsRef<K>,
    K: MaxKeyType,
{
    fn from(t: &T) -> Self {
        MaxKey(t.as_ref().clone())
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
