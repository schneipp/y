use std::fs;
use std::io;

use std::time::Duration;

use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    prelude::*,
};

use crossterm::event::KeyModifiers;
use ratatui::widgets::Clear;

use crate::buffer::{BufferPool, YBuffer, YLine};
use crate::completion::{self, CompletionState};
use crate::config::Config;
use crate::layout::{SplitDirection, SplitNode};
use crate::lsp::LspManager;
use crate::mode::{Mode, YankRegister};
use crate::plugins;
use crate::render::buffer_widget::BufferWidget;
use crate::render::status_bar::StatusBar;
use crate::theme::ThemeManager;
use crate::view::View;

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
    pub config: Config,
    pub lsp_manager: LspManager,
    pub lsp_picker_active: bool,
    pub completion: CompletionState,
    pub show_welcome: bool,
    pub rg_available: bool,
}

impl Editor {
    pub fn default() -> Self {
        let mut config = Config::load();
        config.ensure_known_servers();
        Self::with_config(config)
    }

    pub fn with_config(config: Config) -> Self {
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

        let rg_available = std::process::Command::new("rg")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok();

        let theme_name = config.theme.clone();
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
            config,
            lsp_manager: LspManager::new(),
            lsp_picker_active: false,
            completion: CompletionState::new(),
            show_welcome: true,
            rg_available,
        };
        editor.switch_theme(&theme_name);
        editor.apply_theme_to_plugins();
        editor
    }

    pub fn from_file(filename: &str) -> io::Result<Self> {
        let mut config = Config::load();
        config.ensure_known_servers();
        Self::from_file_with_config(filename, config)
    }

    pub fn from_file_with_config(filename: &str, config: Config) -> io::Result<Self> {
        let content = match fs::read_to_string(filename) {
            Ok(content) => content,
            Err(_) => {
                let mut editor = Self::with_config(config);
                editor.filename = Some(filename.to_string());
                editor.show_welcome = false;
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

        let mut editor = Self::with_config(config);
        editor.show_welcome = false;
        *editor.buffer_pool.get_mut(0) = YBuffer::from(lines);
        editor.buffer_pool.get_entry_mut(0).filename = Some(filename.to_string());
        editor.filename = Some(filename.to_string());

        // Start LSP server for this file if applicable
        if let Some(ext) = std::path::Path::new(filename).extension().and_then(|e| e.to_str()) {
            let root = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());
            editor.lsp_manager.ensure_server_for_extension(ext, &editor.config, &root);

            // Send didOpen if server config exists
            if let Some(server_config) = editor.config.server_for_extension(ext) {
                let server_name = server_config.name.clone();
                let language = server_config.language.clone();
                let abs_path = std::fs::canonicalize(filename)
                    .unwrap_or_else(|_| std::path::PathBuf::from(filename));
                let uri = format!("file://{}", abs_path.display());
                let text: String = editor.buffer_pool.get(0).lines.iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                editor.lsp_manager.did_open(&server_name, &uri, &language, &text);
            }
        }

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
            self.config.theme = name.to_string();
            self.config.save();
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

    /// VSCode Ctrl+D behavior:
    /// - In normal mode: select the word under cursor (enters visual mode)
    /// - In visual mode: find next occurrence of current selection, add cursor there
    pub fn select_word_or_next_match(&mut self) {
        if self.mode == Mode::Visual {
            self.add_visual_cursor_at_next_match();
            return;
        }

        // Normal mode: select word under cursor → enter visual mode
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let primary = &view.cursor_states[view.primary_cursor_idx].cursor;
        let row = primary.row;
        let col = primary.col;

        if row >= buffer.lines.len() {
            return;
        }
        let line = &buffer.lines[row].text;
        let chars: Vec<char> = line.chars().collect();
        if col >= chars.len() {
            return;
        }

        // Find word boundaries
        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        let mut end = col;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        if start == end {
            return;
        }

        // Select the word: visual_start at word start, cursor at word end
        let cs = &mut view.cursor_states[view.primary_cursor_idx];
        cs.visual_start = Some((row, start));
        cs.cursor.col = end - 1;
        cs.cursor.desired_col = end - 1;
        self.mode = Mode::Visual;
    }

    /// Ctrl+N in visual mode: find next occurrence of the selection and add a cursor there.
    pub fn add_visual_cursor_at_next_match(&mut self) {
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);

        // Get the selected text from the primary cursor
        let needle = match view.get_selection_text(view.primary_cursor_idx, buffer) {
            Some(t) => t,
            None => return,
        };

        view.add_cursor_at_next_selection_match(buffer, &needle);
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

        // Start LSP server for this file if applicable
        if let Some(ext) = std::path::Path::new(filename).extension().and_then(|e| e.to_str()) {
            let root = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());
            self.lsp_manager.ensure_server_for_extension(ext, &self.config, &root);

            if let Some(server_config) = self.config.server_for_extension(ext) {
                let server_name = server_config.name.clone();
                let language = server_config.language.clone();
                let abs_path = std::fs::canonicalize(filename)
                    .unwrap_or_else(|_| std::path::PathBuf::from(filename));
                let uri = format!("file://{}", abs_path.display());
                let buffer = self.buffer_pool.get(buffer_id);
                let text: String = buffer.lines.iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.lsp_manager.did_open(&server_name, &uri, &language, &text);
            }
        }
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

    /// Show LSP debug info via fuzzy finder modal
    pub fn show_lsp_info(&mut self) {
        let items = self.lsp_manager.debug_info();
        if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.activate_with_items(" LSP Info ", items);
                self.mode = Mode::FuzzyFinder;
                self.buffer_picker_ids = None;
                self.theme_picker_active = false;
                self.lsp_picker_active = false;
            }
        }
    }

    /// Show LSP setup picker via fuzzy finder
    pub fn show_lsp_setup(&mut self) {
        let items: Vec<String> = self.config.lsp.servers.iter().map(|s| {
            let status = if crate::lsp::types::is_binary_available(&s.binary) {
                if s.enabled { "enabled" } else { "disabled" }
            } else {
                "not found"
            };
            format!("{} ({}) [{}]", s.name, s.language, status)
        }).collect();

        if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.activate_with_items(" LSP Servers ", items);
                self.mode = Mode::FuzzyFinder;
                self.buffer_picker_ids = None;
                self.theme_picker_active = false;
                self.lsp_picker_active = true;
            }
        }
    }

    fn handle_lsp_picker_result(&mut self) {
        // Check if fuzzy finder was dismissed (Esc clears custom_items)
        let was_cancelled = if let Some(plugin) = self.plugin_manager.get("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_ref().as_any().downcast_ref::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                !fuzzy_plugin.is_custom_mode()
            } else {
                false
            }
        } else {
            false
        };

        if was_cancelled {
            self.lsp_picker_active = false;
            return;
        }

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

        // Get selected item from filtered results
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

        if let Some(display_str) = selected_name {
            // Parse server name from "name (language) [status]"
            if let Some(server_name) = display_str.split(' ').next() {
                if let Some(server) = self.config.lsp.servers.iter_mut().find(|s| s.name == server_name) {
                    server.enabled = !server.enabled;
                    self.config.save();
                }
            }
        }

        // Clean up fuzzy finder state
        if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
            if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                fuzzy_plugin.custom_items = None;
            }
        }

        // Re-open the picker to show updated state
        self.show_lsp_setup();
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
            self.lsp_manager.poll();
            self.process_lsp_responses();

            let viewport_height = terminal.get_frame().size().height.saturating_sub(2) as usize;
            self.adjust_scroll(viewport_height);

            self.sync_syntax_highlights();
            terminal.draw(|frame| self.render_frame(frame))?;

            // Poll with timeout so LSP messages are processed even while idle
            if event::poll(Duration::from_millis(100))? {
                self.handle_events()?;

                // Drain all pending events before re-rendering (event coalescing)
                while event::poll(Duration::ZERO)? {
                    self.handle_events()?;
                    if self.exit {
                        break;
                    }
                }
            }
        }
        self.lsp_manager.shutdown_all();
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

                // Compute LSP status for this view's buffer
                let lsp_status_string = buf_entry.filename.as_ref().and_then(|f| {
                    std::path::Path::new(f)
                        .extension()
                        .and_then(|e| e.to_str())
                        .and_then(|ext| {
                            self.lsp_manager
                                .status_for_extension(ext, &self.config)
                                .map(|(name, st)| format!("{}: {}", name, st))
                        })
                });

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
                    lsp_status: lsp_status_string.as_deref(),
                };
                status.render(*rect, frame.buffer_mut());

                // Inner area for buffer content
                let inner = Rect {
                    x: rect.x + 1,
                    y: rect.y + 1,
                    width: rect.width.saturating_sub(2),
                    height: rect.height.saturating_sub(2),
                };

                // Ghost text only for the active view in insert mode
                let ghost = if is_active && self.mode == Mode::Insert {
                    self.completion.ghost_text()
                } else {
                    None
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
                    ghost_text: ghost,
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

        // Render completion popup
        if self.completion.active && !self.completion.filtered.is_empty() {
            let active_view_id = self.views[self.active_view_idx].id;
            if let Some((_, rect)) = view_rects.iter().find(|(id, _)| *id == active_view_id) {
                self.render_completion_popup(frame.buffer_mut(), rect, area);
            }
        }

        // Render welcome screen
        if self.show_welcome {
            self.render_welcome(area, frame.buffer_mut());
        }

        // Hide cursor behind welcome screen
        if self.show_welcome {
            return;
        }

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

        // Set cursor shape based on mode (block in normal, bar in insert)
        let cursor_style = match self.mode {
            Mode::Insert => SetCursorStyle::SteadyBar,
            Mode::Command | Mode::FuzzyFinder => SetCursorStyle::SteadyBar,
            _ => SetCursorStyle::SteadyBlock,
        };
        let _ = crossterm::execute!(std::io::stdout(), cursor_style);
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
        if self.show_welcome {
            self.show_welcome = false;
            // Let the keypress fall through to normal handling
        }

        // Ctrl+L accepts ghost text (first suggestion) — works even without popup visible
        // This keybinding is reserved for AI completions in the future.
        if self.mode == Mode::Insert
            && key_event.code == crossterm::event::KeyCode::Char('l')
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            if self.completion.ghost_text().is_some() {
                self.accept_ghost_text();
                return;
            }
        }

        // Handle completion popup keys when popup is active
        if self.completion.active {
            match key_event.code {
                crossterm::event::KeyCode::Enter => {
                    self.accept_completion();
                    return;
                }
                crossterm::event::KeyCode::Char('n')
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.completion.navigate(1);
                    return;
                }
                crossterm::event::KeyCode::Char('p')
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.completion.navigate(-1);
                    return;
                }
                crossterm::event::KeyCode::Esc => {
                    self.completion.dismiss();
                    // Fall through to mode handler
                }
                crossterm::event::KeyCode::Char(_) | crossterm::event::KeyCode::Backspace => {
                    // Let chars and backspace through; we'll update the filter after mode dispatch
                }
                _ => {
                    self.completion.dismiss();
                }
            }
        }

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
                // Check if LSP picker just completed a selection
                if self.lsp_picker_active {
                    self.handle_lsp_picker_result();
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

        // After insert mode keystrokes, update completion
        if self.mode == Mode::Insert {
            match key_event.code {
                crossterm::event::KeyCode::Char(_) | crossterm::event::KeyCode::Backspace => {
                    self.post_insert_completion_update();
                }
                _ => {}
            }
        }
    }

    // ── Completion helpers ──────────────────────────────────────────────

    /// Get the word prefix at the cursor position. Returns (word_start_col, prefix_string).
    fn get_word_prefix_at_cursor(&self) -> (usize, String) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let row = view.cursor().row;
        let col = view.cursor().col;

        if row >= buffer.lines.len() {
            return (col, String::new());
        }

        let line = &buffer.lines[row].text;
        let end = col.min(line.len());
        let before_cursor = &line[..end];

        let word_start = before_cursor
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        let prefix = before_cursor[word_start..].to_string();
        (word_start, prefix)
    }

    /// Called after each char/backspace in insert mode to request or update completions.
    fn post_insert_completion_update(&mut self) {
        let (word_start, prefix) = self.get_word_prefix_at_cursor();
        let view = &self.views[self.active_view_idx];
        let cursor_row = view.cursor().row;

        // If popup is active, update the filter with the new prefix
        if self.completion.active {
            if cursor_row != self.completion.trigger_row {
                self.completion.dismiss();
            } else {
                self.completion.update_prefix(&prefix);
            }
        }

        // Request new completions from LSP if we have a prefix
        if prefix.is_empty() {
            if !self.completion.active {
                return;
            }
        }

        self.request_lsp_completion(cursor_row, word_start, &prefix);
    }

    /// Send didChange + textDocument/completion to the LSP server.
    fn request_lsp_completion(&mut self, cursor_row: usize, word_start: usize, prefix: &str) {
        let view = &self.views[self.active_view_idx];
        let buffer_id = view.buffer_id;
        let cursor_col = view.cursor().col;

        // Get filename and extension
        let filename = match self.buffer_pool.get_entry(buffer_id).filename.clone() {
            Some(f) => f,
            None => return,
        };
        let ext = match std::path::Path::new(&filename)
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

        let abs_path = std::fs::canonicalize(&filename)
            .unwrap_or_else(|_| std::path::PathBuf::from(&filename));
        let uri = format!("file://{}", abs_path.display());

        // Send didChange with full buffer text
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

        // Send completion request
        if let Some(id) =
            self.lsp_manager
                .request_completion(&server_config.name, &uri, cursor_row, cursor_col)
        {
            self.completion.pending_request_id = Some(id);
            self.completion.trigger_row = cursor_row;
            self.completion.trigger_col = word_start;
            self.completion.prefix = prefix.to_string();
        }
    }

    /// Process LSP responses and route completion results to CompletionState.
    fn process_lsp_responses(&mut self) {
        let responses: Vec<_> = self.lsp_manager.pending_responses.drain(..).collect();
        for resp in responses {
            // Check if this is a completion response we're waiting for
            if self.completion.pending_request_id == Some(resp.id) {
                self.completion.pending_request_id = None;
                if let Some(result) = resp.result {
                    let items = parse_completion_response(&result);
                    if !items.is_empty() {
                        let (word_start, prefix) = self.get_word_prefix_at_cursor();
                        let view = &self.views[self.active_view_idx];
                        let cursor_row = view.cursor().row;
                        self.completion
                            .activate(items, cursor_row, word_start, &prefix);
                    }
                }
            }
        }
    }

    /// Accept the ghost text (first filtered completion item) via Ctrl+L.
    fn accept_ghost_text(&mut self) {
        // Ghost text is always the first filtered item
        let item = match self.completion.filtered.first()
            .and_then(|&idx| self.completion.items.get(idx))
            .cloned()
        {
            Some(i) => i,
            None => {
                self.completion.dismiss();
                return;
            }
        };

        let text = completion::strip_snippets(item.text_to_insert());
        let trigger_col = self.completion.trigger_col;
        let trigger_row = self.completion.trigger_row;

        let view = &self.views[self.active_view_idx];
        let cursor_row = view.cursor().row;
        let cursor_col = view.cursor().col;

        if cursor_row != trigger_row {
            self.completion.dismiss();
            return;
        }

        let buffer = self.active_buffer_mut();
        if trigger_row < buffer.lines.len() {
            let line = &buffer.lines[trigger_row].text;
            let before = line[..trigger_col.min(line.len())].to_string();
            let after = line[cursor_col.min(line.len())..].to_string();
            buffer.lines[trigger_row].text = format!("{}{}{}", before, text, after);
        }

        let new_col = trigger_col + text.len();
        let view = &mut self.views[self.active_view_idx];
        let primary = view.primary_cursor_idx;
        view.cursor_states[primary].cursor.col = new_col;
        view.cursor_states[primary].cursor.desired_col = new_col;

        self.completion.dismiss();
    }

    /// Accept the currently selected completion item.
    fn accept_completion(&mut self) {
        let item = match self.completion.selected_item().cloned() {
            Some(i) => i,
            None => {
                self.completion.dismiss();
                return;
            }
        };

        let text = completion::strip_snippets(item.text_to_insert());
        let trigger_col = self.completion.trigger_col;
        let trigger_row = self.completion.trigger_row;

        let view = &self.views[self.active_view_idx];
        let cursor_row = view.cursor().row;
        let cursor_col = view.cursor().col;

        if cursor_row != trigger_row {
            self.completion.dismiss();
            return;
        }

        // Replace from trigger_col to cursor_col with completion text
        let buffer = self.active_buffer_mut();
        if trigger_row < buffer.lines.len() {
            let line = &buffer.lines[trigger_row].text;
            let before = line[..trigger_col.min(line.len())].to_string();
            let after = line[cursor_col.min(line.len())..].to_string();
            buffer.lines[trigger_row].text = format!("{}{}{}", before, text, after);
        }

        // Move cursor to end of inserted text
        let new_col = trigger_col + text.len();
        let view = &mut self.views[self.active_view_idx];
        let primary = view.primary_cursor_idx;
        view.cursor_states[primary].cursor.col = new_col;
        view.cursor_states[primary].cursor.desired_col = new_col;

        self.completion.dismiss();
    }

    /// Render the completion popup near the cursor.
    fn render_welcome(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::text::{Line, Span, Text};
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::style::Style;

        let theme = self.theme_manager.current();
        let accent = theme.ui.popup_border;
        let dim = theme.ui.line_number_fg;
        let fg = theme.ui.foreground;

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  y editor",
            Style::default().fg(accent).add_modifier(ratatui::style::Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        if !self.rg_available {
            lines.push(Line::from(Span::styled(
                "  Warning: 'rg' (ripgrep) not found",
                Style::default().fg(ratatui::style::Color::Red),
            )));
            lines.push(Line::from(Span::styled(
                "  Fuzzy finder requires ripgrep to work",
                Style::default().fg(ratatui::style::Color::Red),
            )));
            lines.push(Line::from(""));
        }

        let shortcuts = [
            ("  Space f f", "  Find file"),
            ("  Space /  ", "  Grep in project"),
            ("  Space b b", "  Buffer picker"),
            ("  Space f t", "  Switch theme"),
            ("", ""),
            ("  :e <file>", "  Open file"),
            ("  :w       ", "  Save"),
            ("  :q       ", "  Quit"),
            ("  :wq      ", "  Save & quit"),
            ("", ""),
            ("  :sp      ", "  Split horizontal"),
            ("  :vs      ", "  Split vertical"),
            ("  Ctrl+W q ", "  Close split"),
            ("", ""),
            ("  i        ", "  Insert mode"),
            ("  v / V    ", "  Visual / Visual Line"),
            ("  u        ", "  Undo"),
            ("  Ctrl+N   ", "  Multi-cursor select"),
        ];

        for (key, desc) in &shortcuts {
            if key.is_empty() {
                lines.push(Line::from(""));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(*key, Style::default().fg(accent)),
                    Span::styled(*desc, Style::default().fg(dim)),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press any key to start",
            Style::default().fg(dim),
        )));

        let text = Text::from(lines);
        let height = text.lines.len() as u16 + 2;
        let width = 40u16;
        let popup_x = area.width.saturating_sub(width) / 2;
        let popup_y = area.height.saturating_sub(height) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: width.min(area.width),
            height: height.min(area.height),
        };

        Clear.render(popup_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent))
            .style(Style::default().bg(theme.ui.background).fg(fg));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);
        Paragraph::new(text).render(inner, buf);
    }

    fn render_completion_popup(
        &self,
        buf: &mut ratatui::buffer::Buffer,
        view_rect: &Rect,
        area: Rect,
    ) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let theme = self.theme_manager.current();

        // Line number gutter width
        let max_line = buffer.lines.len();
        let digits = if max_line == 0 {
            1
        } else {
            (max_line as f64).log10() as u16 + 1
        };
        let ln_width = digits + 1;

        let cursor_screen_y =
            view_rect.y + 1 + (view.cursor().row.saturating_sub(view.scroll_offset)) as u16;
        let popup_x = view_rect.x + 1 + ln_width + self.completion.trigger_col as u16;

        let max_visible = 10usize;
        let visible_count = self.completion.filtered.len().min(max_visible);
        let popup_height = visible_count as u16 + 2; // borders

        // Width based on longest label
        let max_label_len = self
            .completion
            .filtered
            .iter()
            .filter_map(|&idx| self.completion.items.get(idx))
            .map(|item| {
                let kind_len = item.kind.map(|k| k.short_label().len() + 1).unwrap_or(0);
                item.label.len() + kind_len
            })
            .max()
            .unwrap_or(20);
        let popup_width = (max_label_len as u16 + 4).max(15).min(60); // +4 for borders+padding

        // Position: below cursor, or above if not enough space
        let popup_y = if cursor_screen_y + 1 + popup_height <= area.height {
            cursor_screen_y + 1
        } else {
            cursor_screen_y.saturating_sub(popup_height)
        };

        let popup_x = popup_x.min(area.width.saturating_sub(popup_width));

        let popup_rect = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width.min(area.width.saturating_sub(popup_x)),
            height: popup_height.min(area.height.saturating_sub(popup_y)),
        };

        // Clear background
        Clear.render(popup_rect, buf);

        // Draw border
        let block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(theme.ui.popup_border))
            .style(Style::default().bg(theme.ui.background));
        let inner = block.inner(popup_rect);
        block.render(popup_rect, buf);

        // Calculate scroll offset for the list
        let scroll_offset = if self.completion.selected >= max_visible {
            self.completion.selected - max_visible + 1
        } else {
            0
        };

        // Render items
        let inner_width = inner.width as usize;
        for (i, &item_idx) in self
            .completion
            .filtered
            .iter()
            .skip(scroll_offset)
            .take(visible_count)
            .enumerate()
        {
            if let Some(item) = self.completion.items.get(item_idx) {
                let y = inner.y + i as u16;
                let is_selected = scroll_offset + i == self.completion.selected;

                let style = if is_selected {
                    Style::default()
                        .fg(theme.ui.popup_selected_fg)
                        .bg(theme.ui.popup_selected_bg)
                } else {
                    Style::default()
                        .fg(theme.ui.foreground)
                        .bg(theme.ui.background)
                };

                // Build the display line: "kd label"
                let kind_str = item
                    .kind
                    .map(|k| format!("{} ", k.short_label()))
                    .unwrap_or_default();
                let display = format!(
                    " {}{}",
                    kind_str,
                    &item.label[..item.label.len().min(inner_width.saturating_sub(kind_str.len() + 1))]
                );

                // Pad to full width
                let padded = format!("{:<width$}", display, width = inner_width);
                buf.set_string(inner.x, y, &padded, style);
            }
        }
    }
}

/// Parse LSP completion response into CompletionItems.
fn parse_completion_response(result: &serde_json::Value) -> Vec<completion::CompletionItem> {
    // Response can be CompletionList { items: [...] } or directly [...]
    let items_value = if let Some(items) = result.get("items") {
        items
    } else if result.is_array() {
        result
    } else {
        return Vec::new();
    };

    items_value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(completion::CompletionItem::from_lsp)
                .collect()
        })
        .unwrap_or_default()
}
