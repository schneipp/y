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
        while start > 0 && chars[start - 1].is_alphanumeric() || (start > 0 && chars[start - 1] == '_') {
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
            let search_line = &buffer.lines[r].text;
            let start_col = if r == search_start_row { search_start_col } else { 0 };
            if let Some(pos) = search_line[start_col..].find(&word) {
                let found_col = start_col + pos;
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
