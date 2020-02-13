mod fuzz;
mod quickcheck_map;
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
