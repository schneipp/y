use crate::buffer::BufferId;
use crate::cursor::Cursor;

pub type ViewId = usize;

#[derive(Debug, Clone)]
pub struct CursorState {
    pub cursor: Cursor,
    pub visual_start: Option<(usize, usize)>,
}

#[derive(Debug, Clone)]
pub struct View {
    pub id: ViewId,
    pub buffer_id: BufferId,
    pub cursor_states: Vec<CursorState>,
    pub primary_cursor_idx: usize,
    pub scroll_offset: usize,
}

impl View {
    pub fn new(id: ViewId, buffer_id: BufferId) -> Self {
        Self {
            id,
            buffer_id,
            cursor_states: vec![CursorState {
                cursor: Cursor::new(),
                visual_start: None,
            }],
            primary_cursor_idx: 0,
            scroll_offset: 0,
        }
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor_states[self.primary_cursor_idx].cursor
    }

    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursor_states[self.primary_cursor_idx].cursor
    }

    pub fn visual_start(&self) -> Option<(usize, usize)> {
        self.cursor_states[self.primary_cursor_idx].visual_start
    }

    pub fn set_visual_start(&mut self, pos: Option<(usize, usize)>) {
        self.cursor_states[self.primary_cursor_idx].visual_start = pos;
    }

    pub fn add_cursor_above(&mut self) {
        let primary = &self.cursor_states[self.primary_cursor_idx];
        if primary.cursor.row > 0 {
            let mut new_cursor = primary.cursor.clone();
            new_cursor.row -= 1;
            self.cursor_states.push(CursorState {
                cursor: new_cursor,
                visual_start: None,
            });
            self.dedup_cursors();
        }
    }

    pub fn add_cursor_below(&mut self) {
        let primary = &self.cursor_states[self.primary_cursor_idx];
        let new_row = primary.cursor.row + 1;
        let mut new_cursor = primary.cursor.clone();
        new_cursor.row = new_row;
        self.cursor_states.push(CursorState {
            cursor: new_cursor,
            visual_start: None,
        });
        self.dedup_cursors();
    }

    pub fn add_cursor_at_next_match(&mut self, buffer: &crate::buffer::YBuffer) {
        let primary = &self.cursor_states[self.primary_cursor_idx].cursor;
        let row = primary.row;
        let col = primary.col;

        if row >= buffer.lines.len() {
            return;
        }

        // Get word under cursor
        let line = &buffer.lines[row].text;
        let chars: Vec<char> = line.chars().collect();
        if col >= chars.len() {
            return;
        }

        // Find word boundaries
        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        let mut end = col;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        if start == end {
            return;
        }

        let word: String = chars[start..end].iter().collect();

        // Search for next occurrence after the last cursor position
        let last_cursor = self.cursor_states.last().unwrap();
        let search_start_row = last_cursor.cursor.row;
        let search_start_col = last_cursor.cursor.col + 1;

        for r in search_start_row..buffer.lines.len() {
            let yline = &buffer.lines[r];
            let start_char = if r == search_start_row { search_start_col } else { 0 };
            // Convert char index to byte index for slicing
            let start_byte = yline.char_to_byte(start_char);
            if let Some(byte_pos) = yline.text[start_byte..].find(&word) {
                // Convert byte position back to character position
                let found_byte = start_byte + byte_pos;
                let found_col = yline.text[..found_byte].chars().count();
                // Don't add duplicate
                let already_exists = self.cursor_states.iter().any(|cs| {
                    cs.cursor.row == r && cs.cursor.col == found_col
                });
                if !already_exists {
                    self.cursor_states.push(CursorState {
                        cursor: Cursor {
                            row: r,
                            col: found_col,
                            desired_col: found_col,
                        },
                        visual_start: None,
                    });
                    self.dedup_cursors();
                    return;
                }
            }
        }
    }

    /// Get the selected text for a given cursor state from the buffer.
    /// Only works for single-line selections for now.
    pub fn get_selection_text(
        &self,
        cs_idx: usize,
        buffer: &crate::buffer::YBuffer,
    ) -> Option<String> {
        let cs = &self.cursor_states[cs_idx];
        let (start_row, start_col) = cs.visual_start?;
        let end_row = cs.cursor.row;
        let end_col = cs.cursor.col;

        // Normalize start/end
        let (s_row, s_col, e_row, e_col) =
            if (start_row, start_col) <= (end_row, end_col) {
                (start_row, start_col, end_row, end_col)
            } else {
                (end_row, end_col, start_row, start_col)
            };

        if s_row == e_row && s_row < buffer.lines.len() {
            let yline = &buffer.lines[s_row];
            let char_count = yline.char_count();
            let from = s_col.min(char_count);
            let to = (e_col + 1).min(char_count);
            if from < to {
                let selected: String = yline.text.chars().skip(from).take(to - from).collect();
                return Some(selected);
            }
        } else if s_row < buffer.lines.len() && e_row < buffer.lines.len() {
            // Multi-line selection
            let mut result = String::new();
            let first = &buffer.lines[s_row];
            let first_char_count = first.char_count();
            let first_selected: String = first.text.chars().skip(s_col.min(first_char_count)).collect();
            result.push_str(&first_selected);
            for r in (s_row + 1)..e_row {
                result.push('\n');
                result.push_str(&buffer.lines[r].text);
            }
            result.push('\n');
            let last = &buffer.lines[e_row];
            let last_char_count = last.char_count();
            let last_selected: String = last.text.chars().take((e_col + 1).min(last_char_count)).collect();
            result.push_str(&last_selected);
            return Some(result);
        }
        None
    }

    /// Add a new cursor at the next occurrence of `needle` in the buffer,
    /// with a visual selection covering the match. Used for Ctrl+N in visual mode.
    pub fn add_cursor_at_next_selection_match(
        &mut self,
        buffer: &crate::buffer::YBuffer,
        needle: &str,
    ) {
        if needle.is_empty() {
            return;
        }

        // Search after the last cursor's position
        let last = self.cursor_states.last().unwrap();
        let search_start_row = last.cursor.row;
        let search_start_col = last.cursor.col + 1;

        for r in search_start_row..buffer.lines.len() {
            let yline = &buffer.lines[r];
            let char_count = yline.char_count();
            let start_char = if r == search_start_row {
                search_start_col.min(char_count)
            } else {
                0
            };
            let start_byte = yline.char_to_byte(start_char);
            if let Some(byte_pos) = yline.text[start_byte..].find(needle) {
                let found_byte = start_byte + byte_pos;
                let found_col = yline.text[..found_byte].chars().count();
                let match_end = found_col + needle.chars().count() - 1;

                // Don't add duplicate
                let already_exists = self.cursor_states.iter().any(|cs| {
                    cs.cursor.row == r && cs.cursor.col == match_end
                        && cs.visual_start == Some((r, found_col))
                });
                if !already_exists {
                    self.cursor_states.push(CursorState {
                        cursor: Cursor {
                            row: r,
                            col: match_end,
                            desired_col: match_end,
                        },
                        visual_start: Some((r, found_col)),
                    });
                    return;
                }
            }
        }

        // Wrap around: search from the beginning
        let first_cursor = &self.cursor_states[0];
        let wrap_end_row = first_cursor
            .visual_start
            .map(|(r, _)| r)
            .unwrap_or(first_cursor.cursor.row);
        let wrap_end_col = first_cursor
            .visual_start
            .map(|(_, c)| c)
            .unwrap_or(first_cursor.cursor.col);

        for r in 0..=wrap_end_row.min(buffer.lines.len().saturating_sub(1)) {
            let yline = &buffer.lines[r];
            let mut search_from_byte = 0;
            while let Some(byte_pos) = yline.text[search_from_byte..].find(needle) {
                let found_byte = search_from_byte + byte_pos;
                let found_col = yline.text[..found_byte].chars().count();
                let match_end = found_col + needle.chars().count() - 1;

                // Stop if we've reached or passed the first cursor's position
                if r == wrap_end_row && found_col >= wrap_end_col {
                    return;
                }

                let already_exists = self.cursor_states.iter().any(|cs| {
                    cs.cursor.row == r && cs.cursor.col == match_end
                        && cs.visual_start == Some((r, found_col))
                });
                if !already_exists {
                    self.cursor_states.push(CursorState {
                        cursor: Cursor {
                            row: r,
                            col: match_end,
                            desired_col: match_end,
                        },
                        visual_start: Some((r, found_col)),
                    });
                    return;
                }
                // Move to next byte after the found position
                search_from_byte = found_byte + needle.len();
            }
        }
    }

    pub fn collapse_to_primary(&mut self) {
        let primary = self.cursor_states.remove(self.primary_cursor_idx);
        self.cursor_states = vec![primary];
        self.primary_cursor_idx = 0;
    }

    pub fn has_multiple_cursors(&self) -> bool {
        self.cursor_states.len() > 1
    }

    pub fn dedup_cursors(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let mut new_states = Vec::new();
        let mut new_primary = 0;

        for (i, cs) in self.cursor_states.iter().enumerate() {
            let key = (cs.cursor.row, cs.cursor.col);
            if seen.insert(key) {
                if i == self.primary_cursor_idx {
                    new_primary = new_states.len();
                }
                new_states.push(cs.clone());
            }
        }

        self.cursor_states = new_states;
        self.primary_cursor_idx = new_primary;
    }
}
