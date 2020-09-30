// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod fuzz;
mod quickcheck_map;
mod quickcheck_stack;

use crate::{ByteHash, Compound, HandleType};
pub use fuzz::{fuzz_content, fuzz_content_iterations};
pub use quickcheck;
pub use rand;
pub use tempfile;

pub use arbitrary;

/// Trait to test for correct empty state of a structure
pub trait CorrectEmptyState<H> {
    /// Make sure the collection is properly empty
    fn assert_correct_empty_state(&self);
}

impl<C, H> CorrectEmptyState<H> for C
where
    C: Compound<H>,
    H: ByteHash,
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
