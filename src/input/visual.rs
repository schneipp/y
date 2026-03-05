use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::Editor;

impl Editor {
    pub fn handle_visual_mode(&mut self, key_event: KeyEvent) {
        // Ctrl+N: add cursor at next match of the current selection (VSCode Ctrl+D)
        if key_event.code == KeyCode::Char('n')
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.select_word_or_next_match();
            return;
        }

        match key_event.code {
            KeyCode::Esc => self.enter_normal_mode(),
            KeyCode::Char('h') => self.move_cursor_left(),
            KeyCode::Char('j') => self.move_cursor_down(),
            KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Char('l') => self.move_cursor_right(),
            KeyCode::Char('w') => self.move_word_forward(),
            KeyCode::Char('W') => self.move_WORD_forward(),
            KeyCode::Char('b') => self.move_word_backward(),
            KeyCode::Char('B') => self.move_WORD_backward(),
            KeyCode::Char('0') => self.move_to_line_start(),
            KeyCode::Char('$') => self.move_to_line_end(),
            KeyCode::Char('%') => self.goto_matching_bracket(),
            KeyCode::Char('G') => self.goto_last_line(),
            KeyCode::Char('d') | KeyCode::Char('x') => self.delete_visual_selection(),
            KeyCode::Char('c') | KeyCode::Char('s') => self.change_visual_selection(),
            KeyCode::Char('a') => self.append_after_visual_selection(),
            KeyCode::Char('y') => self.yank_visual_selection(),
            KeyCode::Char('n') => self.search_next(),
            KeyCode::Char('N') => self.search_prev(),
            KeyCode::Char('V') => self.enter_visual_line_mode(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Up => self.move_cursor_up(),
            KeyCode::Right => self.move_cursor_right(),
            _ => {}
        }
    }

    pub fn handle_visual_line_mode(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.enter_normal_mode(),
            KeyCode::Char('j') => self.move_cursor_down(),
            KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Char('G') => self.goto_last_line(),
            KeyCode::Char('d') | KeyCode::Char('x') => self.delete_visual_selection(),
            KeyCode::Char('y') => self.yank_visual_selection(),
            KeyCode::Char('v') => self.enter_visual_mode(),
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Up => self.move_cursor_up(),
            _ => {}
        }
    }
}
