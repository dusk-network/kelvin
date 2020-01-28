use std::fmt::Write;

use crate::{ByteHash, Compound};

#[derive(Default)]
pub struct DrawState {
    pub recursion: usize,
}

impl DrawState {
    pub fn pad(&self) -> String {
        let mut string = String::new();
        for _ in 0..self.recursion {
            string.push_str("  ");
        }
        string
    }
}

/// Trait allowing structures to be printed for debugging.
pub trait DebugDraw<H> {
    /// Draw the structure as a string
    fn draw_conf(&self, state: &mut DrawState) -> String;
    /// Draw the structure as a string
    fn draw(&self) -> String {
        self.draw_conf(&mut DrawState::default())
    }
}

impl<C, H> DebugDraw<H> for C
where
    C: Compound<H>,
    C::Leaf: std::fmt::Debug,
    H: ByteHash,
{
    fn draw_conf(&self, state: &mut DrawState) -> String {
        let mut s = String::new();

        write!(&mut s, "[ ").unwrap();

        let mut iter = self.children().iter();

        if let Some(n) = iter.next() {
            write!(&mut s, "{}", n.draw_conf(state)).unwrap();
        }

        for el in iter {
            write!(&mut s, ", {}", el.draw_conf(state)).unwrap();
        }

        write!(&mut s, "] ").unwrap();
        s
    }
}
