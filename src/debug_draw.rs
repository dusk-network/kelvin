use std::fmt::Write;

use crate::{ByteHash, Compound};

/// Trait allowing structures to be printed for debugging.
pub trait DebugDraw<H> {
    /// Draw the structure as a string
    fn draw(&self) -> String;
}

impl<C, H> DebugDraw<H> for C
where
    C: Compound<H>,
    C::Leaf: std::fmt::Debug,
    H: ByteHash,
{
    fn draw(&self) -> String {
        let mut s = String::new();

        write!(&mut s, "{}: [ ", self.children().len()).unwrap();

        let mut iter = self.children().into_iter();

        if let Some(n) = iter.next() {
            write!(&mut s, "{}", n.draw()).unwrap();
        }

        for el in iter {
            write!(&mut s, ", {}", el.draw()).unwrap();
        }

        write!(&mut s, "] ").unwrap();

        s
    }
}
