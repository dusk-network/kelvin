mod quickcheck_map;
pub use quickcheck;
pub use rand;
pub use tempfile;

use crate::{ByteHash, Compound, HandleType};

/// Trait to test for correct empty state of a structure
pub trait CorrectEmptyState<H> {
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
