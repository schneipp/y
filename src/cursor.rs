use crate::buffer::YBuffer;

#[derive(Debug, Clone)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub desired_col: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            desired_col: 0,
        }
    }

    pub fn get_character_number(&self, buffer: &YBuffer) -> usize {
        let mut char_count = 0;

        for i in 0..self.row {
            if i < buffer.lines.len() {
                char_count += buffer.lines[i].char_count() + 1;
            }
        }

        char_count += self.col;
        char_count
    }
}
