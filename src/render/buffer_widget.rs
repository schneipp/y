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
    pub relative_line_numbers: bool,
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

    /// Map a char index in original text to a char index in the tab-expanded text.
    fn char_to_visual(text: &str, char_idx: usize, tab_width: usize) -> usize {
        let mut vcol = 0;
        for (i, ch) in text.chars().enumerate() {
            if i >= char_idx {
                break;
            }
            if ch == '\t' {
                vcol += tab_width - (vcol % tab_width);
            } else {
                vcol += 1;
            }
        }
        vcol
    }

    fn build_highlighted_spans_tabbed(
        text: &str,
        display_text: &str,
        highlights: &[(usize, usize, Color)],
    ) -> Line<'static> {
        let tab_width = 4;
        let mut spans = Vec::with_capacity(highlights.len() * 2 + 1);
        let mut vis_pos = 0;
        let char_count = text.chars().count();

        for &(start_col, end_col, color) in highlights {
            let vis_start = Self::char_to_visual(text, start_col, tab_width);
            let vis_end = Self::char_to_visual(text, end_col.min(char_count), tab_width);

            if vis_pos < vis_start {
                let slice: String = display_text.chars().skip(vis_pos).take(vis_start - vis_pos).collect();
                spans.push(Span::raw(slice));
            }

            let actual_start = vis_pos.max(vis_start);
            if actual_start < vis_end {
                let slice: String = display_text.chars().skip(actual_start).take(vis_end - actual_start).collect();
                spans.push(Span::styled(slice, Style::default().fg(color)));
            }
            vis_pos = vis_end;
        }

        let display_len = display_text.chars().count();
        if vis_pos < display_len {
            let slice: String = display_text.chars().skip(vis_pos).collect();
            spans.push(Span::raw(slice));
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
                let cursor_row = self.view.cursor().row;
                let ln_str = if self.relative_line_numbers {
                    if row == cursor_row {
                        format!("{:>width$} ", row + 1, width = (ln_width - 1) as usize)
                    } else {
                        let rel = (row as isize - cursor_row as isize).unsigned_abs();
                        format!("{:>width$} ", rel, width = (ln_width - 1) as usize)
                    }
                } else {
                    format!("{:>width$} ", row + 1, width = (ln_width - 1) as usize)
                };
                let ln_fg = if self.relative_line_numbers && row == cursor_row {
                    ui.foreground
                } else {
                    ui.line_number_fg
                };
                buf.set_string(area.x, y, &ln_str, Style::default().fg(ln_fg).bg(ui.background));
            }

            let content_x = area.x + ln_width;
            let text = &yline.text;
            let display_text = yline.expanded_text(4);

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
                        let line = Self::build_highlighted_spans_tabbed(text, &display_text, highlights);
                        buf.set_line(content_x, y, &line, content_width);
                        used_syntax = true;
                    }
                }
            }
            if !used_syntax {
                buf.set_string(content_x, y, display_text.as_str(), Style::default().fg(ui.foreground));
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
                        let vis_len = display_text.chars().count();
                        for c in 0..vis_len.max(1) {
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
                        let vis_start = Self::char_to_visual(text, sel_start, 4);
                        let vis_end = Self::char_to_visual(text, sel_end + 1, 4);
                        for vc in vis_start..vis_end {
                            let sx = content_x + vc as u16;
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
                    let vis_start = Self::char_to_visual(text, match_start_col, 4);
                    let vis_end = Self::char_to_visual(text, match_start_col + query_char_len, 4);
                    for vc in vis_start..vis_end {
                        let sx = content_x + vc as u16;
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
                    let ghost_x = content_x + yline.visual_col(cursor.col, 4) as u16;
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
                    let screen_x = content_x + Self::char_to_visual(text, cc, 4) as u16;
                    if screen_x < area.x + area.width && y < area.y + area.height {
                        let cell = buf.get_mut(screen_x, y);
                        cell.set_style(Style::default().bg(ui.secondary_cursor_bg));
                    }
                }
            }
        }
    }
}
