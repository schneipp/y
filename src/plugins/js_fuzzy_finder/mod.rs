use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

use crate::plugins::{Plugin, PluginContext};
use crate::plugins::deno_runtime::{DenoPluginRuntime, JsKeyEvent, JsContext, JsPluginAction};

#[derive(Debug, PartialEq, Clone)]
pub enum FuzzyFinderType {
    Files,
    Grep,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RenderData {
    active: bool,
    title: String,
    query: String,
    results: Vec<String>,
    selected: usize,
}

impl Default for RenderData {
    fn default() -> Self {
        Self {
            active: false,
            title: String::new(),
            query: String::new(),
            results: Vec::new(),
            selected: 0,
        }
    }
}

pub struct JsFuzzyFinderPlugin {
    runtime: RefCell<DenoPluginRuntime>,
    plugin_name: String,
    cached_render_data: RefCell<RenderData>,
}

impl JsFuzzyFinderPlugin {
    pub fn new() -> Self {
        let mut runtime = DenoPluginRuntime::new();

        // Load the fuzzy finder JavaScript plugin
        if let Err(e) = runtime.load_plugin("plugins/fuzzy_finder.js") {
            eprintln!("Failed to load fuzzy_finder.js: {}", e);
        }

        Self {
            runtime: RefCell::new(runtime),
            plugin_name: "fuzzyFinder".to_string(),
            cached_render_data: RefCell::new(RenderData::default()),
        }
    }

    /// Activate the plugin with a specific finder type
    pub fn activate(&mut self, finder_type: FuzzyFinderType) {
        let finder_type_str = match finder_type {
            FuzzyFinderType::Files => "files",
            FuzzyFinderType::Grep => "grep",
        };

        // Call activate method with finder type
        let code = format!(
            r#"
            if (typeof {} !== 'undefined') {{
                {}.activate("{}");
            }}
            "#,
            self.plugin_name, self.plugin_name, finder_type_str
        );

        if let Err(e) = self.runtime.borrow_mut().runtime.execute_script("<activate>", code) {
            eprintln!("Failed to activate fuzzy finder: {}", e);
        }

        self.update_render_data();
    }

    fn update_render_data(&self) {
        let code = format!(
            r#"
            (function() {{
                if (typeof {} !== 'undefined') {{
                    return JSON.stringify({}.getRenderData());
                }}
                return JSON.stringify({{ active: false, title: "", query: "", results: [], selected: 0 }});
            }})()
            "#,
            self.plugin_name, self.plugin_name
        );

        let mut runtime = self.runtime.borrow_mut();
        match runtime.runtime.execute_script("<getRenderData>", code) {
            Ok(result) => {
                let scope = &mut runtime.runtime.handle_scope();
                let local = deno_core::v8::Local::new(scope, result);
                let result_str = local.to_rust_string_lossy(scope);

                if let Ok(data) = serde_json::from_str::<RenderData>(&result_str) {
                    *self.cached_render_data.borrow_mut() = data;
                }
            }
            Err(e) => {
                eprintln!("Failed to get render data: {}", e);
            }
        }
    }

    /// Get the length of the current query for cursor positioning
    pub fn get_query_length(&self) -> usize {
        self.cached_render_data.borrow().query.len()
    }

    fn convert_key_event(key: KeyEvent) -> JsKeyEvent {
        let code = match key.code {
            KeyCode::Char(_) => "Char",
            KeyCode::Enter => "Enter",
            KeyCode::Esc => "Esc",
            KeyCode::Backspace => "Backspace",
            KeyCode::Left => "Left",
            KeyCode::Right => "Right",
            KeyCode::Up => "Up",
            KeyCode::Down => "Down",
            _ => "Other",
        }
        .to_string();

        let char = match key.code {
            KeyCode::Char(c) => Some(c.to_string()),
            _ => None,
        };

        let mut modifiers = Vec::new();
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            modifiers.push("CONTROL".to_string());
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            modifiers.push("SHIFT".to_string());
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            modifiers.push("ALT".to_string());
        }

        JsKeyEvent {
            code,
            modifiers,
            kind: "Press".to_string(),
            char,
        }
    }

    fn convert_context(ctx: &PluginContext) -> JsContext {
        JsContext {
            cursor_row: ctx.cursor.row,
            cursor_col: ctx.cursor.col,
            mode: format!("{:?}", ctx.mode),
            filename: ctx.filename.clone(),
            modified: *ctx.modified,
            buffer_lines: ctx.buffer.lines.iter().map(|l| l.text.clone()).collect(),
        }
    }
}

impl Plugin for JsFuzzyFinderPlugin {
    fn name(&self) -> &str {
        "js_fuzzy_finder"
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        if !self.is_active() {
            return false;
        }

        let js_key = Self::convert_key_event(key);
        let js_ctx = Self::convert_context(ctx);

        let mut runtime = self.runtime.borrow_mut();
        match runtime.handle_key(&self.plugin_name, &js_key, &js_ctx) {
            Ok(response) => {
                // Update cached render data after handling key
                drop(runtime); // Release the borrow before updating
                self.update_render_data();

                // Handle action if any
                if let Some(action) = response.action {
                    match action {
                        JsPluginAction::SetMode { mode } => {
                            if mode == "Normal" {
                                *ctx.mode = crate::Mode::Normal;
                            }
                        }
                        JsPluginAction::OpenFile { path, line } => {
                            // Read file
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                let lines: Vec<crate::YLine> = if content.is_empty() {
                                    vec![crate::YLine::new()]
                                } else {
                                    content
                                        .lines()
                                        .map(|line| crate::YLine::from(line.to_string()))
                                        .collect()
                                };
                                *ctx.buffer = crate::YBuffer::from(lines);

                                // Set cursor position
                                if let Some(line_num) = line {
                                    ctx.cursor.row = line_num.min(ctx.buffer.lines.len().saturating_sub(1));
                                } else {
                                    ctx.cursor.row = 0;
                                }
                                ctx.cursor.col = 0;
                                ctx.cursor.desired_col = 0;
                                *ctx.mode = crate::Mode::Normal;
                                *ctx.modified = false;
                            }
                        }
                        _ => {}
                    }
                }
                response.consumed
            }
            Err(e) => {
                eprintln!("Error handling key in JS plugin: {}", e);
                false
            }
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer, _ctx: &PluginContext) {
        let render_data = self.cached_render_data.borrow();

        if !render_data.active {
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

        // Create block for popup
        let popup_block = Block::default()
            .title(render_data.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = popup_block.inner(popup_area);
        popup_block.render(popup_area, buf);

        // Render query line
        if inner.height > 0 {
            let query_text = format!("> {}", render_data.query);
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

            let visible_results: Vec<Line> = render_data
                .results
                .iter()
                .enumerate()
                .skip(render_data.selected.saturating_sub(10))
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

                    if idx == render_data.selected {
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
        self.cached_render_data.borrow().active
    }

    fn deactivate(&mut self) {
        let mut runtime = self.runtime.borrow_mut();
        let _ = runtime.deactivate_plugin(&self.plugin_name);
        drop(runtime);
        self.update_render_data();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
