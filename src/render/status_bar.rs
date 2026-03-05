use ratatui::{
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{block::*, Block, Borders, Widget},
    buffer::Buffer,
};

use crate::mode::Mode;
use crate::theme::Theme;

pub struct StatusBar<'a> {
    pub mode: &'a Mode,
    pub filename: &'a Option<String>,
    pub modified: bool,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub char_num: usize,
    pub command_buffer: &'a str,
    pub is_active: bool,
    pub theme: &'a Theme,
    pub lsp_status: Option<&'a str>,
    pub search_info: Option<String>,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let ui = &self.theme.ui;

        let title_text = if let Some(ref filename) = self.filename {
            if self.modified {
                format!(" Y Editor - {}[+] ", filename)
            } else {
                format!(" Y Editor - {} ", filename)
            }
        } else {
            " Y Editor [No Name] ".to_string()
        };
        let title = Title::from(title_text.bold());

        let position_info = Title::from(
            format!(
                " F1:Help | LN:{} CL:{} CHR:{} ",
                self.cursor_row + 1,
                self.cursor_col + 1,
                self.char_num
            )
            .fg(ui.status_position_fg)
            .bold(),
        );

        let mode_color = match self.mode {
            Mode::Normal => ui.status_mode_normal,
            Mode::Insert => ui.status_mode_insert,
            Mode::Visual | Mode::VisualLine => ui.status_mode_visual,
            Mode::Command => ui.status_mode_command,
            Mode::FuzzyFinder => ui.status_mode_normal,
            Mode::Search => ui.status_mode_command,
            Mode::Normie => ui.status_mode_insert,
        };

        let mode_text = match self.mode {
            Mode::Normal => "-- NORMAL --",
            Mode::Insert => "-- INSERT --",
            Mode::Visual => "-- VISUAL --",
            Mode::VisualLine => "-- VISUAL LINE --",
            Mode::Command => "-- COMMAND --",
            Mode::Search => "-- SEARCH --",
            Mode::FuzzyFinder => "-- FINDER --",
            Mode::Normie => "-- EDIT --",
        };

        let instructions = if *self.mode == Mode::Command {
            Title::from(Line::from(vec![
                ":".into(),
                self.command_buffer.to_string().fg(ui.popup_query),
            ]))
        } else if *self.mode == Mode::Search {
            let mut spans = vec![
                "/".into(),
                self.command_buffer.to_string().fg(ui.popup_query),
            ];
            if let Some(ref info) = self.search_info {
                spans.push(" ".into());
                spans.push(format!("[{}]", info).fg(ui.status_position_fg));
            }
            Title::from(Line::from(spans))
        } else {
            let mut spans = vec![
                " ".into(),
                mode_text.fg(mode_color).bold(),
                " ".into(),
            ];
            if let Some(ref info) = self.search_info {
                spans.push(format!("[{}]", info).fg(ui.status_position_fg));
                spans.push(" ".into());
            }
            Title::from(Line::from(spans))
        };

        let border_color = if self.is_active {
            ui.border_active
        } else {
            ui.border_inactive
        };

        let mut block = Block::default()
            .title(title.alignment(Alignment::Center))
            .title(
                instructions
                    .alignment(Alignment::Left)
                    .position(Position::Bottom),
            )
            .title(
                position_info
                    .alignment(Alignment::Right)
                    .position(Position::Bottom),
            )
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(border_color));

        // Show LSP status in top-right corner
        if let Some(status) = self.lsp_status {
            let lsp_color = if status == "ready" {
                ui.status_mode_normal
            } else {
                ui.status_mode_insert
            };
            let lsp_title = Title::from(
                format!(" {} ", status).fg(lsp_color).bold(),
            );
            block = block.title(
                lsp_title
                    .alignment(Alignment::Right)
                    .position(Position::Top),
            );
        }

        block.render(area, buf);
    }
}
