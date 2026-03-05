use crate::editor::Editor;

impl Editor {
    pub fn save_state(&mut self) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        self.undo_stack.push(buffer.clone());
        self.redo_stack.clear();
        self.modified = true;
    }

    pub fn undo(&mut self) {
        if let Some(previous_buffer) = self.undo_stack.pop() {
            let view = &mut self.views[self.active_view_idx];
            let buffer = self.buffer_pool.get_mut(view.buffer_id);
            self.redo_stack.push(buffer.clone());
            *buffer = previous_buffer;

            let cs = &mut view.cursor_states[view.primary_cursor_idx];
            if cs.cursor.row >= buffer.lines.len() {
                cs.cursor.row = buffer.lines.len().saturating_sub(1);
            }
            if cs.cursor.row < buffer.lines.len() {
                let line_char_count = buffer.lines[cs.cursor.row].char_count();
                if cs.cursor.col > line_char_count {
                    cs.cursor.col = line_char_count;
                }
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next_buffer) = self.redo_stack.pop() {
            let view = &mut self.views[self.active_view_idx];
            let buffer = self.buffer_pool.get_mut(view.buffer_id);
            self.undo_stack.push(buffer.clone());
            *buffer = next_buffer;

            let cs = &mut view.cursor_states[view.primary_cursor_idx];
            if cs.cursor.row >= buffer.lines.len() {
                cs.cursor.row = buffer.lines.len().saturating_sub(1);
            }
            if cs.cursor.row < buffer.lines.len() {
                let line_char_count = buffer.lines[cs.cursor.row].char_count();
                if cs.cursor.col > line_char_count {
                    cs.cursor.col = line_char_count;
                }
            }
            cs.cursor.desired_col = cs.cursor.col;
        }
    }
}
