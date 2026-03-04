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

    fn render_visual_line<'b>(text: &'b str, sel_start: usize, sel_end: usize, fg: Color, bg: Color) -> Line<'b> {
        let char_len = text.chars().count();
        let start = sel_start.min(char_len);
        let end = (sel_end + 1).min(char_len);

        let start_byte = Self::char_col_to_byte(text, start);
        let end_byte = Self::char_col_to_byte(text, end);

        let mut spans = Vec::with_capacity(3);
        if start_byte > 0 {
            spans.push(Span::raw(&text[..start_byte]));
        }
        if start_byte < end_byte {
            spans.push(Span::styled(
                &text[start_byte..end_byte],
                Style::default().fg(fg).bg(bg),
            ));
        }
        if end_byte < text.len() {
            spans.push(Span::raw(&text[end_byte..]));
        }
        Line::from(spans)
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

        let visual_start = self.view.visual_start();
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

            let rendered = if let Some((start_row, start_col)) = visual_start {
                if *self.mode == Mode::Visual || *self.mode == Mode::VisualLine {
                    let end_row = cursor.row;
                    let end_col = cursor.col;

                    if *self.mode == Mode::VisualLine {
                        let (first_line, last_line) = if start_row <= end_row {
                            (start_row, end_row)
                        } else {
                            (end_row, start_row)
                        };
                        if row >= first_line && row <= last_line {
                            Some(Line::from(Span::styled(
                                text.as_str(),
                                Style::default().fg(ui.visual_selection_fg).bg(ui.visual_selection_bg),
                            )))
                        } else {
                            None
                        }
                    } else {
                        let (start_pos, end_pos) =
                            if (start_row, start_col) <= (end_row, end_col) {
                                ((start_row, start_col), (end_row, end_col))
                            } else {
                                ((end_row, end_col), (start_row, start_col))
                            };

                        if row >= start_pos.0 && row <= end_pos.0 {
                            let sel_start = if row == start_pos.0 { start_pos.1 } else { 0 };
                            let sel_end = if row == end_pos.0 {
                                end_pos.1
                            } else {
                                text.chars().count().saturating_sub(1)
                            };
                            Some(Self::render_visual_line(
                                text, sel_start, sel_end,
                                ui.visual_selection_fg, ui.visual_selection_bg,
                            ))
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(line) = rendered {
                buf.set_line(content_x, y, &line, content_width);
            } else {
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
