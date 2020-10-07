// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

/*
use arrayvec::ArrayString;

/// The state of drawing
#[derive(Default)]
pub struct DrawState {
    /// the current level of drawing recursion
    pub recursion: usize,
}

impl DrawState {
    /// pad the output to the current recursion level
    pub fn pad<const N: usize>(&self) -> ArrayString<_, N> {
        let mut string = String::new();
        for _ in 0..self.recursion {
            string.push_str("  ");
        }
        string
    }
}

/// Trait allowing structures to be printed for debugging.
pub trait DebugDraw<S, const N: usize> {
    /// Draw the structure as a string
    fn draw_conf(&self, state: &mut DrawState) -> ArrayString<_, N>;
    /// Draw the structure as a string
    fn draw(&self) -> String {
        self.draw_conf(&mut DrawState::default())
    }
}
*/
