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
                line.insert_char_at(cs.cursor.col, c);
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
            let line = &buffer.lines[cs.cursor.row];
            let byte_idx = line.char_to_byte(cs.cursor.col);
            let current_line = &line.text;
            let before = current_line[..byte_idx].to_string();
            let after = current_line[byte_idx..].to_string();

            // Compute indentation
            let base_indent = leading_whitespace(&before).to_string();
            let indent_unit = detect_indent_unit(buffer).to_string();

            let before_trimmed_end = before.trim_end();
            let after_trimmed_start = after.trim_start();

            let opens = before_trimmed_end
                .chars()
                .last()
                .map(|c| matches!(c, '{' | '(' | '[' | ':'))
                .unwrap_or(false);
            let closes = after_trimmed_start
                .chars()
                .next()
                .map(|c| matches!(c, '}' | ')' | ']'))
                .unwrap_or(false);

            if opens && closes {
                // e.g. {|} → split into 3 lines
                let new_indent = format!("{}{}", base_indent, indent_unit);
                buffer.lines[cs.cursor.row].text = before;
                buffer.lines.insert(cs.cursor.row + 1, YLine::from(new_indent.clone()));
                buffer.lines.insert(
                    cs.cursor.row + 2,
                    YLine::from(format!("{}{}", base_indent, after.trim_start())),
                );
                cs.cursor.row += 1;
                cs.cursor.col = new_indent.len();
            } else if opens {
                // e.g. { at end → indent one level
                let new_indent = format!("{}{}", base_indent, indent_unit);
                buffer.lines[cs.cursor.row].text = before;
                buffer.lines.insert(
                    cs.cursor.row + 1,
                    YLine::from(format!("{}{}", new_indent, after.trim_start())),
                );
                cs.cursor.row += 1;
                cs.cursor.col = new_indent.len();
            } else {
                // Normal: preserve indentation
                buffer.lines[cs.cursor.row].text = before;
                buffer.lines.insert(
                    cs.cursor.row + 1,
                    YLine::from(format!("{}{}", base_indent, after.trim_start())),
                );
                cs.cursor.row += 1;
                cs.cursor.col = base_indent.len();
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    pub fn backspace(&mut self) {
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
            let row = view.cursor_states[i].cursor.row;
            let col = view.cursor_states[i].cursor.col;
            if col > 0 {
                if row < buffer.lines.len() {
                    buffer.lines[row].remove_char_at(col - 1);
                    view.cursor_states[i].cursor.col = col - 1;
                    view.cursor_states[i].cursor.desired_col = col - 1;
                }
            } else if row > 0 {
                let current_line = buffer.lines.remove(row);
                let new_col = buffer.lines[row - 1].char_count();
                buffer.lines[row - 1].text.push_str(&current_line.text);
                view.cursor_states[i].cursor.row = row - 1;
                view.cursor_states[i].cursor.col = new_col;
                view.cursor_states[i].cursor.desired_col = new_col;
                // Adjust other cursors on lines below the removed line
                for j in 0..view.cursor_states.len() {
                    if j != i && view.cursor_states[j].cursor.row > row - 1 {
                        view.cursor_states[j].cursor.row -= 1;
                    }
                }
            }
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
                let char_count = line.char_count();
                if cs.cursor.col < char_count {
                    line.remove_char_at(cs.cursor.col);
                    let new_char_count = line.char_count();
                    if cs.cursor.col >= new_char_count && new_char_count > 0 {
                        cs.cursor.col = new_char_count - 1;
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

            let new_char_count = buffer.lines[cs.cursor.row].char_count();
            if cs.cursor.col >= new_char_count && new_char_count > 0 {
                cs.cursor.col = new_char_count - 1;
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
            line.truncate_at_char(cs.cursor.col);

            let new_char_count = line.char_count();
            if cs.cursor.col > 0 && cs.cursor.col >= new_char_count {
                cs.cursor.col = new_char_count.saturating_sub(1);
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
        // Collect all selections (from all cursor states)
        let view = &self.views[self.active_view_idx];
        let is_visual_line = self.mode == crate::mode::Mode::VisualLine;

        let mut selections: Vec<((usize, usize), (usize, usize))> = view
            .cursor_states
            .iter()
            .filter_map(|cs| {
                let (sr, sc) = cs.visual_start?;
                let (er, ec) = (cs.cursor.row, cs.cursor.col);
                // Normalize so start <= end
                if (sr, sc) <= (er, ec) {
                    Some(((sr, sc), (er, ec)))
                } else {
                    Some(((er, ec), (sr, sc)))
                }
            })
            .collect();

        if selections.is_empty() {
            return;
        }

        self.save_state();

        // Sort in reverse order (bottom-to-top, right-to-left) for safe deletion
        selections.sort_by(|a, b| (b.0).cmp(&(a.0)));

        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);

        // Track where the primary cursor should end up
        let mut final_row = selections.last().unwrap().0 .0;
        let mut final_col = selections.last().unwrap().0 .1;

        for (start_pos, end_pos) in &selections {
            if is_visual_line {
                for _ in start_pos.0..=end_pos.0.min(buffer.lines.len().saturating_sub(1)) {
                    if start_pos.0 < buffer.lines.len() {
                        buffer.lines.remove(start_pos.0);
                    }
                }
            } else if start_pos.0 == end_pos.0 {
                // Single-line selection
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
                }
            } else {
                // Multi-line selection
                let first_line_text = if start_pos.0 < buffer.lines.len() {
                    buffer.lines[start_pos.0]
                        .text
                        .chars()
                        .take(start_pos.1)
                        .collect::<String>()
                } else {
                    String::new()
                };
                let last_line_text = if end_pos.0 < buffer.lines.len() {
                    buffer.lines[end_pos.0]
                        .text
                        .chars()
                        .skip(end_pos.1 + 1)
                        .collect::<String>()
                } else {
                    String::new()
                };
                for _ in start_pos.0..=end_pos.0.min(buffer.lines.len().saturating_sub(1)) {
                    if start_pos.0 < buffer.lines.len() {
                        buffer.lines.remove(start_pos.0);
                    }
                }
                let combined = format!("{}{}", first_line_text, last_line_text);
                buffer.lines.insert(start_pos.0, YLine::from(combined));
            }
        }

        if buffer.lines.is_empty() {
            buffer.lines.push(YLine::new());
        }

        final_row = final_row.min(buffer.lines.len().saturating_sub(1));
        if is_visual_line {
            final_col = 0;
        } else {
            final_col = final_col.min(
                buffer.lines[final_row]
                    .char_count()
                    .saturating_sub(1),
            );
        }

        // Collapse to single cursor at the topmost deletion point
        view.cursor_states = vec![crate::view::CursorState {
            cursor: crate::cursor::Cursor {
                row: final_row,
                col: final_col,
                desired_col: final_col,
            },
            visual_start: None,
        }];
        view.primary_cursor_idx = 0;

        self.enter_normal_mode();
    }

    /// Delete visual selections across all cursors and enter insert mode (multi-cursor `c`).
    pub fn change_visual_selection(&mut self) {
        let view = &self.views[self.active_view_idx];
        let is_visual_line = self.mode == crate::mode::Mode::VisualLine;

        // Collect all single-line selections, sorted bottom-to-top
        let mut selections: Vec<(usize, (usize, usize), (usize, usize))> = view
            .cursor_states
            .iter()
            .enumerate()
            .filter_map(|(idx, cs)| {
                let (sr, sc) = cs.visual_start?;
                let (er, ec) = (cs.cursor.row, cs.cursor.col);
                let ((sr2, sc2), (er2, ec2)) = if (sr, sc) <= (er, ec) {
                    ((sr, sc), (er, ec))
                } else {
                    ((er, ec), (sr, sc))
                };
                Some((idx, (sr2, sc2), (er2, ec2)))
            })
            .collect();

        if selections.is_empty() {
            return;
        }

        self.save_state();

        // Sort bottom-to-top for safe deletion
        selections.sort_by(|a, b| (b.1).cmp(&(a.1)));

        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);

        // Delete each selection and record the new cursor position
        let mut new_cursors: Vec<(usize, usize)> = Vec::new();
        for (_, start_pos, end_pos) in &selections {
            if is_visual_line {
                for _ in start_pos.0..=end_pos.0.min(buffer.lines.len().saturating_sub(1)) {
                    if start_pos.0 < buffer.lines.len() {
                        buffer.lines.remove(start_pos.0);
                    }
                }
                // Insert an empty line for typing
                buffer.lines.insert(start_pos.0, YLine::new());
                new_cursors.push((start_pos.0, 0));
            } else if start_pos.0 == end_pos.0 {
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
                }
                new_cursors.push((start_pos.0, start_pos.1));
            } else {
                let first_text = if start_pos.0 < buffer.lines.len() {
                    buffer.lines[start_pos.0]
                        .text
                        .chars()
                        .take(start_pos.1)
                        .collect::<String>()
                } else {
                    String::new()
                };
                let last_text = if end_pos.0 < buffer.lines.len() {
                    buffer.lines[end_pos.0]
                        .text
                        .chars()
                        .skip(end_pos.1 + 1)
                        .collect::<String>()
                } else {
                    String::new()
                };
                for _ in start_pos.0..=end_pos.0.min(buffer.lines.len().saturating_sub(1)) {
                    if start_pos.0 < buffer.lines.len() {
                        buffer.lines.remove(start_pos.0);
                    }
                }
                buffer.lines.insert(start_pos.0, YLine::from(format!("{}{}", first_text, last_text)));
                new_cursors.push((start_pos.0, start_pos.1));
            }
        }

        if buffer.lines.is_empty() {
            buffer.lines.push(YLine::new());
        }

        // Reverse so cursors are top-to-bottom
        new_cursors.reverse();

        // Set cursor states for multi-cursor insert mode
        view.cursor_states = new_cursors
            .iter()
            .map(|&(r, c)| crate::view::CursorState {
                cursor: crate::cursor::Cursor {
                    row: r.min(buffer.lines.len().saturating_sub(1)),
                    col: c,
                    desired_col: c,
                },
                visual_start: None,
            })
            .collect();
        view.primary_cursor_idx = 0;

        self.mode = crate::mode::Mode::Insert;
    }

    /// Move cursors to end of each visual selection and enter insert mode (multi-cursor `a`).
    pub fn append_after_visual_selection(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);

        // Position each cursor at the end of its selection (one past, for insert/append)
        for cs in &mut view.cursor_states {
            if let Some((sr, sc)) = cs.visual_start {
                let (er, ec) = (cs.cursor.row, cs.cursor.col);
                // Find the end of the selection
                let (end_row, end_col) = if (sr, sc) > (er, ec) {
                    (sr, sc)
                } else {
                    (er, ec)
                };
                let line_char_count = if end_row < buffer.lines.len() {
                    buffer.lines[end_row].char_count()
                } else {
                    0
                };
                cs.cursor.row = end_row;
                // Place cursor one past the end of selection for appending
                cs.cursor.col = (end_col + 1).min(line_char_count);
                cs.cursor.desired_col = cs.cursor.col;
            }
            cs.visual_start = None;
        }
        view.primary_cursor_idx = 0;

        self.mode = crate::mode::Mode::Insert;
    }

    pub fn append(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row < buffer.lines.len() {
            let line_char_count = buffer.lines[cs.cursor.row].char_count();
            if cs.cursor.col < line_char_count {
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
            let current_line = &buffer.lines[cs.cursor.row].text;
            let base_indent = leading_whitespace(current_line);
            let indent_unit = detect_indent_unit(buffer);

            // If the line ends with an opener, indent one level deeper
            let trimmed = current_line.trim_end();
            let opens = trimmed
                .chars()
                .last()
                .map(|c| matches!(c, '{' | '(' | '[' | ':'))
                .unwrap_or(false);

            let new_indent = if opens {
                format!("{}{}", base_indent, indent_unit)
            } else {
                base_indent.to_string()
            };

            buffer.lines.insert(cs.cursor.row + 1, YLine::from(new_indent.clone()));
            cs.cursor.row += 1;
            cs.cursor.col = new_indent.len();
            cs.cursor.desired_col = cs.cursor.col;
            self.mode = crate::mode::Mode::Insert;
        }
    }

    pub fn open_line_above(&mut self) {
        self.save_state();
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get_mut(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        if cs.cursor.row < buffer.lines.len() {
            let base_indent = leading_whitespace(&buffer.lines[cs.cursor.row].text).to_string();

            buffer.lines.insert(cs.cursor.row, YLine::from(base_indent.clone()));
            cs.cursor.col = base_indent.len();
            cs.cursor.desired_col = cs.cursor.col;
            self.mode = crate::mode::Mode::Insert;
        } else {
            buffer.lines.insert(cs.cursor.row, YLine::new());
            cs.cursor.col = 0;
            cs.cursor.desired_col = 0;
            self.mode = crate::mode::Mode::Insert;
        }
    }
}

/// Extract leading whitespace from a line.
fn leading_whitespace(line: &str) -> &str {
    let trimmed = line.trim_start();
    &line[..line.len() - trimmed.len()]
}

/// Detect the indent unit used in the buffer. Looks at existing lines to figure out
/// if the file uses tabs or spaces, and how many spaces per level. Defaults to 4 spaces.
fn detect_indent_unit(buffer: &crate::buffer::YBuffer) -> &'static str {
    let mut min_spaces: usize = usize::MAX;
    for line in &buffer.lines {
        let text = &line.text;
        if text.starts_with('\t') {
            return "\t";
        }
        let spaces = text.len() - text.trim_start_matches(' ').len();
        if spaces >= 2 && spaces < min_spaces {
            min_spaces = spaces;
        }
    }
    if min_spaces == usize::MAX {
        return "    ";
    }
    match min_spaces {
        2 => "  ",
        3 => "   ",
        _ => "    ",
    }
}
