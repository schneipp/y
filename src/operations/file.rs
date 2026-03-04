use std::fs;

use crate::editor::Editor;

impl Editor {
    pub fn save_file(&mut self) {
        let view = &self.views[self.active_view_idx];
        let buffer_id = view.buffer_id;

        // Use per-buffer filename, fall back to editor-level
        let filename = self.buffer_pool.get_entry(buffer_id).filename.clone()
            .or_else(|| self.filename.clone());

        if let Some(ref filename) = filename {
            let buffer = self.buffer_pool.get(buffer_id);
            let content: String = buffer
                .lines
                .iter()
                .map(|line| line.text.clone())
                .collect::<Vec<String>>()
                .join("\n");

            if fs::write(filename, content).is_ok() {
                self.modified = false;
                self.buffer_pool.get_entry_mut(buffer_id).modified = false;
            }
        }
    }

    pub fn quit_command(&mut self) {
        if !self.modified {
            self.exit = true;
        }
    }
}
