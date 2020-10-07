// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

#[cfg(test)]
extern crate std;

mod quickcheck_map;
mod quickcheck_stack;

use crate::{Compound, HandleType};
pub use arbitrary;
pub use quickcheck;
pub use rand;
pub use tempfile;

use canonical::Store;

/// Trait to test for correct empty state of a structure
pub trait CorrectEmptyState<Se, const N: usize> {
    /// Make sure the collection is properly empty
    fn assert_correct_empty_state(&self);
}

impl<C, S, const N: usize> CorrectEmptyState<S, N> for C
where
    C: Compound<S, N>,
    S: Store,
{
    fn assert_correct_empty_state(&self) {
        for child in self.children().iter() {
            match child.handle_type() {
                HandleType::None => (),
                _ => panic!("invalid empty state"),
            }
        }
    }
}
