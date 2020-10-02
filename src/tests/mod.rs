// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

mod fuzz;
mod quickcheck_map;
mod quickcheck_stack;

use crate::{Compound, HandleType};
pub use arbitrary;
pub use fuzz::{fuzz_content, fuzz_content_iterations};
pub use quickcheck;
pub use rand;
pub use tempfile;

use canonical::Store;

/// Trait to test for correct empty state of a structure
pub trait CorrectEmptyState<Se> {
    /// Make sure the collection is properly empty
    fn assert_correct_empty_state(&self);
}

impl<C, S> CorrectEmptyState<S> for C
where
    C: Compound<S>,
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
