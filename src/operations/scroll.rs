use crate::editor::Editor;

impl Editor {
    pub fn page_down(&mut self) {
        let viewport_height = 30;
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        let new_row = (cs.cursor.row + viewport_height).min(buffer.lines.len().saturating_sub(1));
        cs.cursor.row = new_row;
        let col = first_non_ws(&buffer.lines[new_row].text);
        cs.cursor.col = col;
        cs.cursor.desired_col = col;
        view.scroll_offset = (view.scroll_offset + viewport_height)
            .min(buffer.lines.len().saturating_sub(viewport_height));
    }

    pub fn page_up(&mut self) {
        let viewport_height = 30;
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        let new_row = cs.cursor.row.saturating_sub(viewport_height);
        cs.cursor.row = new_row;
        let col = first_non_ws(&buffer.lines[new_row].text);
        cs.cursor.col = col;
        cs.cursor.desired_col = col;
        view.scroll_offset = view.scroll_offset.saturating_sub(viewport_height);
    }

    pub fn half_page_down(&mut self) {
        let viewport_height = 15;
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        let new_row = (cs.cursor.row + viewport_height).min(buffer.lines.len().saturating_sub(1));
        cs.cursor.row = new_row;
        let col = first_non_ws(&buffer.lines[new_row].text);
        cs.cursor.col = col;
        cs.cursor.desired_col = col;
    }

    pub fn half_page_up(&mut self) {
        let viewport_height = 15;
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        let new_row = cs.cursor.row.saturating_sub(viewport_height);
        cs.cursor.row = new_row;
        let col = first_non_ws(&buffer.lines[new_row].text);
        cs.cursor.col = col;
        cs.cursor.desired_col = col;
    }

    pub fn adjust_scroll(&mut self, viewport_height: usize) {
        let view = &mut self.views[self.active_view_idx];
        let cursor_row = view.cursor().row;
        if cursor_row < view.scroll_offset {
            view.scroll_offset = cursor_row;
        } else if cursor_row >= view.scroll_offset + viewport_height {
            view.scroll_offset = cursor_row.saturating_sub(viewport_height - 1);
        }
    }
}

fn first_non_ws(line: &str) -> usize {
    line.chars().position(|c| !c.is_whitespace()).unwrap_or(0)
}
