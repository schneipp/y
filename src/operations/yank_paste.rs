use crate::buffer::YLine;
use crate::editor::Editor;
use crate::mode::{Mode, YankRegister, YankType};

impl Editor {
    pub fn yank_line(&mut self) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cursor = view.cursor();

        if cursor.row < buffer.lines.len() {
            let line_text = buffer.lines[cursor.row].text.clone();
            self.yank_register = Some(YankRegister {
                text: vec![line_text],
                yank_type: YankType::Line,
            });
        }
    }

    pub fn yank_word(&mut self) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cursor = view.cursor();

        if cursor.row >= buffer.lines.len() {
            return;
        }

        let line = &buffer.lines[cursor.row].text;
        let chars: Vec<char> = line.chars().collect();
        let start = cursor.col;
        let mut end = start;

        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        while end < chars.len() && chars[end].is_whitespace() {
            end += 1;
        }

        if start < chars.len() {
            let yanked_text: String = chars[start..end].iter().collect();
            self.yank_register = Some(YankRegister {
                text: vec![yanked_text],
                yank_type: YankType::Character,
            });
        }
    }

    pub fn yank_to_line_end(&mut self) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cursor = view.cursor();

        if cursor.row < buffer.lines.len() {
            let line = &buffer.lines[cursor.row];
            let yanked_text: String = line.text.chars().skip(cursor.col).collect();
            self.yank_register = Some(YankRegister {
                text: vec![yanked_text],
                yank_type: YankType::Character,
            });
        }
    }

    pub fn yank_to_line_start(&mut self) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cursor = view.cursor();

        if cursor.row < buffer.lines.len() {
            let line = &buffer.lines[cursor.row];
            let yanked_text: String = line.text.chars().take(cursor.col).collect();
            self.yank_register = Some(YankRegister {
                text: vec![yanked_text],
                yank_type: YankType::Character,
            });
        }
    }

    pub fn yank_visual_selection(&mut self) {
        let view = &self.views[self.active_view_idx];
        let visual_start = view.visual_start();
        if let Some((start_row, start_col)) = visual_start {
            let buffer = self.buffer_pool.get(view.buffer_id);
            let cursor = view.cursor();
            let end_row = cursor.row;
            let end_col = cursor.col;

            if self.mode == Mode::VisualLine {
                let (first_line, last_line) = if start_row <= end_row {
                    (start_row, end_row)
                } else {
                    (end_row, start_row)
                };

                let mut yanked_lines = Vec::new();
                for row in first_line..=last_line {
                    if row < buffer.lines.len() {
                        yanked_lines.push(buffer.lines[row].text.clone());
                    }
                }

                self.yank_register = Some(YankRegister {
                    text: yanked_lines,
                    yank_type: YankType::Line,
                });
            } else {
                let (start_pos, end_pos) = if (start_row, start_col) <= (end_row, end_col) {
                    ((start_row, start_col), (end_row, end_col))
                } else {
                    ((end_row, end_col), (start_row, start_col))
                };

                if start_pos.0 == end_pos.0 {
                    if start_pos.0 < buffer.lines.len() {
                        let line = &buffer.lines[start_pos.0];
                        let chars: Vec<char> = line.text.chars().collect();
                        let yanked_text: String = chars
                            [start_pos.1..=end_pos.1.min(chars.len().saturating_sub(1))]
                            .iter()
                            .collect();
                        self.yank_register = Some(YankRegister {
                            text: vec![yanked_text],
                            yank_type: YankType::Character,
                        });
                    }
                } else {
                    let mut yanked_lines = Vec::new();

                    if start_pos.0 < buffer.lines.len() {
                        let first_line_text: String =
                            buffer.lines[start_pos.0].text.chars().skip(start_pos.1).collect();
                        yanked_lines.push(first_line_text);
                    }

                    for row in (start_pos.0 + 1)..end_pos.0 {
                        if row < buffer.lines.len() {
                            yanked_lines.push(buffer.lines[row].text.clone());
                        }
                    }

                    if end_pos.0 < buffer.lines.len() && end_pos.0 != start_pos.0 {
                        let last_line_text: String = buffer.lines[end_pos.0]
                            .text
                            .chars()
                            .take(end_pos.1 + 1)
                            .collect();
                        yanked_lines.push(last_line_text);
                    }

                    self.yank_register = Some(YankRegister {
                        text: yanked_lines,
                        yank_type: YankType::Character,
                    });
                }
            }

            self.enter_normal_mode();
        }
    }

    pub fn paste_after(&mut self) {
        if let Some(register) = self.yank_register.clone() {
            self.save_state();
            let view = &mut self.views[self.active_view_idx];
            let buffer = self.buffer_pool.get_mut(view.buffer_id);
            let cs = &mut view.cursor_states[view.primary_cursor_idx];

            match register.yank_type {
                YankType::Line => {
                    let insert_row = cs.cursor.row + 1;
                    for (i, line_text) in register.text.iter().enumerate() {
                        buffer
                            .lines
                            .insert(insert_row + i, YLine::from(line_text.clone()));
                    }
                    if insert_row < buffer.lines.len() {
                        cs.cursor.row = insert_row;
                        cs.cursor.col = 0;
                        cs.cursor.desired_col = 0;
                    }
                }
                YankType::Character => {
                    if cs.cursor.row < buffer.lines.len() {
                        if register.text.len() == 1 {
                            let line = &mut buffer.lines[cs.cursor.row];
                            let insert_pos = (cs.cursor.col + 1).min(line.text.len());
                            line.text.insert_str(insert_pos, &register.text[0]);
                            cs.cursor.col = insert_pos;
                            cs.cursor.desired_col = cs.cursor.col;
                        } else {
                            let current_line = &buffer.lines[cs.cursor.row].text;
                            let before =
                                current_line.chars().take(cs.cursor.col + 1).collect::<String>();
                            let after =
                                current_line.chars().skip(cs.cursor.col + 1).collect::<String>();

                            buffer.lines[cs.cursor.row].text =
                                format!("{}{}", before, register.text[0]);

                            for i in 1..(register.text.len() - 1) {
                                buffer.lines.insert(
                                    cs.cursor.row + i,
                                    YLine::from(register.text[i].clone()),
                                );
                            }

                            if register.text.len() > 1 {
                                let last_yanked = &register.text[register.text.len() - 1];
                                buffer.lines.insert(
                                    cs.cursor.row + register.text.len() - 1,
                                    YLine::from(format!("{}{}", last_yanked, after)),
                                );
                            }

                            cs.cursor.row += register.text.len() - 1;
                            cs.cursor.col = register.text[register.text.len() - 1].len();
                            cs.cursor.desired_col = cs.cursor.col;
                        }
                    }
                }
            }
        }
    }

    pub fn paste_before(&mut self) {
        if let Some(register) = self.yank_register.clone() {
            self.save_state();
            let view = &mut self.views[self.active_view_idx];
            let buffer = self.buffer_pool.get_mut(view.buffer_id);
            let cs = &mut view.cursor_states[view.primary_cursor_idx];

            match register.yank_type {
                YankType::Line => {
                    let insert_row = cs.cursor.row;
                    for (i, line_text) in register.text.iter().enumerate() {
                        buffer
                            .lines
                            .insert(insert_row + i, YLine::from(line_text.clone()));
                    }
                    cs.cursor.row = insert_row;
                    cs.cursor.col = 0;
                    cs.cursor.desired_col = 0;
                }
                YankType::Character => {
                    if cs.cursor.row < buffer.lines.len() {
                        if register.text.len() == 1 {
                            let line = &mut buffer.lines[cs.cursor.row];
                            line.text.insert_str(cs.cursor.col, &register.text[0]);
                            cs.cursor.desired_col = cs.cursor.col;
                        } else {
                            let current_line = &buffer.lines[cs.cursor.row].text;
                            let before =
                                current_line.chars().take(cs.cursor.col).collect::<String>();
                            let after =
                                current_line.chars().skip(cs.cursor.col).collect::<String>();

                            buffer.lines[cs.cursor.row].text =
                                format!("{}{}", before, register.text[0]);

                            for i in 1..(register.text.len() - 1) {
                                buffer.lines.insert(
                                    cs.cursor.row + i,
                                    YLine::from(register.text[i].clone()),
                                );
                            }

                            if register.text.len() > 1 {
                                let last_yanked = &register.text[register.text.len() - 1];
                                buffer.lines.insert(
                                    cs.cursor.row + register.text.len() - 1,
                                    YLine::from(format!("{}{}", last_yanked, after)),
                                );
                            }

                            cs.cursor.row += register.text.len() - 1;
                            cs.cursor.col = register.text[register.text.len() - 1].len();
                            cs.cursor.desired_col = cs.cursor.col;
                        }
                    }
                }
            }
        }
    }
}
