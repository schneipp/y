use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::buffer::{BufferId, YBuffer};
use crate::mode::Mode;
use crate::plugins::PluginManager;
use crate::theme::Theme;
use crate::view::View;

pub struct BufferWidget<'a> {
    pub buffer: &'a YBuffer,
    pub buffer_id: BufferId,
    pub view: &'a View,
    pub mode: &'a Mode,
    pub plugin_manager: &'a PluginManager,
    pub theme: &'a Theme,
    pub is_active: bool,
    pub show_line_numbers: bool,
    /// Ghost text to render inline at cursor position (suffix text, shown dimmed)
    pub ghost_text: Option<&'a str>,
    pub search_query: &'a str,
}

impl<'a> BufferWidget<'a> {
    fn line_number_width(&self) -> u16 {
        if !self.show_line_numbers {
            return 0;
        }
        let max_line = self.buffer.lines.len();
        let digits = if max_line == 0 { 1 } else { (max_line as f64).log10() as u16 + 1 };
        digits + 1
    }

    fn char_col_to_byte(text: &str, char_col: usize) -> usize {
        text.char_indices()
            .nth(char_col)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(text.len())
    }

    fn build_highlighted_spans<'b>(
        text: &'b str,
        highlights: &[(usize, usize, Color)],
    ) -> Line<'b> {
        let mut spans = Vec::with_capacity(highlights.len() * 2 + 1);
        let mut byte_pos = 0;

        for &(start_col, end_col, color) in highlights {
            let start_byte = Self::char_col_to_byte(text, start_col);
            let end_byte = Self::char_col_to_byte(text, end_col.min(text.chars().count()));

            if byte_pos < start_byte {
                spans.push(Span::raw(&text[byte_pos..start_byte]));
            }

            let actual_start = byte_pos.max(start_byte);
            if actual_start < end_byte {
                spans.push(Span::styled(
                    &text[actual_start..end_byte],
                    Style::default().fg(color),
                ));
            }
            byte_pos = end_byte;
        }

        if byte_pos < text.len() {
            spans.push(Span::raw(&text[byte_pos..]));
        }

        Line::from(spans)
    }
}

impl<'a> Widget for BufferWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let ui = &self.theme.ui;
        let viewport_height = area.height as usize;
        let ln_width = self.line_number_width();

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                let cell = buf.get_mut(x, y);
                cell.set_style(Style::default().fg(ui.foreground).bg(ui.background));
                cell.set_char(' ');
            }
        }

        // Collect all visual selections from all cursor states
        let visual_selections: Vec<((usize, usize), (usize, usize))> = if *self.mode == Mode::Visual || *self.mode == Mode::VisualLine {
            self.view
                .cursor_states
                .iter()
                .filter_map(|cs| {
                    let (sr, sc) = cs.visual_start?;
                    Some(((sr, sc), (cs.cursor.row, cs.cursor.col)))
                })
                .collect()
        } else {
            Vec::new()
        };

        let secondary_positions: Vec<(usize, usize)> = if self.view.has_multiple_cursors() {
            self.view
                .cursor_states
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != self.view.primary_cursor_idx)
                .map(|(_, cs)| (cs.cursor.row, cs.cursor.col))
                .collect()
        } else {
            Vec::new()
        };

        let cursor = self.view.cursor();
        let content_width = area.width.saturating_sub(ln_width);

        for (idx, (row, yline)) in self
            .buffer
            .lines
            .iter()
            .enumerate()
            .skip(self.view.scroll_offset)
            .take(viewport_height)
            .enumerate()
        {
            let y = area.y + idx as u16;
            if y >= area.y + area.height {
                break;
            }

            if self.show_line_numbers {
                let ln_str = format!("{:>width$} ", row + 1, width = (ln_width - 1) as usize);
                buf.set_string(area.x, y, &ln_str, Style::default().fg(ui.line_number_fg).bg(ui.background));
            }

            let content_x = area.x + ln_width;
            let text = &yline.text;

            // First, render the base text (syntax highlighted or plain)
            let mut used_syntax = false;
            if let Some(plugin) = self.plugin_manager.get("syntax_highlighter") {
                if let Some(highlighter) = plugin
                    .as_ref()
                    .as_any()
                    .downcast_ref::<crate::plugins::syntax_highlighter::SyntaxHighlighter>()
                {
                    let highlights = highlighter.get_line_highlights(self.buffer_id, row);
                    if !highlights.is_empty() {
                        let line = Self::build_highlighted_spans(text, highlights);
                        buf.set_line(content_x, y, &line, content_width);
                        used_syntax = true;
                    }
                }
            }
            if !used_syntax {
                buf.set_string(content_x, y, text.as_str(), Style::default().fg(ui.foreground));
            }

            // Then overlay visual selections from ALL cursor states
            for &((start_row, start_col), (end_row, end_col)) in &visual_selections {
                if *self.mode == Mode::VisualLine {
                    let (first_line, last_line) = if start_row <= end_row {
                        (start_row, end_row)
                    } else {
                        (end_row, start_row)
                    };
                    if row >= first_line && row <= last_line {
                        let sel_style = Style::default()
                            .fg(ui.visual_selection_fg)
                            .bg(ui.visual_selection_bg);
                        let char_count = text.chars().count();
                        for c in 0..char_count.max(1) {
                            let sx = content_x + c as u16;
                            if sx < area.x + area.width {
                                let cell = buf.get_mut(sx, y);
                                cell.set_style(sel_style);
                            }
                        }
                    }
                } else {
                    // Visual (character) mode
                    let (s, e) = if (start_row, start_col) <= (end_row, end_col) {
                        ((start_row, start_col), (end_row, end_col))
                    } else {
                        ((end_row, end_col), (start_row, start_col))
                    };

                    if row >= s.0 && row <= e.0 {
                        let sel_start = if row == s.0 { s.1 } else { 0 };
                        let sel_end = if row == e.0 {
                            e.1
                        } else {
                            text.chars().count().saturating_sub(1)
                        };
                        let sel_style = Style::default()
                            .fg(ui.visual_selection_fg)
                            .bg(ui.visual_selection_bg);
                        for c in sel_start..=sel_end {
                            let sx = content_x + c as u16;
                            if sx < area.x + area.width {
                                let cell = buf.get_mut(sx, y);
                                cell.set_style(sel_style);
                            }
                        }
                    }
                }
            }

            // Highlight search matches
            if !self.search_query.is_empty() {
                let search_style = Style::default().bg(ui.search_match_bg);
                let query_char_len = self.search_query.chars().count();
                let mut search_from = 0;
                while let Some(byte_pos) = text[search_from..].find(self.search_query) {
                    let match_start_byte = search_from + byte_pos;
                    let match_start_col = text[..match_start_byte].chars().count();
                    for c in match_start_col..match_start_col + query_char_len {
                        let sx = content_x + c as u16;
                        if sx < area.x + area.width {
                            let cell = buf.get_mut(sx, y);
                            cell.set_style(search_style);
                        }
                    }
                    search_from = match_start_byte + self.search_query.len();
                }
            }

            // Render ghost text on the cursor line
            if self.is_active && row == cursor.row {
                if let Some(ghost) = self.ghost_text {
                    let ghost_x = content_x + cursor.col as u16;
                    let ghost_style = Style::default().fg(ui.ghost_text).bg(ui.background);
                    let available = (area.x + area.width).saturating_sub(ghost_x) as usize;
                    if available > 0 {
                        let display: String = ghost.chars().take(available).collect();
                        buf.set_string(ghost_x, y, &display, ghost_style);
                    }
                }
            }

            for &(cr, cc) in &secondary_positions {
                if cr == row {
                    let screen_x = content_x + cc as u16;
                    if screen_x < area.x + area.width && y < area.y + area.height {
                        let cell = buf.get_mut(screen_x, y);
                        cell.set_style(Style::default().bg(ui.secondary_cursor_bg));
                    }
                }
            }
        }
    }
}
