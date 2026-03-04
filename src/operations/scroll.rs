use crate::editor::Editor;
use crate::operations::motion::clamp_cursor_to_line;

impl Editor {
    pub fn page_down(&mut self) {
        let viewport_height = 30;
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        let new_row = (cs.cursor.row + viewport_height).min(buffer.lines.len().saturating_sub(1));
        cs.cursor.row = new_row;
        cs.cursor.col = 0;
        cs.cursor.desired_col = 0;
        view.scroll_offset = (view.scroll_offset + viewport_height)
            .min(buffer.lines.len().saturating_sub(viewport_height));
    }

    pub fn page_up(&mut self) {
        let viewport_height = 30;
        let view = &mut self.views[self.active_view_idx];
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        let new_row = cs.cursor.row.saturating_sub(viewport_height);
        cs.cursor.row = new_row;
        cs.cursor.col = 0;
        cs.cursor.desired_col = 0;
        view.scroll_offset = view.scroll_offset.saturating_sub(viewport_height);
    }

    pub fn half_page_down(&mut self) {
        let viewport_height = 15;
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        let new_row = (cs.cursor.row + viewport_height).min(buffer.lines.len().saturating_sub(1));
        cs.cursor.row = new_row;
        clamp_cursor_to_line(&mut cs.cursor, buffer);
    }

    pub fn half_page_up(&mut self) {
        let viewport_height = 15;
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let cs = &mut view.cursor_states[view.primary_cursor_idx];

        let new_row = cs.cursor.row.saturating_sub(viewport_height);
        cs.cursor.row = new_row;
        clamp_cursor_to_line(&mut cs.cursor, buffer);
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
