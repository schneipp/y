use crossterm::event::{KeyCode, KeyEvent};

use crate::editor::Editor;

impl Editor {
    pub fn handle_insert_mode(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.enter_normal_mode(),
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Enter => self.insert_newline(),
            KeyCode::Backspace => self.backspace(),
            _ => {}
        }
    }
}
