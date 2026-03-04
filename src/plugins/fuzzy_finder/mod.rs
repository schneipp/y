use crossterm::event::{self, KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use std::process::Command;

use crate::plugins::{Plugin, PluginContext};

#[derive(Debug, PartialEq, Clone)]
pub enum FuzzyFinderType {
    Files,
    Grep,
}

pub struct FuzzyFinderPlugin {
    active: bool,
    finder_type: Option<FuzzyFinderType>,
    query: String,
    results: Vec<String>,
    selected: usize,
}

impl FuzzyFinderPlugin {
    pub fn new() -> Self {
        Self {
            active: false,
            finder_type: None,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
        }
    }

    /// Activate the plugin with a specific finder type
    pub fn activate(&mut self, finder_type: FuzzyFinderType) {
        self.active = true;
        self.finder_type = Some(finder_type.clone());
        self.query.clear();
        self.selected = 0;

        match finder_type {
            FuzzyFinderType::Files => self.run_rg_files(),
            FuzzyFinderType::Grep => {
                self.results.clear();
            }
        }
    }

    fn run_rg_files(&mut self) {
        let output = Command::new("rg")
            .args(&["--files", "--hidden", "--glob", "!.git"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let files = String::from_utf8_lossy(&output.stdout);
                self.results = files.lines().map(|s| s.to_string()).collect();
            }
        }
    }

    fn run_rg_grep(&mut self, query: &str) {
        if query.is_empty() {
            self.results.clear();
            return;
        }

        let output = Command::new("rg")
            .args(&[
                "--line-number",
                "--column",
                "--no-heading",
                "--color=never",
                "--hidden",
                "--glob",
                "!.git",
                query,
            ])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let results = String::from_utf8_lossy(&output.stdout);
                self.results = results.lines().take(100).map(|s| s.to_string()).collect();
            } else {
                self.results.clear();
            }
        }
    }

    fn handle_query_update(&mut self) {
        if let Some(ref finder_type) = self.finder_type {
            match finder_type {
                FuzzyFinderType::Files => {
                    // Re-run file search and filter results
                    self.run_rg_files();
                    let query = self.query.to_lowercase();
                    if !query.is_empty() {
                        self.results.retain(|f| f.to_lowercase().contains(&query));
                    }
                    self.results.truncate(100);
                    self.selected = 0;
                }
                FuzzyFinderType::Grep => {
                    let query = self.query.clone();
                    self.run_rg_grep(&query);
                    self.selected = 0;
                }
            }
        }
    }

    fn open_selected_result(&mut self, ctx: &mut PluginContext) -> bool {
        if self.selected < self.results.len() {
            let selected = &self.results[self.selected];

            match self.finder_type.as_ref() {
                Some(FuzzyFinderType::Files) => {
                    // Open the file
                    if let Ok(content) = std::fs::read_to_string(selected) {
                        let lines: Vec<crate::YLine> = if content.is_empty() {
                            vec![crate::YLine::new()]
                        } else {
                            content.lines()
                                .map(|line| crate::YLine::from(line.to_string()))
                                .collect()
                        };
                        *ctx.buffer = crate::YBuffer::from(lines);
                        ctx.cursor.row = 0;
                        ctx.cursor.col = 0;
                        ctx.cursor.desired_col = 0;
                        *ctx.mode = crate::Mode::Normal;
                        *ctx.modified = false;
                        self.deactivate();
                        return true;
                    }
                }
                Some(FuzzyFinderType::Grep) => {
                    // Parse grep result: filename:line:col:text
                    let parts: Vec<&str> = selected.splitn(4, ':').collect();
                    if parts.len() >= 3 {
                        let filename = parts[0];
                        let line_num = parts[1].parse::<usize>().unwrap_or(1);

                        // Open file and jump to line
                        if let Ok(content) = std::fs::read_to_string(filename) {
                            let lines: Vec<crate::YLine> = if content.is_empty() {
                                vec![crate::YLine::new()]
                            } else {
                                content.lines()
                                    .map(|line| crate::YLine::from(line.to_string()))
                                    .collect()
                            };
                            *ctx.buffer = crate::YBuffer::from(lines);
                            ctx.cursor.row = line_num
                                .saturating_sub(1)
                                .min(ctx.buffer.lines.len().saturating_sub(1));
                            ctx.cursor.col = 0;
                            ctx.cursor.desired_col = 0;
                            *ctx.mode = crate::Mode::Normal;
                            *ctx.modified = false;
                            self.deactivate();
                            return true;
                        }
                    }
                }
                None => {}
            }
        }
        false
    }
}

impl Plugin for FuzzyFinderPlugin {
    fn name(&self) -> &str {
        "fuzzy_finder"
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        if !self.active {
            return false;
        }

        match key.code {
            KeyCode::Esc => {
                self.deactivate();
                *ctx.mode = crate::Mode::Normal;
                return true;
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.handle_query_update();
                return true;
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.handle_query_update();
                return true;
            }
            KeyCode::Down => {
                if key.modifiers.contains(event::KeyModifiers::CONTROL) {
                    if self.selected < self.results.len().saturating_sub(1) {
                        self.selected += 1;
                    }
                    return true;
                }
            }
            KeyCode::Up => {
                if key.modifiers.contains(event::KeyModifiers::CONTROL) {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                    return true;
                }
            }
            KeyCode::Enter => {
                return self.open_selected_result(ctx);
            }
            _ => {}
        }

        false
    }

    fn render(&self, area: Rect, buf: &mut Buffer, _ctx: &PluginContext) {
        if !self.active {
            return;
        }

        // Calculate popup size (centered, 80% width, 60% height)
        let popup_width = (area.width as f32 * 0.8) as u16;
        let popup_height = (area.height as f32 * 0.6) as u16;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Clear the popup area
        for y in popup_area.y..popup_area.y + popup_area.height {
            for x in popup_area.x..popup_area.x + popup_area.width {
                if x < buf.area.width && y < buf.area.height {
                    buf.get_mut(x, y).reset();
                }
            }
        }

        // Build title based on finder type
        let title_text = match self.finder_type {
            Some(FuzzyFinderType::Files) => " Find Files ",
            Some(FuzzyFinderType::Grep) => " Find in Files ",
            None => " Fuzzy Finder ",
        };

        // Create block for popup
        let popup_block = Block::default()
            .title(title_text)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        // Calculate inner area for content (before rendering which consumes the block)
        let inner = popup_block.inner(popup_area);

        popup_block.render(popup_area, buf);

        // Render query line
        if inner.height > 0 {
            let query_text = format!("> {}", self.query);
            let query_line = Line::from(Span::styled(
                query_text,
                Style::default().fg(Color::Yellow),
            ));
            let query_area = Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            };
            Paragraph::new(query_line).render(query_area, buf);
        }

        // Render results
        if inner.height > 2 {
            let results_area = Rect {
                x: inner.x,
                y: inner.y + 2,
                width: inner.width,
                height: inner.height.saturating_sub(2),
            };

            let visible_results: Vec<Line> = self
                .results
                .iter()
                .enumerate()
                .skip(self.selected.saturating_sub(10))
                .take(results_area.height as usize)
                .map(|(idx, result)| {
                    let display_text = if result.len() > results_area.width as usize {
                        format!(
                            "{}...",
                            &result[..results_area.width.saturating_sub(3) as usize]
                        )
                    } else {
                        result.clone()
                    };

                    if idx == self.selected {
                        Line::from(Span::styled(
                            format!("> {}", display_text),
                            Style::default().fg(Color::Black).bg(Color::White),
                        ))
                    } else {
                        Line::from(format!("  {}", display_text))
                    }
                })
                .collect();

            let results_text = Text::from(visible_results);
            Paragraph::new(results_text).render(results_area, buf);
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn deactivate(&mut self) {
        self.active = false;
        self.finder_type = None;
        self.query.clear();
        self.results.clear();
        self.selected = 0;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
