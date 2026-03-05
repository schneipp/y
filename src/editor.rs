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
use crate::config::{Config, EditorMode};
use crate::layout::{SplitDirection, SplitNode};
use crate::lsp::LspManager;
use crate::mode::{Mode, YankRegister};
use crate::plugins::{self, Plugin as _};
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
    pub key_registry: crate::keybindings::KeybindingRegistry,
    pub pending_keys: Vec<crate::keybindings::KeyCombo>,
    pub awaiting_char_action: Option<crate::keybindings::Action>,
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
    pub show_mode_selector: bool,
    pub editor_mode: EditorMode,
    pub show_keybindings: bool,
    pub rg_available: bool,
    pub search_query: String,
    pub search_buffer: String,
    pub search_direction: SearchDirection,
    pub jump_list: Vec<JumpLocation>,
    pub jump_list_idx: usize,
    pub pending_definition_request: Option<i64>,
    pub show_settings: bool,
    pub settings_selected: usize,
    pub settings_scroll: usize,
}

#[derive(Debug, Clone)]
pub struct JumpLocation {
    pub filename: Option<String>,
    pub buffer_id: crate::buffer::BufferId,
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum SettingsItemKind {
    EditorMode,
    Theme,
    LspServer(String),
    Separator,
}

#[derive(Debug, Clone)]
struct SettingsItem {
    label: String,
    value: String,
    kind: SettingsItemKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchDirection {
    Forward,
    Backward,
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
        plugin_manager.register(Box::new(
            plugins::git_client::GitClientPlugin::new(),
        ));
        plugin_manager.register(Box::new(
            plugins::file_tree::FileTreePlugin::new(),
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
        let editor_mode = config.editor_mode.clone().unwrap_or(EditorMode::Vim);
        let show_mode_selector = config.editor_mode.is_none();
        let initial_mode = match editor_mode {
            EditorMode::Vim => Mode::Normal,
            EditorMode::Normie => Mode::Normie,
        };
        let mut editor = Self {
            exit: false,
            buffer_pool,
            views: vec![view],
            active_view_idx: 0,
            mode: initial_mode,
            pending_key: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            yank_register: None,
            filename: None,
            command_buffer: String::new(),
            modified: false,
            space_pressed: false,
            ctrl_w_pressed: false,
            key_registry: config.build_registry(),
            pending_keys: Vec::new(),
            awaiting_char_action: None,
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
            show_mode_selector,
            editor_mode,
            show_keybindings: false,
            rg_available,
            search_query: String::new(),
            search_buffer: String::new(),
            search_direction: SearchDirection::Forward,
            jump_list: Vec::new(),
            jump_list_idx: 0,
            pending_definition_request: None,
            show_settings: false,
            settings_selected: 0,
            settings_scroll: 0,
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

    /// The base editing mode: Normal for vim, Normie for normie.
    pub fn default_mode(&self) -> Mode {
        match self.editor_mode {
            EditorMode::Vim => Mode::Normal,
            EditorMode::Normie => Mode::Normie,
        }
    }

    // Mode transitions
    pub fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
    }

    pub fn enter_normal_mode(&mut self) {
        self.mode = self.default_mode();
        self.search_query.clear();
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

    pub fn enter_search_mode(&mut self) {
        self.mode = Mode::Search;
        self.search_buffer.clear();
        self.command_buffer.clear();
        self.search_direction = SearchDirection::Forward;
    }

    pub fn handle_search_mode(&mut self, key_event: crossterm::event::KeyEvent) {
        match key_event.code {
            crossterm::event::KeyCode::Esc => {
                self.mode = self.default_mode();
                self.search_buffer.clear();
                self.command_buffer.clear();
            }
            crossterm::event::KeyCode::Enter => {
                self.search_query = self.search_buffer.clone();
                self.search_buffer.clear();
                self.command_buffer.clear();
                self.mode = self.default_mode();
                if !self.search_query.is_empty() {
                    self.jump_to_next_search_match();
                }
            }
            crossterm::event::KeyCode::Backspace => {
                self.search_buffer.pop();
                self.command_buffer = self.search_buffer.clone();
            }
            crossterm::event::KeyCode::Char(c) => {
                self.search_buffer.push(c);
                self.command_buffer = self.search_buffer.clone();
            }
            _ => {}
        }
    }

    pub fn search_next(&mut self) {
        if !self.search_query.is_empty() {
            self.search_direction = SearchDirection::Forward;
            self.jump_to_next_search_match();
        }
    }

    pub fn search_prev(&mut self) {
        if !self.search_query.is_empty() {
            self.search_direction = SearchDirection::Backward;
            self.jump_to_prev_search_match();
        }
    }

    fn select_search_match(&mut self, row: usize, col: usize) {
        let match_end = col + self.search_query.chars().count().saturating_sub(1);
        let view = &mut self.views[self.active_view_idx];
        view.collapse_to_primary();
        view.set_visual_start(Some((row, col)));
        view.cursor_mut().row = row;
        view.cursor_mut().col = match_end;
        view.cursor_mut().desired_col = match_end;
        self.mode = Mode::Visual;
    }

    fn jump_to_next_search_match(&mut self) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let start_row = view.cursor().row;
        let start_col = view.cursor().col + 1;
        let query = &self.search_query;

        // Search from cursor position forward
        for r in start_row..buffer.lines.len() {
            let yline = &buffer.lines[r];
            let search_from_char = if r == start_row { start_col } else { 0 };
            let search_from_byte = yline.char_to_byte(search_from_char);
            if let Some(byte_pos) = yline.text[search_from_byte..].find(query.as_str()) {
                let found_byte = search_from_byte + byte_pos;
                let found_col = yline.text[..found_byte].chars().count();
                self.select_search_match(r, found_col);
                return;
            }
        }

        // Wrap around from beginning
        for r in 0..=start_row.min(buffer.lines.len().saturating_sub(1)) {
            let yline = &buffer.lines[r];
            let search_to = if r == start_row {
                yline.char_to_byte(start_col.saturating_sub(1))
            } else {
                yline.text.len()
            };
            if let Some(byte_pos) = yline.text[..search_to].find(query.as_str()) {
                let found_col = yline.text[..byte_pos].chars().count();
                self.select_search_match(r, found_col);
                return;
            }
        }
    }

    fn jump_to_prev_search_match(&mut self) {
        let view = &self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let start_row = view.cursor().row;
        let start_col = view.cursor().col;
        let query = &self.search_query;

        // Search backward from cursor position
        for r in (0..=start_row).rev() {
            let yline = &buffer.lines[r];
            let search_up_to_byte = if r == start_row {
                yline.char_to_byte(start_col)
            } else {
                yline.text.len()
            };
            if let Some(byte_pos) = yline.text[..search_up_to_byte].rfind(query.as_str()) {
                let found_col = yline.text[..byte_pos].chars().count();
                self.select_search_match(r, found_col);
                return;
            }
        }

        // Wrap around from end
        for r in (start_row..buffer.lines.len()).rev() {
            let yline = &buffer.lines[r];
            let search_from_byte = if r == start_row {
                yline.char_to_byte(start_col)
            } else {
                0
            };
            if let Some(byte_pos) = yline.text[search_from_byte..].rfind(query.as_str()) {
                let found_byte = search_from_byte + byte_pos;
                let found_col = yline.text[..found_byte].chars().count();
                self.select_search_match(r, found_col);
                return;
            }
        }
    }

    fn compute_search_info(&self, view: &View, buffer: &YBuffer) -> (usize, usize) {
        let query = &self.search_query;
        // Use visual_start if available (match start), otherwise cursor position
        let (cursor_row, cursor_col) = view.visual_start()
            .unwrap_or((view.cursor().row, view.cursor().col));
        let mut total = 0;
        let mut current = 0;
        let mut found_exact = false;

        for (r, yline) in buffer.lines.iter().enumerate() {
            let mut search_from = 0;
            while let Some(byte_pos) = yline.text[search_from..].find(query.as_str()) {
                let match_byte = search_from + byte_pos;
                let match_col = yline.text[..match_byte].chars().count();
                total += 1;
                if r < cursor_row || (r == cursor_row && match_col <= cursor_col) {
                    current += 1;
                }
                if r == cursor_row && match_col == cursor_col {
                    found_exact = true;
                }
                search_from = match_byte + query.len();
            }
        }

        if !found_exact && current == 0 && total > 0 {
            current = total;
        }
        (current, total)
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

    fn handle_file_tree_open(&mut self) {
        let pending = if let Some(plugin) = self.plugin_manager.get_mut("file_tree") {
            if let Some(tree_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::file_tree::FileTreePlugin>() {
                tree_plugin.pending_open.take()
            } else {
                None
            }
        } else {
            None
        };

        if let Some(open) = pending {
            self.open_file_in_view(&open.path);
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

                let search_info = if !self.search_query.is_empty() {
                    let (current, total) = self.compute_search_info(view, buffer);
                    Some(format!("{}/{}", current, total))
                } else {
                    None
                };

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
                    search_info,
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
                let ghost = if is_active && matches!(self.mode, Mode::Insert | Mode::Normie) {
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
                    search_query: &self.search_query,
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

        // Render mode selector or welcome screen
        if self.show_mode_selector {
            self.render_mode_selector(area, frame.buffer_mut());
            return;
        }
        if self.show_welcome {
            self.render_welcome(area, frame.buffer_mut());
            return;
        }
        if self.show_keybindings {
            self.render_keybindings_help(area, frame.buffer_mut());
        }
        if self.show_settings {
            self.render_settings(area, frame.buffer_mut());
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

            let cursor_col = active_view.cursor().col;
            let cursor_row = active_view.cursor().row;
            let buffer = self.buffer_pool.get(active_view.buffer_id);
            let visual_col = if cursor_row < buffer.lines.len() {
                buffer.lines[cursor_row].visual_col(cursor_col, 4)
            } else {
                cursor_col
            };
            let x = rect.x + 1 + ln_width + visual_col as u16;
            let y = rect.y + 1 + (cursor_row.saturating_sub(active_view.scroll_offset)) as u16;
            frame.set_cursor(x, y);
        }

        // Set cursor shape based on mode (block in normal, bar in insert)
        let cursor_style = match self.mode {
            Mode::Insert | Mode::Normie => SetCursorStyle::SteadyBar,
            Mode::Command | Mode::Search | Mode::FuzzyFinder => SetCursorStyle::SteadyBar,
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
        // Mode selector: 'v' for vim, any other key for normie
        if self.show_mode_selector {
            self.show_mode_selector = false;
            self.show_welcome = false;
            let chosen = if key_event.code == crossterm::event::KeyCode::Char('v') {
                EditorMode::Vim
            } else {
                EditorMode::Normie
            };
            self.editor_mode = chosen.clone();
            self.mode = self.default_mode();
            // Persist the choice
            self.config.editor_mode = Some(chosen);
            self.config.save();
            return;
        }

        if self.show_keybindings {
            // Any key dismisses the help popup (F1 toggles via action, Esc/other just close)
            if key_event.code != crossterm::event::KeyCode::F(1) {
                self.show_keybindings = false;
            } else {
                // F1 again toggles off
                self.show_keybindings = false;
            }
            return;
        }

        if self.show_settings {
            self.handle_settings_key(key_event);
            return;
        }

        if self.show_welcome {
            self.show_welcome = false;
            // Let the keypress fall through to normal handling
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
            let dm = self.default_mode();
            let view = &mut self.views[self.active_view_idx];
            let primary_idx = view.primary_cursor_idx;
            let buffer_id = view.buffer_id;
            let buffer = self.buffer_pool.get_mut(buffer_id);
            let mut ctx = plugins::PluginContext {
                buffer,
                buffer_id,
                cursor: &mut view.cursor_states[primary_idx].cursor,
                mode: &mut self.mode,
                default_mode: dm,
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
                // Check if file tree wants to open a file
                self.handle_file_tree_open();
                return;
            }
        }

        match self.mode {
            Mode::Normal | Mode::Insert | Mode::Visual | Mode::VisualLine | Mode::Normie => {
                self.dispatch_via_registry(key_event);
            }
            Mode::Command => self.handle_command_mode(key_event),
            Mode::Search => self.handle_search_mode(key_event),
            Mode::FuzzyFinder => {}
        }

        // After insert/normie mode keystrokes, update completion
        if self.mode == Mode::Insert || self.mode == Mode::Normie {
            match key_event.code {
                crossterm::event::KeyCode::Char(_) | crossterm::event::KeyCode::Backspace => {
                    self.post_insert_completion_update();
                }
                _ => {}
            }
        }
    }

    fn dispatch_via_registry(&mut self, key_event: KeyEvent) {
        use crate::keybindings::{Action, KeyCombo, DispatchResult};
        use crate::keybindings::registry::ModeKey;

        let combo = KeyCombo::from_event(&key_event);

        // Handle awaiting char argument (for f/F)
        if let Some(action) = self.awaiting_char_action.take() {
            if let crate::keybindings::key::Key::Char(c) = combo.key {
                match action {
                    Action::FindCharForward => self.find_char_forward(c),
                    Action::FindCharBackward => self.find_char_backward(c),
                    _ => {}
                }
            }
            return;
        }

        let mode_key = match ModeKey::from_mode(&self.mode) {
            Some(mk) => mk,
            None => return,
        };

        let result = self.key_registry.resolve(&mode_key, combo.clone(), &mut self.pending_keys);

        match result {
            DispatchResult::Executed(action) => {
                self.execute_action(action);
            }
            DispatchResult::Pending => {
                // Waiting for more keys in a sequence
            }
            DispatchResult::AwaitingChar(action) => {
                self.awaiting_char_action = Some(action);
            }
            DispatchResult::Unbound => {
                // In Insert/Normie mode, unbound chars insert text
                if matches!(self.mode, Mode::Insert | Mode::Normie) {
                    match combo.key {
                        crate::keybindings::key::Key::Char(c) if !combo.ctrl && !combo.alt => {
                            self.insert_char(c);
                        }
                        crate::keybindings::key::Key::Tab => {
                            self.insert_char('\t');
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub fn execute_action(&mut self, action: crate::keybindings::Action) {
        use crate::keybindings::Action;
        match action {
            // Mode transitions
            Action::EnterNormalMode => self.enter_normal_mode(),
            Action::EnterInsertMode => self.enter_insert_mode(),
            Action::EnterVisualMode => self.enter_visual_mode(),
            Action::EnterVisualLineMode => self.enter_visual_line_mode(),
            Action::EnterCommandMode => self.enter_command_mode(),
            Action::EnterSearchMode => self.enter_search_mode(),
            Action::Append => self.append(),
            Action::OpenLineBelow => self.open_line_below(),
            Action::OpenLineAbove => self.open_line_above(),

            // Navigation
            Action::MoveCursorLeft => self.move_cursor_left(),
            Action::MoveCursorDown => self.move_cursor_down(),
            Action::MoveCursorUp => self.move_cursor_up(),
            Action::MoveCursorRight => self.move_cursor_right(),
            Action::MoveWordForward => self.move_word_forward(),
            Action::MoveWORDForward => self.move_WORD_forward(),
            Action::MoveWordBackward => self.move_word_backward(),
            Action::MoveWORDBackward => self.move_WORD_backward(),
            Action::MoveToLineStart => self.move_to_line_start(),
            Action::MoveToFirstNonWhitespace => self.move_to_first_non_whitespace(),
            Action::MoveToLineEnd => self.move_to_line_end(),
            Action::GotoFirstLine => self.goto_first_line(),
            Action::GotoLastLine => self.goto_last_line(),
            Action::GotoMatchingBracket => self.goto_matching_bracket(),
            Action::FindCharForward | Action::FindCharBackward => {
                // Handled via AwaitingChar path
            }
            Action::GoToDefinition => self.goto_definition(),
            Action::JumpBack => self.jump_back(),

            // Editing
            Action::DeleteChar => self.delete_char(),
            Action::DeleteLine => self.delete_line(),
            Action::DeleteWord => self.delete_word(),
            Action::DeleteToLineEnd => self.delete_to_line_end(),
            Action::DeleteToLineStart => self.delete_to_line_start(),
            Action::YankLine => self.yank_line(),
            Action::YankWord => self.yank_word(),
            Action::YankToLineEnd => self.yank_to_line_end(),
            Action::YankToLineStart => self.yank_to_line_start(),
            Action::PasteAfter => self.paste_after(),
            Action::PasteBefore => self.paste_before(),
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::InsertNewline => self.insert_newline(),
            Action::Backspace => self.backspace(),

            // Visual mode
            Action::DeleteVisualSelection => self.delete_visual_selection(),
            Action::ChangeVisualSelection => self.change_visual_selection(),
            Action::AppendAfterVisualSelection => self.append_after_visual_selection(),
            Action::YankVisualSelection => self.yank_visual_selection(),

            // Search
            Action::SearchNext => self.search_next(),
            Action::SearchPrev => self.search_prev(),

            // Scroll
            Action::PageDown => self.page_down(),
            Action::PageUp => self.page_up(),
            Action::HalfPageDown => self.half_page_down(),
            Action::HalfPageUp => self.half_page_up(),

            // Multi-cursor
            Action::SelectWordOrNextMatch => self.select_word_or_next_match(),

            // Splits
            Action::SplitHorizontal => self.split_horizontal(),
            Action::SplitVertical => self.split_vertical(),
            Action::FocusNextView => self.focus_next_view(),
            Action::FocusDirectionLeft => self.focus_direction_left(),
            Action::FocusDirectionDown => self.focus_direction_down(),
            Action::FocusDirectionUp => self.focus_direction_up(),
            Action::FocusDirectionRight => self.focus_direction_right(),
            Action::CloseCurrentView => self.close_current_view(),

            // Fuzzy finder / pickers
            Action::FuzzyFindFiles => {
                if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
                    if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                        fuzzy_plugin.activate(crate::plugins::js_fuzzy_finder::FuzzyFinderType::Files);
                        self.mode = Mode::FuzzyFinder;
                    }
                }
            }
            Action::FuzzyGrep => {
                if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
                    if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                        fuzzy_plugin.activate(crate::plugins::js_fuzzy_finder::FuzzyFinderType::Grep);
                        self.mode = Mode::FuzzyFinder;
                    }
                }
            }
            Action::ThemePicker => self.show_theme_picker(),
            Action::BufferPicker => self.show_buffer_picker(),

            // Completion
            Action::AcceptCompletion => self.accept_completion(),
            Action::CompletionNext => self.completion.navigate(1),
            Action::CompletionPrev => self.completion.navigate(-1),
            Action::DismissCompletion => self.completion.dismiss(),
            Action::AcceptGhostText => self.accept_ghost_text(),

            // File operations
            Action::SaveFile => self.save_file(),

            // Lifecycle
            Action::Exit => self.exit = true,

            Action::OpenFileTree => {
                if let Some(plugin) = self.plugin_manager.get_mut("file_tree") {
                    if let Some(tree_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::file_tree::FileTreePlugin>() {
                        if tree_plugin.is_active() {
                            tree_plugin.deactivate();
                        } else {
                            let theme = self.theme_manager.current();
                            tree_plugin.set_colors(
                                theme.ui.popup_border,
                                theme.ui.background,
                                theme.ui.foreground,
                                theme.ui.visual_selection_bg,
                                theme.ui.status_mode_normal,
                                theme.ui.line_number_fg,
                            );
                            tree_plugin.activate_tree();
                        }
                    }
                }
            }
            Action::OpenGit => {
                if let Some(plugin) = self.plugin_manager.get_mut("git_client") {
                    if let Some(git_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::git_client::GitClientPlugin>() {
                        let theme = self.theme_manager.current();
                        git_plugin.set_colors(
                            theme.ui.popup_border,
                            theme.ui.background,
                            theme.ui.foreground,
                            theme.ui.visual_selection_bg,
                            theme.ui.status_mode_normal,
                            theme.ui.line_number_fg,
                        );
                        git_plugin.activate_git();
                    }
                }
            }
            Action::ShowKeybindings => {
                self.show_keybindings = !self.show_keybindings;
            }
            Action::OpenSettings => {
                self.show_settings = !self.show_settings;
                self.settings_selected = 0;
                self.settings_scroll = 0;
            }
            Action::Noop => {}
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

        let yline = &buffer.lines[row];
        let char_count = yline.char_count();
        let end_char = col.min(char_count);
        let before_cursor: String = yline.text.chars().take(end_char).collect();

        // Find word start (in character indices)
        let mut word_start_char = 0;
        for (i, c) in before_cursor.chars().enumerate() {
            if !c.is_alphanumeric() && c != '_' {
                word_start_char = i + 1;
            }
        }

        let prefix: String = before_cursor.chars().skip(word_start_char).collect();
        (word_start_char, prefix)
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

    // ── Jump list ──────────────────────────────────────────────────────

    fn push_jump(&mut self) {
        let view = &self.views[self.active_view_idx];
        let loc = JumpLocation {
            filename: self.filename.clone(),
            buffer_id: view.buffer_id,
            row: view.cursor().row,
            col: view.cursor().col,
        };
        // Truncate forward history when pushing a new jump
        self.jump_list.truncate(self.jump_list_idx);
        self.jump_list.push(loc);
        self.jump_list_idx = self.jump_list.len();
    }

    fn jump_back(&mut self) {
        if self.jump_list_idx == 0 || self.jump_list.is_empty() {
            return;
        }
        // If at the end of the list, save current position first
        if self.jump_list_idx == self.jump_list.len() {
            let view = &self.views[self.active_view_idx];
            let loc = JumpLocation {
                filename: self.filename.clone(),
                buffer_id: view.buffer_id,
                row: view.cursor().row,
                col: view.cursor().col,
            };
            self.jump_list.push(loc);
        }
        self.jump_list_idx -= 1;
        let loc = self.jump_list[self.jump_list_idx].clone();
        self.jump_to_location(&loc);
    }

    fn jump_to_location(&mut self, loc: &JumpLocation) {
        // If it's a different file, open it
        if let Some(ref fname) = loc.filename {
            if self.filename.as_deref() != Some(fname) {
                self.open_file_in_view(fname);
            }
        }
        // Set cursor position
        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let row = loc.row.min(buffer.lines.len().saturating_sub(1));
        let col = loc.col.min(buffer.lines[row].char_count().saturating_sub(1));
        view.cursor_states[view.primary_cursor_idx].cursor.row = row;
        view.cursor_states[view.primary_cursor_idx].cursor.col = col;
        view.cursor_states[view.primary_cursor_idx].cursor.desired_col = col;
    }

    // ── Go to definition ─────────────────────────────────────────────

    fn goto_definition(&mut self) {
        let filename = match &self.filename {
            Some(f) => f.clone(),
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

        let view = &self.views[self.active_view_idx];
        let cursor_row = view.cursor().row;
        let cursor_col = view.cursor().col;

        // Push current position to jump list before jumping
        self.push_jump();

        if let Some(id) = self.lsp_manager.request_definition(
            &server_config.name,
            &uri,
            cursor_row,
            cursor_col,
        ) {
            self.pending_definition_request = Some(id);
        }
    }

    /// Process LSP responses and route completion results to CompletionState.
    fn process_lsp_responses(&mut self) {
        let responses: Vec<_> = self.lsp_manager.pending_responses.drain(..).collect();
        for resp in responses {
            // Check if this is a definition response
            if self.pending_definition_request == Some(resp.id) {
                self.pending_definition_request = None;
                if let Some(result) = resp.result {
                    self.handle_definition_response(&result);
                }
                continue;
            }
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

    fn handle_definition_response(&mut self, result: &serde_json::Value) {
        // LSP definition can return Location, Location[], or LocationLink[]
        let location = if result.is_array() {
            result.as_array().and_then(|arr| arr.first())
        } else if result.is_object() {
            Some(result)
        } else {
            None
        };

        let location = match location {
            Some(loc) => loc,
            None => return,
        };

        // Handle LocationLink (has targetUri/targetRange) or Location (has uri/range)
        let (uri, line, col) = if let Some(target_uri) = location.get("targetUri") {
            let range = location.get("targetSelectionRange")
                .or_else(|| location.get("targetRange"));
            let start = range.and_then(|r| r.get("start"));
            (
                target_uri.as_str().unwrap_or(""),
                start.and_then(|s| s.get("line")).and_then(|l| l.as_u64()).unwrap_or(0) as usize,
                start.and_then(|s| s.get("character")).and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            )
        } else if let Some(uri) = location.get("uri") {
            let start = location.get("range").and_then(|r| r.get("start"));
            (
                uri.as_str().unwrap_or(""),
                start.and_then(|s| s.get("line")).and_then(|l| l.as_u64()).unwrap_or(0) as usize,
                start.and_then(|s| s.get("character")).and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            )
        } else {
            return;
        };

        // Convert file:// URI to path
        let path = if let Some(p) = uri.strip_prefix("file://") {
            p.to_string()
        } else {
            return;
        };

        // Open the file and jump to the position
        self.open_file_in_view(&path);

        let view = &mut self.views[self.active_view_idx];
        let buffer = self.buffer_pool.get(view.buffer_id);
        let row = line.min(buffer.lines.len().saturating_sub(1));
        let col = col.min(buffer.lines[row].char_count().saturating_sub(1));
        view.cursor_states[view.primary_cursor_idx].cursor.row = row;
        view.cursor_states[view.primary_cursor_idx].cursor.col = col;
        view.cursor_states[view.primary_cursor_idx].cursor.desired_col = col;
        // Center the view on the definition
        let area_height = 40; // approximate
        view.scroll_offset = row.saturating_sub(area_height / 2);
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
            let yline = &buffer.lines[trigger_row];
            let char_count = yline.char_count();
            let trigger_char = trigger_col.min(char_count);
            let cursor_char = cursor_col.min(char_count);
            let before: String = yline.text.chars().take(trigger_char).collect();
            let after: String = yline.text.chars().skip(cursor_char).collect();
            buffer.lines[trigger_row].text = format!("{}{}{}", before, text, after);
        }

        let new_col = trigger_col + text.chars().count();
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
            let yline = &buffer.lines[trigger_row];
            let char_count = yline.char_count();
            let trigger_char = trigger_col.min(char_count);
            let cursor_char = cursor_col.min(char_count);
            let before: String = yline.text.chars().take(trigger_char).collect();
            let after: String = yline.text.chars().skip(cursor_char).collect();
            buffer.lines[trigger_row].text = format!("{}{}{}", before, text, after);
        }

        // Move cursor to end of inserted text
        let new_col = trigger_col + text.chars().count();
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

        let shortcuts: Vec<(&str, &str)> = if self.editor_mode == EditorMode::Normie {
            vec![
                ("  Ctrl+O   ", "  Find file"),
                ("  Ctrl+P   ", "  Grep in project"),
                ("  Ctrl+F   ", "  Search in file"),
                ("", ""),
                ("  Ctrl+S   ", "  Save"),
                ("  Ctrl+Q   ", "  Quit"),
                ("  Ctrl+W   ", "  Close split"),
                ("", ""),
                ("  Ctrl+Z   ", "  Undo"),
                ("  Ctrl+Y   ", "  Redo"),
                ("  Ctrl+D   ", "  Multi-cursor select"),
                ("", ""),
                ("  Ctrl+G   ", "  Git client"),
                ("  F1       ", "  Keybinding help"),
                ("", ""),
                ("  Ctrl+←/→ ", "  Word navigation"),
                ("  Ctrl+Home", "  Go to start"),
                ("  Ctrl+End ", "  Go to end"),
            ]
        } else {
            vec![
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
                ("  Space g  ", "  Git client"),
                ("  F1       ", "  Keybinding help"),
            ]
        };

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

    fn render_keybindings_help(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::text::{Line, Span, Text};
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::style::Style;

        let theme = self.theme_manager.current();
        let accent = theme.ui.popup_border;
        let dim = theme.ui.line_number_fg;
        let fg = theme.ui.foreground;

        let mut lines: Vec<Line> = Vec::new();

        let title = if self.editor_mode == EditorMode::Normie {
            "  Keybindings (Normal Mode)"
        } else {
            "  Keybindings (Vim Mode)"
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            title,
            Style::default().fg(accent).add_modifier(ratatui::style::Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        let bindings: Vec<(&str, &str)> = if self.editor_mode == EditorMode::Normie {
            vec![
                ("  Ctrl+S       ", "Save"),
                ("  Ctrl+Q       ", "Quit"),
                ("  Ctrl+Z       ", "Undo"),
                ("  Ctrl+Y       ", "Redo"),
                ("  Ctrl+F       ", "Search"),
                ("  Ctrl+D       ", "Multi-cursor select"),
                ("  Ctrl+O       ", "Find file"),
                ("  Ctrl+P       ", "Grep in project"),
                ("  Ctrl+W       ", "Close split"),
                ("  Ctrl+L       ", "Accept ghost text"),
                ("  F2           ", "Settings"),
                ("", ""),
                ("  Arrows       ", "Navigate"),
                ("  Ctrl+←/→     ", "Word navigation"),
                ("  Home / End   ", "Line start / end"),
                ("  Ctrl+Home/End", "File start / end"),
                ("  PgUp / PgDn  ", "Page up / down"),
                ("", ""),
                ("  Enter        ", "New line"),
                ("  Backspace    ", "Delete backward"),
                ("  Delete       ", "Delete forward"),
                ("  F1           ", "This help"),
            ]
        } else {
            vec![
                ("  i / a        ", "Insert / Append"),
                ("  o / O        ", "Open line below / above"),
                ("  Esc          ", "Normal mode"),
                ("  v / V        ", "Visual / Visual Line"),
                ("  :            ", "Command mode"),
                ("  /            ", "Search"),
                ("", ""),
                ("  h j k l      ", "Navigate"),
                ("  w / b        ", "Word forward / back"),
                ("  0 / $ / ^    ", "Line start / end / first char"),
                ("  gg / G       ", "File start / end"),
                ("  %            ", "Matching bracket"),
                ("  f / F        ", "Find char forward / back"),
                ("", ""),
                ("  dd / dw / d$ ", "Delete line / word / to end"),
                ("  yy / yw / y$ ", "Yank line / word / to end"),
                ("  p / P        ", "Paste after / before"),
                ("  x            ", "Delete char"),
                ("  u / Ctrl+R   ", "Undo / Redo"),
                ("", ""),
                ("  Ctrl+N       ", "Multi-cursor select"),
                ("  Ctrl+D/U     ", "Half page down / up"),
                ("  Ctrl+O       ", "Jump back"),
                ("  Ctrl+F/B     ", "Page down / up"),
                ("  gd           ", "Go to definition (LSP)"),
                ("", ""),
                ("  Space f f    ", "Find file"),
                ("  Space /      ", "Grep in project"),
                ("  Space b b    ", "Buffer picker"),
                ("  Space f t    ", "Theme picker"),
                ("  Space g      ", "Git client"),
                ("", ""),
                ("  Ctrl+W s/v   ", "Split horiz / vert"),
                ("  Ctrl+W hjkl  ", "Focus split"),
                ("  Ctrl+W q     ", "Close split"),
                ("  F1           ", "This help"),
                ("  F2           ", "Settings"),
            ]
        };

        for (key, desc) in &bindings {
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
            "  Press any key to close",
            Style::default().fg(dim),
        )));

        let text = Text::from(lines);
        let height = (text.lines.len() as u16 + 2).min(area.height);
        let width = 48u16;
        let popup_x = area.width.saturating_sub(width) / 2;
        let popup_y = area.height.saturating_sub(height) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: width.min(area.width),
            height,
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

    // ── Settings dialog ──────────────────────────────────────────────

    fn settings_items(&self) -> Vec<SettingsItem> {
        let mut items = Vec::new();

        // Editor mode
        let mode_str = match self.editor_mode {
            EditorMode::Vim => "Vim",
            EditorMode::Normie => "Normal",
        };
        items.push(SettingsItem {
            label: "Editor Mode".to_string(),
            value: mode_str.to_string(),
            kind: SettingsItemKind::EditorMode,
        });

        // Theme
        items.push(SettingsItem {
            label: "Theme".to_string(),
            value: self.theme_manager.current_name().to_string(),
            kind: SettingsItemKind::Theme,
        });

        // Separator
        items.push(SettingsItem {
            label: String::new(),
            value: String::new(),
            kind: SettingsItemKind::Separator,
        });

        // LSP servers
        for server in &self.config.lsp.servers {
            let status = if server.enabled {
                self.lsp_manager
                    .status
                    .get(&server.name)
                    .map(|s| s.as_str())
                    .unwrap_or("stopped")
            } else {
                "disabled"
            };
            items.push(SettingsItem {
                label: format!("{} ({})", server.name, server.language),
                value: status.to_string(),
                kind: SettingsItemKind::LspServer(server.name.clone()),
            });
        }

        items
    }

    fn settings_selectable_count(&self) -> usize {
        self.settings_items().iter().filter(|i| i.kind != SettingsItemKind::Separator).count()
    }

    fn settings_item_at_selection(&self, sel: usize) -> Option<SettingsItem> {
        let items = self.settings_items();
        let mut selectable_idx = 0;
        for item in items {
            if item.kind == SettingsItemKind::Separator {
                continue;
            }
            if selectable_idx == sel {
                return Some(item);
            }
            selectable_idx += 1;
        }
        None
    }

    fn handle_settings_key(&mut self, key_event: KeyEvent) {
        use crossterm::event::KeyCode;

        let count = self.settings_selectable_count();
        match key_event.code {
            KeyCode::Esc | KeyCode::F(2) => {
                self.show_settings = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.settings_selected > 0 {
                    self.settings_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.settings_selected + 1 < count {
                    self.settings_selected += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.activate_settings_item();
            }
            _ => {}
        }
    }

    fn activate_settings_item(&mut self) {
        let item = match self.settings_item_at_selection(self.settings_selected) {
            Some(i) => i,
            None => return,
        };
        match item.kind {
            SettingsItemKind::EditorMode => {
                let new_mode = match self.editor_mode {
                    EditorMode::Vim => EditorMode::Normie,
                    EditorMode::Normie => EditorMode::Vim,
                };
                self.editor_mode = new_mode.clone();
                self.mode = self.default_mode();
                self.config.editor_mode = Some(new_mode);
                self.config.save();
            }
            SettingsItemKind::Theme => {
                self.show_settings = false;
                self.show_theme_picker();
            }
            SettingsItemKind::LspServer(ref name) => {
                if let Some(server) = self.config.lsp.servers.iter_mut().find(|s| s.name == *name) {
                    server.enabled = !server.enabled;
                }
                self.config.save();
            }
            SettingsItemKind::Separator => {}
        }
    }

    fn render_settings(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::text::{Line, Span, Text};
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::style::{Modifier, Style};

        let theme = self.theme_manager.current();
        let accent = theme.ui.popup_border;
        let dim = theme.ui.line_number_fg;
        let fg = theme.ui.foreground;
        let sel_bg = theme.ui.visual_selection_bg;

        let items = self.settings_items();
        let mut lines: Vec<Line> = Vec::new();
        let mut selectable_idx = 0;

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Settings",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for item in &items {
            if item.kind == SettingsItemKind::Separator {
                lines.push(Line::from(""));
                continue;
            }

            let is_selected = selectable_idx == self.settings_selected;
            let key_style = if is_selected {
                Style::default().fg(accent).bg(sel_bg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(fg)
            };
            let val_style = if is_selected {
                Style::default().fg(dim).bg(sel_bg)
            } else {
                Style::default().fg(dim)
            };

            let label = format!("  {:<30}", item.label);
            let value = format!(" {}", item.value);

            lines.push(Line::from(vec![
                Span::styled(label, key_style),
                Span::styled(value, val_style),
            ]));
            selectable_idx += 1;
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ navigate  Enter toggle  Esc close",
            Style::default().fg(dim),
        )));

        let text = Text::from(lines);
        let height = (text.lines.len() as u16 + 2).min(area.height);
        let width = 56u16;
        let popup_x = area.width.saturating_sub(width) / 2;
        let popup_y = area.height.saturating_sub(height) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: width.min(area.width),
            height,
        };

        Clear.render(popup_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent))
            .title(" Settings (F2) ")
            .title_style(Style::default().fg(accent))
            .style(Style::default().bg(theme.ui.background).fg(fg));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);
        Paragraph::new(text).render(inner, buf);
    }

    fn render_mode_selector(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::text::{Line, Span, Text};
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::style::Style;

        let theme = self.theme_manager.current();
        let accent = theme.ui.popup_border;
        let dim = theme.ui.line_number_fg;
        let fg = theme.ui.foreground;

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  y editor",
                Style::default().fg(accent).add_modifier(ratatui::style::Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Choose your editing mode:",
                Style::default().fg(fg),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  v", Style::default().fg(accent).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::styled("  Vim mode", Style::default().fg(dim)),
            ]),
            Line::from(Span::styled(
                "     Modal editing (hjkl, i/Esc, :w)",
                Style::default().fg(dim),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  any other key", Style::default().fg(accent).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::styled("  Normal mode", Style::default().fg(dim)),
            ]),
            Line::from(Span::styled(
                "     Standard editing (Ctrl+S, arrows)",
                Style::default().fg(dim),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Choice is saved to ~/.config/y/config.toml",
                Style::default().fg(dim),
            )),
        ];

        let text = Text::from(lines);
        let height = text.lines.len() as u16 + 2;
        let width = 50u16;
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
