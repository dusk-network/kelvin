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
