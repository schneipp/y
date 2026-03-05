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
pub struct RenderData {
    pub active: bool,
    pub title: String,
    pub query: String,
    pub results: Vec<String>,
    pub selected: usize,
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

/// Pending file-open action for the editor to handle (plugin can't do it — needs buffer pool access)
pub struct PendingOpen {
    pub path: String,
    pub line: Option<usize>,
}

/// Popup colors, synced from theme
pub struct PopupColors {
    pub border: Color,
    pub query: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
}

impl Default for PopupColors {
    fn default() -> Self {
        Self {
            border: Color::Cyan,
            query: Color::Yellow,
            selected_fg: Color::Black,
            selected_bg: Color::White,
        }
    }
}

pub struct JsFuzzyFinderPlugin {
    runtime: RefCell<DenoPluginRuntime>,
    plugin_name: String,
    pub cached_render_data: RefCell<RenderData>,
    pub custom_items: Option<Vec<String>>,
    pub pending_open: Option<PendingOpen>,
    pub popup_colors: PopupColors,
}

impl JsFuzzyFinderPlugin {
    pub fn new() -> Self {
        let mut runtime = DenoPluginRuntime::new();

        // Load the embedded fuzzy finder JavaScript plugin
        let js_source = include_str!("../../../plugins/fuzzy_finder.js");
        if let Err(e) = runtime.runtime.execute_script("<fuzzy_finder>", js_source.to_string()) {
            eprintln!("Failed to load fuzzy_finder.js: {}", e);
        }

        Self {
            runtime: RefCell::new(runtime),
            plugin_name: "fuzzyFinder".to_string(),
            cached_render_data: RefCell::new(RenderData::default()),
            custom_items: None,
            pending_open: None,
            popup_colors: PopupColors::default(),
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

    /// Activate with a custom item list (for buffer picker etc.)
    pub fn activate_with_items(&mut self, title: &str, items: Vec<String>) {
        *self.cached_render_data.borrow_mut() = RenderData {
            active: true,
            title: title.to_string(),
            query: String::new(),
            results: items.clone(),
            selected: 0,
        };
        self.custom_items = Some(items);
    }

    /// Check if this is a custom-items session (buffer picker, etc.)
    pub fn is_custom_mode(&self) -> bool {
        self.custom_items.is_some()
    }

    /// Get the selected index in custom mode
    pub fn get_selected_index(&self) -> usize {
        self.cached_render_data.borrow().selected
    }

    fn render_popup(&self, area: Rect, buf: &mut Buffer) {
        let render_data = self.cached_render_data.borrow();

        if !render_data.active {
            return;
        }

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

        // Use Clear widget for better double-buffer interaction
        ratatui::widgets::Clear.render(popup_area, buf);

        let popup_block = Block::default()
            .title(render_data.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.popup_colors.border));

        let inner = popup_block.inner(popup_area);
        popup_block.render(popup_area, buf);

        if inner.height > 0 {
            let query_text = format!("> {}", render_data.query);
            let query_line = Line::from(Span::styled(
                query_text,
                Style::default().fg(self.popup_colors.query),
            ));
            let query_area = Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            };
            Paragraph::new(query_line).render(query_area, buf);
        }

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
                            Style::default().fg(self.popup_colors.selected_fg).bg(self.popup_colors.selected_bg),
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

    fn handle_custom_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        let mut render_data = self.cached_render_data.borrow_mut();

        match key.code {
            KeyCode::Esc => {
                drop(render_data);
                self.custom_items = None;
                *self.cached_render_data.borrow_mut() = RenderData::default();
                *ctx.mode = ctx.default_mode.clone();
                return true;
            }
            KeyCode::Enter => {
                // Selection confirmed — keep selected index, deactivate
                let _selected = render_data.selected;
                drop(render_data);
                // Don't clear custom_items yet — the editor needs to read the selection
                *ctx.mode = ctx.default_mode.clone();
                // Mark as inactive but preserve state for the editor to read
                self.cached_render_data.borrow_mut().active = false;
                return true;
            }
            KeyCode::Char(c) => {
                render_data.query.push(c);
                // Filter items by query
                if let Some(ref items) = self.custom_items {
                    let query = render_data.query.to_lowercase();
                    render_data.results = items
                        .iter()
                        .filter(|item| item.to_lowercase().contains(&query))
                        .cloned()
                        .collect();
                    render_data.selected = 0;
                }
                return true;
            }
            KeyCode::Backspace => {
                render_data.query.pop();
                if let Some(ref items) = self.custom_items {
                    if render_data.query.is_empty() {
                        render_data.results = items.clone();
                    } else {
                        let query = render_data.query.to_lowercase();
                        render_data.results = items
                            .iter()
                            .filter(|item| item.to_lowercase().contains(&query))
                            .cloned()
                            .collect();
                    }
                    render_data.selected = 0;
                }
                return true;
            }
            KeyCode::Down => {
                if render_data.selected < render_data.results.len().saturating_sub(1) {
                    render_data.selected += 1;
                }
                return true;
            }
            KeyCode::Up => {
                if render_data.selected > 0 {
                    render_data.selected -= 1;
                }
                return true;
            }
            _ => {}
        }

        false
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

        // Custom items mode (buffer picker etc.) — handle locally without JS runtime
        if self.custom_items.is_some() {
            return self.handle_custom_key(key, ctx);
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
                                *ctx.mode = ctx.default_mode.clone();
                            }
                        }
                        JsPluginAction::OpenFile { path, line } => {
                            // Store as pending — the editor will handle this
                            // (plugin can't create new buffers or switch views)
                            self.pending_open = Some(PendingOpen { path, line });
                            *ctx.mode = ctx.default_mode.clone();
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
        self.render_popup(area, buf);
    }

    fn render_readonly(&self, area: Rect, buf: &mut Buffer, _ctx: &crate::plugins::PluginRenderContext) {
        self.render_popup(area, buf);
    }

    fn is_active(&self) -> bool {
        self.cached_render_data.borrow().active
    }

    fn deactivate(&mut self) {
        self.custom_items = None;
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
