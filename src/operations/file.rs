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
            // Try LSP formatting before saving
            self.format_with_lsp(buffer_id, filename);

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

    /// Request LSP formatting and apply text edits to the buffer.
    fn format_with_lsp(&mut self, buffer_id: crate::buffer::BufferId, filename: &str) {
        let ext = match std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
        {
            Some(e) => e.to_string(),
            None => return,
        };

        let server_config = match self.config.server_for_extension(&ext) {
            Some(s) => s.clone(),
            None => return,
        };

        if !self.lsp_manager.is_server_ready(&server_config.name) {
            return;
        }

        let abs_path = std::fs::canonicalize(filename)
            .unwrap_or_else(|_| std::path::PathBuf::from(filename));
        let uri = format!("file://{}", abs_path.display());

        // Send didChange with latest buffer content first
        let buffer = self.buffer_pool.get(buffer_id);
        let text: String = buffer
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let version = self.completion.bump_version();
        self.lsp_manager
            .did_change(&server_config.name, &uri, version, &text);

        // Detect indent settings from buffer
        let (tab_size, insert_spaces) = detect_indent_settings(self.buffer_pool.get(buffer_id));

        let request_id = match self.lsp_manager.request_formatting(
            &server_config.name,
            &uri,
            tab_size,
            insert_spaces,
        ) {
            Some(id) => id,
            None => return,
        };

        // Wait for the formatting response (up to 5 seconds)
        let result = self.lsp_manager.wait_for_response(
            &server_config.name,
            request_id,
            std::time::Duration::from_secs(5),
        );

        if let Some(edits_value) = result {
            if let Some(edits) = edits_value.as_array() {
                apply_text_edits(self.buffer_pool.get_mut(buffer_id), edits);
            }
        }
    }

    pub fn quit_command(&mut self) {
        if !self.modified {
            self.exit = true;
        }
    }
}

/// Detect tab size and whether to use spaces from the buffer content.
fn detect_indent_settings(buffer: &crate::buffer::YBuffer) -> (u32, bool) {
    for line in &buffer.lines {
        if line.text.starts_with('\t') {
            return (4, false);
        }
        let spaces = line.text.len() - line.text.trim_start_matches(' ').len();
        if spaces >= 2 {
            return (spaces.min(8) as u32, true);
        }
    }
    (4, true)
}

/// Apply LSP TextEdit[] to a buffer. Edits are applied in reverse order
/// (bottom-to-top) to avoid position invalidation.
fn apply_text_edits(buffer: &mut crate::buffer::YBuffer, edits: &[serde_json::Value]) {
    // Parse and sort edits by position (reverse order for safe application)
    let mut parsed: Vec<(usize, usize, usize, usize, String)> = edits
        .iter()
        .filter_map(|edit| {
            let range = edit.get("range")?;
            let start = range.get("start")?;
            let end = range.get("end")?;
            let new_text = edit.get("newText")?.as_str()?.to_string();
            Some((
                start.get("line")?.as_u64()? as usize,
                start.get("character")?.as_u64()? as usize,
                end.get("line")?.as_u64()? as usize,
                end.get("character")?.as_u64()? as usize,
                new_text,
            ))
        })
        .collect();

    // Sort by position, reverse (bottom-right first)
    parsed.sort_by(|a, b| (b.2, b.3, b.0, b.1).cmp(&(a.2, a.3, a.0, a.1)));

    for (start_line, start_char, end_line, end_char, new_text) in parsed {
        if start_line >= buffer.lines.len() {
            continue;
        }

        // Collect the text before the edit start and after the edit end
        // LSP uses character positions, convert to proper string operations
        let start_yline = &buffer.lines[start_line];
        let start_char_count = start_yline.char_count();
        let before = if start_char <= start_char_count {
            start_yline.text.chars().take(start_char).collect::<String>()
        } else {
            start_yline.text.clone()
        };

        let after = if end_line < buffer.lines.len() {
            let end_yline = &buffer.lines[end_line];
            let end_char_count = end_yline.char_count();
            if end_char <= end_char_count {
                end_yline.text.chars().skip(end_char).collect::<String>()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Remove lines from start to end
        let remove_count = end_line.min(buffer.lines.len() - 1) - start_line + 1;
        for _ in 0..remove_count {
            if start_line < buffer.lines.len() {
                buffer.lines.remove(start_line);
            }
        }

        // Insert new content
        let combined = format!("{}{}{}", before, new_text, after);
        let new_lines: Vec<&str> = combined.split('\n').collect();
        for (i, line_text) in new_lines.iter().enumerate() {
            buffer
                .lines
                .insert(start_line + i, crate::buffer::YLine::from(line_text.to_string()));
        }
    }

    // Ensure buffer is never empty
    if buffer.lines.is_empty() {
        buffer.lines.push(crate::buffer::YLine::new());
    }
}
