// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// The state of drawing
#[derive(Default)]
pub struct DrawState {
    /// the current level of drawing recursion
    pub recursion: usize,
}

impl DrawState {
    /// pad the output to the current recursion level
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
