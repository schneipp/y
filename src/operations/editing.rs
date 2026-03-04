use crate::buffer::YLine;
use crate::editor::Editor;

impl Editor {
    pub fn insert_char(&mut self, c: char) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);

        // Process cursors bottom-to-top for multi-cursor correctness
        let mut indices: Vec<usize> = (0..view.cursor_states.len()).collect();
        indices.sort_by(|a, b| {
            let ca = &view.cursor_states[*a].cursor;
            let cb = &view.cursor_states[*b].cursor;
            (cb.row, cb.col).cmp(&(ca.row, ca.col))
        });

        for i in indices {
            let cs = &mut view.cursor_states[i];
            if cs.cursor.row < buffer.lines.len() {
                let line = &mut buffer.lines[cs.cursor.row];
                line.text.insert(cs.cursor.col, c);
                cs.cursor.col += 1;
                cs.cursor.desired_col = cs.cursor.col;
            }
        }
    }

    pub fn insert_newline(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row < buffer.lines.len() {
            let current_line = &buffer.lines[cs.cursor.row].text;
            let before = current_line[..cs.cursor.col].to_string();
            let after = current_line[cs.cursor.col..].to_string();

            buffer.lines[cs.cursor.row].text = before;
            buffer.lines.insert(cs.cursor.row + 1, YLine::from(after));

            cs.cursor.row += 1;
            cs.cursor.col = 0;
            cs.cursor.desired_col = 0;
        }
    }

    pub fn backspace(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.col > 0 {
            if cs.cursor.row < buffer.lines.len() {
                buffer.lines[cs.cursor.row].text.remove(cs.cursor.col - 1);
                cs.cursor.col -= 1;
                cs.cursor.desired_col = cs.cursor.col;
            }
        } else if cs.cursor.row > 0 {
            let current_line = buffer.lines.remove(cs.cursor.row);
            cs.cursor.row -= 1;
            cs.cursor.col = buffer.lines[cs.cursor.row].text.len();
            buffer.lines[cs.cursor.row].text.push_str(&current_line.text);
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    pub fn delete_char(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);

        let mut indices: Vec<usize> = (0..view.cursor_states.len()).collect();
        indices.sort_by(|a, b| {
            let ca = &view.cursor_states[*a].cursor;
            let cb = &view.cursor_states[*b].cursor;
            (cb.row, cb.col).cmp(&(ca.row, ca.col))
        });

        for i in indices {
            let cs = &mut view.cursor_states[i];
            if cs.cursor.row < buffer.lines.len() {
                let line = &mut buffer.lines[cs.cursor.row];
                if cs.cursor.col < line.text.len() {
                    line.text.remove(cs.cursor.col);
                    if cs.cursor.col >= line.text.len() && line.text.len() > 0 {
                        cs.cursor.col = line.text.len() - 1;
                    } else if line.text.is_empty() {
                        cs.cursor.col = 0;
                    }
                    cs.cursor.desired_col = cs.cursor.col;
                }
            }
        }
    }

    pub fn delete_line(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row < buffer.lines.len() {
            buffer.lines.remove(cs.cursor.row);
            if buffer.lines.is_empty() {
                buffer.lines.push(YLine::new());
            }
            if cs.cursor.row >= buffer.lines.len() {
                cs.cursor.row = buffer.lines.len() - 1;
            }
            cs.cursor.col = 0;
            cs.cursor.desired_col = 0;
        }
    }

    pub fn delete_word(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row >= buffer.lines.len() {
            return;
        }

        let line = &buffer.lines[cs.cursor.row].text;
        let chars: Vec<char> = line.chars().collect();
        let start = cs.cursor.col;
        let mut end = start;

        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        while end < chars.len() && chars[end].is_whitespace() {
            end += 1;
        }

        if start < chars.len() {
            let mut new_text = String::new();
            for (i, ch) in chars.iter().enumerate() {
                if i < start || i >= end {
                    new_text.push(*ch);
                }
            }
            buffer.lines[cs.cursor.row].text = new_text;

            if cs.cursor.col >= buffer.lines[cs.cursor.row].text.len()
                && buffer.lines[cs.cursor.row].text.len() > 0
            {
                cs.cursor.col = buffer.lines[cs.cursor.row].text.len() - 1;
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    pub fn delete_to_line_end(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row < buffer.lines.len() {
            let line = &mut buffer.lines[cs.cursor.row];
            line.text.truncate(cs.cursor.col);

            if cs.cursor.col > 0 && cs.cursor.col >= line.text.len() {
                cs.cursor.col = line.text.len().saturating_sub(1);
            } else if line.text.is_empty() {
                cs.cursor.col = 0;
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    pub fn delete_to_line_start(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row < buffer.lines.len() {
            let remaining: String = buffer.lines[cs.cursor.row].text.chars().skip(cs.cursor.col).collect();
            buffer.lines[cs.cursor.row].text = remaining;
            cs.cursor.col = 0;
            cs.cursor.desired_col = 0;
        }
    }

    pub fn delete_visual_selection(&mut self) {
        let view = &self.views[self.active_view_idx];
        let visual_start = view.visual_start();
        if let Some((start_row, start_col)) = visual_start {
            self.save_state();

            let view = &mut self.views[self.active_view_idx];
            let buffer = self.buffer_pool.get_mut(view.buffer_id);
            let cs = &mut view.cursor_states[view.primary_cursor_idx];

            let end_row = cs.cursor.row;
            let end_col = cs.cursor.col;

            if self.mode == crate::mode::Mode::VisualLine {
                let (first_line, last_line) = if start_row <= end_row {
                    (start_row, end_row)
                } else {
                    (end_row, start_row)
                };

                for _ in first_line..=last_line {
                    if first_line < buffer.lines.len() {
                        buffer.lines.remove(first_line);
                    }
                }

                if buffer.lines.is_empty() {
                    buffer.lines.push(YLine::new());
                }

                cs.cursor.row = first_line.min(buffer.lines.len() - 1);
                cs.cursor.col = 0;
                cs.cursor.desired_col = 0;
            } else {
                let (start_pos, end_pos) = if (start_row, start_col) <= (end_row, end_col) {
                    ((start_row, start_col), (end_row, end_col))
                } else {
                    ((end_row, end_col), (start_row, start_col))
                };

                if start_pos.0 == end_pos.0 {
                    if start_pos.0 < buffer.lines.len() {
                        let line = &mut buffer.lines[start_pos.0];
                        let chars: Vec<char> = line.text.chars().collect();
                        let mut new_text = String::new();
                        for (i, ch) in chars.iter().enumerate() {
                            if i < start_pos.1 || i > end_pos.1 {
                                new_text.push(*ch);
                            }
                        }
                        line.text = new_text;
                        cs.cursor.row = start_pos.0;
                        cs.cursor.col = start_pos.1;
                    }
                } else {
                    let first_line_text = if start_pos.0 < buffer.lines.len() {
                        buffer.lines[start_pos.0].text.chars().take(start_pos.1).collect::<String>()
                    } else {
                        String::new()
                    };

                    let last_line_text = if end_pos.0 < buffer.lines.len() {
                        buffer.lines[end_pos.0].text.chars().skip(end_pos.1 + 1).collect::<String>()
                    } else {
                        String::new()
                    };

                    for _ in start_pos.0..=end_pos.0.min(buffer.lines.len() - 1) {
                        if start_pos.0 < buffer.lines.len() {
                            buffer.lines.remove(start_pos.0);
                        }
                    }

                    let combined = format!("{}{}", first_line_text, last_line_text);
                    buffer.lines.insert(start_pos.0, YLine::from(combined));

                    cs.cursor.row = start_pos.0;
                    cs.cursor.col = start_pos.1;
                }

                cs.cursor.desired_col = cs.cursor.col;
            }

            self.enter_normal_mode();
        }
    }

    pub fn append(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row < buffer.lines.len() {
            let line_len = buffer.lines[cs.cursor.row].text.len();
            if cs.cursor.col < line_len {
                cs.cursor.col += 1;
                cs.cursor.desired_col = cs.cursor.col;
            }
        }
        self.mode = crate::mode::Mode::Insert;
    }

    pub fn open_line_below(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row < buffer.lines.len() {
            buffer.lines.insert(cs.cursor.row + 1, YLine::new());
            cs.cursor.row += 1;
            cs.cursor.col = 0;
            cs.cursor.desired_col = 0;
            self.mode = crate::mode::Mode::Insert;
        }
    }

    pub fn open_line_above(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        buffer.lines.insert(cs.cursor.row, YLine::new());
        cs.cursor.col = 0;
        cs.cursor.desired_col = 0;
        self.mode = crate::mode::Mode::Insert;
    }
}
