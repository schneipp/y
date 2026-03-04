use std::fs;
use std::io;

use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    prelude::*,
};

use crate::buffer::{BufferPool, YBuffer, YLine};
use crate::layout::{SplitDirection, SplitNode};
use crate::mode::{Mode, YankRegister};
use crate::plugins;
use crate::render::buffer_widget::BufferWidget;
use crate::render::status_bar::StatusBar;
use crate::theme::ThemeManager;
use crate::view::View;

#[derive(Debug)]
pub struct Editor {
    pub exit: bool,
    pub buffer_pool: BufferPool,
    pub views: Vec<View>,
    pub active_view_idx: usize,
    pub mode: Mode,
    pub pending_key: Option<char>,
    pub undo_stack: Vec<YBuffer>,
    pub redo_stack: Vec<YBuffer>,
    pub yank_register: Option<YankRegister>,
    pub filename: Option<String>,
    pub command_buffer: String,
    pub modified: bool,
    pub space_pressed: bool,
    pub ctrl_w_pressed: bool,
    pub plugin_manager: plugins::PluginManager,
    pub theme_manager: ThemeManager,
    pub split_root: SplitNode,
    // Legacy fuzzy finder state (kept for compatibility)
    pub fuzzy_finder_type: Option<crate::mode::FuzzyFinderType>,
    pub fuzzy_query: String,
    pub fuzzy_results: Vec<String>,
    pub fuzzy_selected: usize,
    // Buffer picker: maps fuzzy finder selection index to BufferId
    pub buffer_picker_ids: Option<Vec<crate::buffer::BufferId>>,
    pub theme_picker_active: bool,
}

impl Editor {
    pub fn default() -> Self {
        let buffer = YBuffer::from(vec![YLine::new()]);

        let mut plugin_manager = plugins::PluginManager::new();
        plugin_manager.register(Box::new(
            plugins::syntax_highlighter::SyntaxHighlighter::new(),
        ));
        plugin_manager.register(Box::new(
            plugins::js_fuzzy_finder::JsFuzzyFinderPlugin::new(),
        ));

        let mut buffer_pool = BufferPool::new();
        let buffer_id = buffer_pool.add_with_filename(buffer, None);
        let view = View::new(0, buffer_id);

        let mut editor = Self {
            exit: false,
            buffer_pool,
            views: vec![view],
            active_view_idx: 0,
            mode: Mode::Normal,
            pending_key: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            yank_register: None,
            filename: None,
            command_buffer: String::new(),
            modified: false,
            space_pressed: false,
            ctrl_w_pressed: false,
            plugin_manager,
            theme_manager: ThemeManager::new(),
            split_root: SplitNode::single(0),
            fuzzy_finder_type: None,
            fuzzy_query: String::new(),
            fuzzy_results: Vec::new(),
            fuzzy_selected: 0,
            buffer_picker_ids: None,
            theme_picker_active: false,
        };
        editor.apply_theme_to_plugins();
        editor
    }

    pub fn from_file(filename: &str) -> io::Result<Self> {
        let content = match fs::read_to_string(filename) {
            Ok(content) => content,
            Err(_) => {
                let mut editor = Self::default();
                editor.filename = Some(filename.to_string());
                return Ok(editor);
            }
        };

        let lines: Vec<YLine> = if content.is_empty() {
            vec![YLine::new()]
        } else {
            content
                .lines()
                .map(|line| YLine::from(line.to_string()))
                .collect()
        };

        let mut editor = Self::default();
        *editor.buffer_pool.get_mut(0) = YBuffer::from(lines);
        editor.buffer_pool.get_entry_mut(0).filename = Some(filename.to_string());
        editor.filename = Some(filename.to_string());
        Ok(editor)
    }

    /// Push current theme colors into plugins
    fn apply_theme_to_plugins(&mut self) {
        let theme = self.theme_manager.current();
        let syntax_colors = theme.syntax.clone();
        let ui = &theme.ui;

        if let Some(plugin) = self.plugin_manager.get_mut("syntax_highlighter") {
            if let Some(hl) = plugin.as_any_mut()
                .downcast_mut::<crate::plugins::syntax_highlighter::SyntaxHighlighter>()
            {
                hl.set_syntax_colors(syntax_colors);
            }
        }

        if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(ff) = plugin.as_any_mut()
                .downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>()
            {
                ff.popup_colors = crate::plugins::js_fuzzy_finder::PopupColors {
                    border: ui.popup_border,
                    query: ui.popup_query,
                    selected_fg: ui.popup_selected_fg,
                    selected_bg: ui.popup_selected_bg,
                };
            }
        }
    }

    /// Switch to a named theme
    pub fn switch_theme(&mut self, name: &str) {
        if self.theme_manager.set_theme(name).is_ok() {
            self.apply_theme_to_plugins();
        }
    }

    /// Show theme picker via fuzzy finder
    pub fn show_theme_picker(&mut self) {
        let themes: Vec<String> = self.theme_manager.list().iter().map(|s| s.to_string()).collect();
        if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.activate_with_items(" Themes ", themes);
                self.mode = Mode::FuzzyFinder;
                // Use a sentinel to distinguish from buffer picker
                self.buffer_picker_ids = None;
                self.theme_picker_active = true;
            }
        }
    }

    pub fn active_view(&self) -> &View {
        &self.views[self.active_view_idx]
    }

    pub fn active_view_mut(&mut self) -> &mut View {
        &mut self.views[self.active_view_idx]
    }

    pub fn active_buffer(&self) -> &YBuffer {
        let view = &self.views[self.active_view_idx];
        self.buffer_pool.get(view.buffer_id)
    }

    pub fn active_buffer_mut(&mut self) -> &mut YBuffer {
        let buffer_id = self.views[self.active_view_idx].buffer_id;
        self.buffer_pool.get_mut(buffer_id)
    }

    // Mode transitions
    pub fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
    }

    pub fn enter_normal_mode(&mut self) {
        self.mode = Mode::Normal;
        let view = &mut self.views[self.active_view_idx];
        view.set_visual_start(None);
        // Collapse multi-cursor on Esc
        if view.has_multiple_cursors() {
            view.collapse_to_primary();
        }
    }

    pub fn enter_visual_mode(&mut self) {
        self.mode = Mode::Visual;
        let view = &mut self.views[self.active_view_idx];
        let row = view.cursor().row;
        let col = view.cursor().col;
        view.set_visual_start(Some((row, col)));
    }

    pub fn enter_visual_line_mode(&mut self) {
        self.mode = Mode::VisualLine;
        let view = &mut self.views[self.active_view_idx];
        let row = view.cursor().row;
        let col = view.cursor().col;
        view.set_visual_start(Some((row, col)));
    }

    pub fn enter_command_mode(&mut self) {
        self.mode = Mode::Command;
        self.command_buffer.clear();
    }

    // Split view operations
    pub fn split_horizontal(&mut self) {
        let current_view = &self.views[self.active_view_idx];
        let buffer_id = current_view.buffer_id;
        let current_view_id = current_view.id;

        let new_view_id = self.views.len();
        let mut new_view = View::new(new_view_id, buffer_id);
        // Copy cursor position from current view
        new_view.cursor_states = current_view.cursor_states.clone();
        new_view.scroll_offset = current_view.scroll_offset;
        self.views.push(new_view);

        self.split_root.split_view(current_view_id, new_view_id, SplitDirection::Horizontal);
        self.active_view_idx = self.views.iter().position(|v| v.id == new_view_id).unwrap();
    }

    pub fn split_vertical(&mut self) {
        let current_view = &self.views[self.active_view_idx];
        let buffer_id = current_view.buffer_id;
        let current_view_id = current_view.id;

        let new_view_id = self.views.len();
        let mut new_view = View::new(new_view_id, buffer_id);
        new_view.cursor_states = current_view.cursor_states.clone();
        new_view.scroll_offset = current_view.scroll_offset;
        self.views.push(new_view);

        self.split_root.split_view(current_view_id, new_view_id, SplitDirection::Vertical);
        self.active_view_idx = self.views.iter().position(|v| v.id == new_view_id).unwrap();
    }

    pub fn focus_next_view(&mut self) {
        let current_id = self.views[self.active_view_idx].id;
        if let Some(next_id) = self.split_root.next_view_after(current_id) {
            if let Some(idx) = self.views.iter().position(|v| v.id == next_id) {
                self.active_view_idx = idx;
            }
        }
    }

    pub fn focus_direction_left(&mut self) {
        // Simple: just cycle for now
        self.focus_next_view();
    }

    pub fn focus_direction_down(&mut self) {
        self.focus_next_view();
    }

    pub fn focus_direction_up(&mut self) {
        self.focus_next_view();
    }

    pub fn focus_direction_right(&mut self) {
        self.focus_next_view();
    }

    /// Open a file into a new buffer (or reuse existing buffer for the same file).
    /// Switches the active view to show that buffer.
    pub fn open_file_in_view(&mut self, filename: &str) {
        // Check if buffer already exists for this file
        let buffer_id = if let Some(id) = self.buffer_pool.find_by_filename(filename) {
            id
        } else {
            // Load file into new buffer
            let content = std::fs::read_to_string(filename).unwrap_or_default();
            let lines: Vec<YLine> = if content.is_empty() {
                vec![YLine::new()]
            } else {
                content
                    .lines()
                    .map(|line| YLine::from(line.to_string()))
                    .collect()
            };
            self.buffer_pool.add_with_filename(
                YBuffer::from(lines),
                Some(filename.to_string()),
            )
        };

        // Switch active view to this buffer
        let view = &mut self.views[self.active_view_idx];
        view.buffer_id = buffer_id;
        view.cursor_states[view.primary_cursor_idx].cursor = crate::cursor::Cursor::new();
        view.scroll_offset = 0;

        // Update editor-level filename to match active buffer
        self.filename = self.buffer_pool.get_entry(buffer_id).filename.clone();
        self.modified = self.buffer_pool.get_entry(buffer_id).modified;
    }

    /// Switch the active view to show a specific buffer by ID
    pub fn switch_to_buffer(&mut self, buffer_id: crate::buffer::BufferId) {
        let view = &mut self.views[self.active_view_idx];
        view.buffer_id = buffer_id;
        view.cursor_states[view.primary_cursor_idx].cursor = crate::cursor::Cursor::new();
        view.scroll_offset = 0;
        self.filename = self.buffer_pool.get_entry(buffer_id).filename.clone();
        self.modified = self.buffer_pool.get_entry(buffer_id).modified;
    }

    /// Show buffer picker via the fuzzy finder plugin
    pub fn show_buffer_picker(&mut self) {
        let buffer_list = self.buffer_pool.buffer_list();
        if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.activate_with_items(
                    " Open Buffers ",
                    buffer_list.iter().map(|(_, name)| name.clone()).collect(),
                );
                self.mode = Mode::FuzzyFinder;
                // Store buffer IDs for selection callback
                self.buffer_picker_ids = Some(buffer_list.iter().map(|(id, _)| *id).collect());
            }
        }
    }

    fn handle_buffer_picker_result(&mut self) {
        // Check if the fuzzy finder just deactivated (selection was made)
        // Note: can't use has_active_plugin() because syntax_highlighter is always active
        let selection_made = if let Some(plugin) = self.plugin_manager.get("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_ref().as_any().downcast_ref::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.is_custom_mode() && !fuzzy_plugin.cached_render_data.borrow().active
            } else {
                false
            }
        } else {
            false
        };

        if !selection_made {
            return;
        }

        // Get the selected index
        let selected_idx = if let Some(plugin) = self.plugin_manager.get("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_ref().as_any().downcast_ref::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.get_selected_index()
            } else {
                0
            }
        } else {
            0
        };

        // Map filtered selection back to original buffer list
        let selected_name = if let Some(plugin) = self.plugin_manager.get("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_ref().as_any().downcast_ref::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                let render_data = fuzzy_plugin.cached_render_data.borrow();
                render_data.results.get(selected_idx).cloned()
            } else {
                None
            }
        } else {
            None
        };

        if let (Some(_ids), Some(name)) = (&self.buffer_picker_ids, selected_name) {
            // Find the buffer ID matching the selected name
            let buffer_list = self.buffer_pool.buffer_list();
            if let Some((buffer_id, _)) = buffer_list.iter().find(|(_, n)| *n == name) {
                self.switch_to_buffer(*buffer_id);
            }
        }

        // Clean up
        self.buffer_picker_ids = None;
        if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.custom_items = None;
            }
        }
    }

    fn handle_theme_picker_result(&mut self) {
        let selection_made = if let Some(plugin) = self.plugin_manager.get("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_ref().as_any().downcast_ref::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.is_custom_mode() && !fuzzy_plugin.cached_render_data.borrow().active
            } else {
                false
            }
        } else {
            false
        };

        if !selection_made {
            return;
        }

        // Get selected theme name from filtered results
        let selected_name = if let Some(plugin) = self.plugin_manager.get("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_ref().as_any().downcast_ref::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                let idx = fuzzy_plugin.get_selected_index();
                let render_data = fuzzy_plugin.cached_render_data.borrow();
                render_data.results.get(idx).cloned()
            } else {
                None
            }
        } else {
            None
        };

        if let Some(name) = selected_name {
            self.switch_theme(&name);
        }

        // Clean up
        self.theme_picker_active = false;
        if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.custom_items = None;
            }
        }
    }

    /// Check if the fuzzy finder plugin has a pending file-open action and handle it
    fn handle_fuzzy_finder_open(&mut self) {
        let pending = if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.pending_open.take()
            } else {
                None
            }
        } else {
            None
        };

        if let Some(open) = pending {
            self.open_file_in_view(&open.path);
            if let Some(line_num) = open.line {
                let view = &mut self.views[self.active_view_idx];
                let buffer = self.buffer_pool.get(view.buffer_id);
                let row = line_num.min(buffer.lines.len().saturating_sub(1));
                view.cursor_states[view.primary_cursor_idx].cursor.row = row;
                view.cursor_states[view.primary_cursor_idx].cursor.col = 0;
                view.cursor_states[view.primary_cursor_idx].cursor.desired_col = 0;
            }
        }
    }

    pub fn close_current_view(&mut self) {
        let view_ids = self.split_root.view_ids();
        if view_ids.len() <= 1 {
            // Last view, quit
            self.exit = true;
            return;
        }

        let current_id = self.views[self.active_view_idx].id;
        self.split_root.remove_view(current_id);

        // Focus the next available view
        let remaining_ids = self.split_root.view_ids();
        if let Some(&first_id) = remaining_ids.first() {
            if let Some(idx) = self.views.iter().position(|v| v.id == first_id) {
                self.active_view_idx = idx;
            }
        }
    }

    // Main loop with event coalescing
    pub fn run(&mut self, terminal: &mut crate::tui::Tui) -> io::Result<()> {
        while !self.exit {
            let viewport_height = terminal.get_frame().size().height.saturating_sub(2) as usize;
            self.adjust_scroll(viewport_height);

            self.sync_syntax_highlights();
            terminal.draw(|frame| self.render_frame(frame))?;

            // Wait for at least one event
            self.handle_events()?;

            // Drain all pending events before re-rendering (event coalescing)
            while event::poll(Duration::ZERO)? {
                self.handle_events()?;
                if self.exit {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Ensure all visible buffers have up-to-date syntax highlights
    fn sync_syntax_highlights(&mut self) {
        // Collect visible buffer IDs
        let buffer_ids: Vec<crate::buffer::BufferId> = self.views.iter().map(|v| v.buffer_id).collect();

        if let Some(plugin) = self.plugin_manager.get_mut("syntax_highlighter") {
            if let Some(highlighter) = plugin.as_any_mut().downcast_mut::<crate::plugins::syntax_highlighter::SyntaxHighlighter>() {
                for &bid in &buffer_ids {
                    let buffer = self.buffer_pool.get(bid);
                    highlighter.parse_buffer(buffer, bid);
                }
            }
        }
    }

    fn render_frame(&self, frame: &mut ratatui::Frame) {
        let area = frame.size();
        let view_rects = self.split_root.compute_rects(area);

        for (view_id, rect) in &view_rects {
            if let Some(view) = self.views.iter().find(|v| v.id == *view_id) {
                let buffer = self.buffer_pool.get(view.buffer_id);
                let is_active = self.views[self.active_view_idx].id == *view_id;

                // Use per-buffer filename for each split
                let buf_entry = self.buffer_pool.get_entry(view.buffer_id);
                let view_filename = buf_entry.filename.clone().or_else(|| self.filename.clone());
                let view_modified = buf_entry.modified || self.modified;

                let theme = self.theme_manager.current();
                let status = StatusBar {
                    mode: &self.mode,
                    filename: &view_filename,
                    modified: view_modified,
                    cursor_row: view.cursor().row,
                    cursor_col: view.cursor().col,
                    char_num: view.cursor().get_character_number(buffer),
                    command_buffer: &self.command_buffer,
                    is_active,
                    theme,
                };
                status.render(*rect, frame.buffer_mut());

                // Inner area for buffer content
                let inner = Rect {
                    x: rect.x + 1,
                    y: rect.y + 1,
                    width: rect.width.saturating_sub(2),
                    height: rect.height.saturating_sub(2),
                };

                // Render buffer content
                let widget = BufferWidget {
                    buffer,
                    buffer_id: view.buffer_id,
                    view,
                    mode: &self.mode,
                    plugin_manager: &self.plugin_manager,
                    theme,
                    is_active,
                    show_line_numbers: true,
                };
                widget.render(inner, frame.buffer_mut());
            }
        }

        // Render active plugins using read-only context (no cloning)
        let active_view = &self.views[self.active_view_idx];
        let active_buffer = self.buffer_pool.get(active_view.buffer_id);
        let ctx = plugins::PluginRenderContext {
            buffer: active_buffer,
            cursor: active_view.cursor(),
            mode: &self.mode,
            filename: &self.filename,
        };
        self.plugin_manager.render_readonly(area, frame.buffer_mut(), &ctx);

        // Set terminal cursor position
        let active_view = &self.views[self.active_view_idx];
        let active_view_id = active_view.id;

        if self.mode == Mode::FuzzyFinder {
            if let Some(plugin) = self.plugin_manager.get("js_fuzzy_finder") {
                if let Some(fuzzy_plugin) = plugin.as_ref().as_any().downcast_ref::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                    let query_len = fuzzy_plugin.get_query_length();
                    let popup_width = (area.width as f32 * 0.8) as u16;
                    let popup_height = (area.height as f32 * 0.6) as u16;
                    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
                    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
                    let x = popup_x + 1 + 2 + query_len as u16;
                    let y = popup_y + 1;
                    frame.set_cursor(x, y);
                    return;
                }
            }
        }

        // Find the rect for the active view
        if let Some((_, rect)) = view_rects.iter().find(|(id, _)| *id == active_view_id) {
            let ln_width = if true { // show_line_numbers
                let max_line = self.buffer_pool.get(active_view.buffer_id).lines.len();
                let digits = if max_line == 0 { 1 } else { (max_line as f64).log10() as u16 + 1 };
                digits + 1
            } else {
                0
            };

            let x = rect.x + 1 + ln_width + active_view.cursor().col as u16;
            let y = rect.y + 1 + (active_view.cursor().row.saturating_sub(active_view.scroll_offset)) as u16;
            frame.set_cursor(x, y);
        }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Check if any plugin is active and should handle the event
        if self.plugin_manager.has_active_plugin() {
            let view = &mut self.views[self.active_view_idx];
            let primary_idx = view.primary_cursor_idx;
            let buffer_id = view.buffer_id;
            let buffer = self.buffer_pool.get_mut(buffer_id);
            let mut ctx = plugins::PluginContext {
                buffer,
                buffer_id,
                cursor: &mut view.cursor_states[primary_idx].cursor,
                mode: &mut self.mode,
                filename: &self.filename,
                modified: &mut self.modified,
            };

            if self.plugin_manager.handle_key(key_event, &mut ctx) {
                // Check if buffer picker just completed a selection
                if self.buffer_picker_ids.is_some() {
                    self.handle_buffer_picker_result();
                }
                // Check if theme picker just completed a selection
                if self.theme_picker_active {
                    self.handle_theme_picker_result();
                }
                // Check if fuzzy finder wants to open a file (only in active view)
                self.handle_fuzzy_finder_open();
                return;
            }
        }

        match self.mode {
            Mode::Normal => self.handle_normal_mode(key_event),
            Mode::Insert => self.handle_insert_mode(key_event),
            Mode::Visual => self.handle_visual_mode(key_event),
            Mode::VisualLine => self.handle_visual_line_mode(key_event),
            Mode::Command => self.handle_command_mode(key_event),
            Mode::FuzzyFinder => {}
        }
    }
}
