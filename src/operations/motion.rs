use crate::editor::Editor;

impl Editor {
    pub fn move_cursor_left(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.col > 0 {
                cs.cursor.col -= 1;
                cs.cursor.desired_col = cs.cursor.col;
            }
        }
    }

    pub fn move_cursor_right(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row < buffer.lines.len() {
                let line_len = buffer.lines[cs.cursor.row].text.len();
                if cs.cursor.col < line_len {
                    cs.cursor.col += 1;
                    cs.cursor.desired_col = cs.cursor.col;
                }
            }
        }
    }

    pub fn move_cursor_up(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row > 0 {
                cs.cursor.row -= 1;
                clamp_cursor_to_line(&mut cs.cursor, buffer);
            }
        }
    }

    pub fn move_cursor_down(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row < buffer.lines.len() - 1 {
                cs.cursor.row += 1;
                clamp_cursor_to_line(&mut cs.cursor, buffer);
            }
        }
    }

    pub fn clamp_cursor(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            clamp_cursor_to_line(&mut cs.cursor, buffer);
        }
    }

    pub fn move_word_forward(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row >= buffer.lines.len() {
                continue;
            }
            let line = &buffer.lines[cs.cursor.row].text;
            let chars: Vec<char> = line.chars().collect();

            while cs.cursor.col < chars.len() && !chars[cs.cursor.col].is_whitespace() {
                cs.cursor.col += 1;
            }
            while cs.cursor.col < chars.len() && chars[cs.cursor.col].is_whitespace() {
                cs.cursor.col += 1;
            }
            if cs.cursor.col >= chars.len() && cs.cursor.row < buffer.lines.len() - 1 {
                cs.cursor.row += 1;
                cs.cursor.col = 0;
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    pub fn move_word_backward(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row >= buffer.lines.len() {
                continue;
            }
            if cs.cursor.col == 0 {
                if cs.cursor.row > 0 {
                    cs.cursor.row -= 1;
                    cs.cursor.col = buffer.lines[cs.cursor.row].text.len();
                    if cs.cursor.col > 0 {
                        cs.cursor.col -= 1;
                    }
                }
                cs.cursor.desired_col = cs.cursor.col;
                continue;
            }

            let line = &buffer.lines[cs.cursor.row].text;
            let chars: Vec<char> = line.chars().collect();
            cs.cursor.col -= 1;
            while cs.cursor.col > 0 && chars[cs.cursor.col].is_whitespace() {
                cs.cursor.col -= 1;
            }
            while cs.cursor.col > 0 && !chars[cs.cursor.col - 1].is_whitespace() {
                cs.cursor.col -= 1;
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    #[allow(non_snake_case)]
    pub fn move_WORD_forward(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row >= buffer.lines.len() {
                continue;
            }
            let line = &buffer.lines[cs.cursor.row].text;
            let chars: Vec<char> = line.chars().collect();

            while cs.cursor.col < chars.len() && !chars[cs.cursor.col].is_whitespace() {
                cs.cursor.col += 1;
            }
            while cs.cursor.col < chars.len() && chars[cs.cursor.col].is_whitespace() {
                cs.cursor.col += 1;
            }
            if cs.cursor.col >= chars.len() && cs.cursor.row < buffer.lines.len() - 1 {
                cs.cursor.row += 1;
                cs.cursor.col = 0;
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    #[allow(non_snake_case)]
    pub fn move_WORD_backward(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row >= buffer.lines.len() {
                continue;
            }
            if cs.cursor.col == 0 {
                if cs.cursor.row > 0 {
                    cs.cursor.row -= 1;
                    cs.cursor.col = buffer.lines[cs.cursor.row].text.len();
                    if cs.cursor.col > 0 {
                        cs.cursor.col -= 1;
                    }
                }
                cs.cursor.desired_col = cs.cursor.col;
                continue;
            }

            let line = &buffer.lines[cs.cursor.row].text;
            let chars: Vec<char> = line.chars().collect();
            cs.cursor.col -= 1;
            while cs.cursor.col > 0 && chars[cs.cursor.col].is_whitespace() {
                cs.cursor.col -= 1;
            }
            while cs.cursor.col > 0 && !chars[cs.cursor.col - 1].is_whitespace() {
                cs.cursor.col -= 1;
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    pub fn move_to_line_start(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        for cs in view.cursor_states.iter_mut() {
            cs.cursor.col = 0;
            cs.cursor.desired_col = 0;
        }
    }

    pub fn move_to_line_end(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row < buffer.lines.len() {
                let line_len = buffer.lines[cs.cursor.row].text.len();
                cs.cursor.col = if line_len > 0 { line_len - 1 } else { 0 };
                cs.cursor.desired_col = cs.cursor.col;
            }
        }
    }

    pub fn goto_first_line(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        for cs in view.cursor_states.iter_mut() {
            cs.cursor.row = 0;
            cs.cursor.col = 0;
            cs.cursor.desired_col = 0;
        }
    }

    pub fn goto_last_line(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        if !buffer.lines.is_empty() {
            for cs in view.cursor_states.iter_mut() {
                cs.cursor.row = buffer.lines.len() - 1;
                cs.cursor.col = 0;
                cs.cursor.desired_col = 0;
            }
        }
    }

    pub fn find_char_forward(&mut self, target: char) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row < buffer.lines.len() {
                let line = &buffer.lines[cs.cursor.row].text;
                let chars: Vec<char> = line.chars().collect();
                for i in (cs.cursor.col + 1)..chars.len() {
                    if chars[i] == target {
                        cs.cursor.col = i;
                        cs.cursor.desired_col = i;
                        break;
                    }
                }
            }
        }
    }

    pub fn find_char_backward(&mut self, target: char) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        for cs in view.cursor_states.iter_mut() {
            if cs.cursor.row < buffer.lines.len() {
                let line = &buffer.lines[cs.cursor.row].text;
                let chars: Vec<char> = line.chars().collect();
                for i in (0..cs.cursor.col).rev() {
                    if chars[i] == target {
                        cs.cursor.col = i;
                        cs.cursor.desired_col = i;
                        break;
                    }
                }
            }
        }
    }
}

pub fn clamp_cursor_to_line(cursor: &mut crate::cursor::Cursor, buffer: &crate::buffer::YBuffer) {
    if cursor.row < buffer.lines.len() {
        let line_len = buffer.lines[cursor.row].text.len();
        cursor.col = cursor.desired_col.min(line_len);
    }
}
