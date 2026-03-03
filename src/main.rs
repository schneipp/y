//This is the Y editor.
//Y another editor you ask?
//Because this is the Y editor.
use std::io;
use std::fs;
use std::env;
use std::process::Command;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    prelude::*,
    symbols::border,
    widgets::{block::*, *},
};
mod tui;

mod commands;

mod plugins;

#[derive(Debug, PartialEq, Clone)]
pub enum Mode {
    Normal,
    Insert,
    Visual,      // Character-wise visual mode
    VisualLine,  // Line-wise visual mode
    Command,     // Command mode (:w, :q, etc.)
    FuzzyFinder, // Fuzzy finder mode (telescope-like)
}

#[derive(Debug, PartialEq, Clone)]
pub enum FuzzyFinderType {
    Files,      // Find files
    Grep,       // Search text in files
}

#[derive(Debug, Clone)]
pub struct Cursor {
    row: usize,
    col: usize,
    desired_col: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            desired_col: 0,
        }
    }

    pub fn get_character_number(&self, buffer: &YBuffer) -> usize {
        let mut char_count = 0;

        // Count all characters in lines before current row
        for i in 0..self.row {
            if i < buffer.lines.len() {
                char_count += buffer.lines[i].text.len() + 1; // +1 for newline
            }
        }

        // Add columns in current line
        char_count += self.col;

        char_count
    }
}

pub struct YBuffers {
    buffers: Vec<YBuffer>,
}
#[derive(Debug, Clone)]
pub struct YBuffer {
    pub lines: Vec<YLine>,
}
impl YBuffer {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }
    pub fn from(lines: Vec<YLine>) -> Self {
        Self { lines }
    }
}
#[derive(Debug, Clone)]
pub struct YLine {
    pub text: String,
}
impl YLine {
    pub fn new() -> Self {
        Self {
            text: String::new(),
        }
    }
    pub fn from(text: String) -> Self {
        Self { text }
    }
    pub fn with_style(mut self) -> Self {
        self
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut terminal = tui::init()?;
    let mut app = if args.len() > 1 {
        App::from_file(&args[1])?
    } else {
        App::default()
    };

    let app_result = app.run(&mut terminal);
    tui::restore()?;
    app_result
}

#[derive(Debug, Clone)]
pub enum YankType {
    Character,
    Line,
}

#[derive(Debug, Clone)]
pub struct YankRegister {
    text: Vec<String>, // Lines of text
    yank_type: YankType,
}

#[derive(Debug)]
pub struct App {
    exit: bool,
    buffer: YBuffer,
    mode: Mode,
    cursor: Cursor,
    pending_key: Option<char>, // For multi-key commands like gg
    undo_stack: Vec<YBuffer>,
    redo_stack: Vec<YBuffer>,
    visual_start: Option<(usize, usize)>, // (row, col) where visual selection started
    yank_register: Option<YankRegister>, // Clipboard for yanked text
    filename: Option<String>, // Current file being edited
    command_buffer: String, // Buffer for command mode input
    modified: bool, // Has the buffer been modified since last save?
    // Fuzzy finder state
    fuzzy_finder_type: Option<FuzzyFinderType>,
    fuzzy_query: String,
    fuzzy_results: Vec<String>,
    fuzzy_selected: usize,
    space_pressed: bool, // Track if space was just pressed
    // Plugin system
    plugin_manager: plugins::PluginManager,
    // Scrolling
    scroll_offset: usize, // Line number at top of viewport
}

impl Default for App {
    fn default() -> Self {
        let buffer = YBuffer::from(vec![
            YLine::new(),
        ]);

        // Initialize plugin manager and register plugins
        let mut plugin_manager = plugins::PluginManager::new();
        plugin_manager.register(Box::new(plugins::fuzzy_finder::FuzzyFinderPlugin::new()));

        Self {
            exit: false,
            buffer,
            mode: Mode::Normal,
            cursor: Cursor::new(),
            pending_key: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            visual_start: None,
            yank_register: None,
            filename: None,
            command_buffer: String::new(),
            modified: false,
            fuzzy_finder_type: None,
            fuzzy_query: String::new(),
            fuzzy_results: Vec::new(),
            fuzzy_selected: 0,
            space_pressed: false,
            plugin_manager,
            scroll_offset: 0,
        }
    }
}

impl App {
    fn from_file(filename: &str) -> io::Result<Self> {
        let content = match fs::read_to_string(filename) {
            Ok(content) => content,
            Err(_) => {
                // File doesn't exist, create new buffer with filename
                let mut plugin_manager = plugins::PluginManager::new();
                plugin_manager.register(Box::new(plugins::fuzzy_finder::FuzzyFinderPlugin::new()));

                return Ok(Self {
                    exit: false,
                    buffer: YBuffer::from(vec![YLine::new()]),
                    mode: Mode::Normal,
                    cursor: Cursor::new(),
                    pending_key: None,
                    undo_stack: Vec::new(),
                    redo_stack: Vec::new(),
                    visual_start: None,
                    yank_register: None,
                    filename: Some(filename.to_string()),
                    command_buffer: String::new(),
                    modified: false,
                    fuzzy_finder_type: None,
                    fuzzy_query: String::new(),
                    fuzzy_results: Vec::new(),
                    fuzzy_selected: 0,
                    space_pressed: false,
                    plugin_manager,
                    scroll_offset: 0,
                });
            }
        };

        let lines: Vec<YLine> = if content.is_empty() {
            vec![YLine::new()]
        } else {
            content.lines()
                .map(|line| YLine::from(line.to_string()))
                .collect()
        };

        let mut plugin_manager = plugins::PluginManager::new();
        plugin_manager.register(Box::new(plugins::fuzzy_finder::FuzzyFinderPlugin::new()));

        Ok(Self {
            exit: false,
            buffer: YBuffer::from(lines),
            mode: Mode::Normal,
            cursor: Cursor::new(),
            pending_key: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            visual_start: None,
            yank_register: None,
            filename: Some(filename.to_string()),
            command_buffer: String::new(),
            modified: false,
            fuzzy_finder_type: None,
            fuzzy_query: String::new(),
            fuzzy_results: Vec::new(),
            fuzzy_selected: 0,
            space_pressed: false,
            plugin_manager,
            scroll_offset: 0,
        })
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut tui::Tui) -> io::Result<()> {
        while !self.exit {
            // Adjust scroll to keep cursor visible (estimate viewport height)
            let viewport_height = terminal.get_frame().size().height.saturating_sub(2) as usize;
            self.adjust_scroll(viewport_height);

            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }
    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.size());

        // Calculate cursor screen position
        // +1 for left border, +1 for top border
        // Adjust for scroll offset
        let cursor_x = self.cursor.col as u16 + 1;
        let cursor_y = (self.cursor.row.saturating_sub(self.scroll_offset)) as u16 + 1;

        // Set the terminal cursor position
        frame.set_cursor(cursor_x, cursor_y);
    }
    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
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
            let mut ctx = plugins::PluginContext {
                buffer: &mut self.buffer,
                cursor: &mut self.cursor,
                mode: &mut self.mode,
                filename: &self.filename,
                modified: &mut self.modified,
            };

            if self.plugin_manager.handle_key(key_event, &mut ctx) {
                // Plugin consumed the event
                return;
            }
        }

        // Handle normal mode switching
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key_event),
            Mode::Insert => self.handle_insert_mode(key_event),
            Mode::Visual => self.handle_visual_mode(key_event),
            Mode::VisualLine => self.handle_visual_line_mode(key_event),
            Mode::Command => self.handle_command_mode(key_event),
            Mode::FuzzyFinder => {
                // Fuzzy finder is now handled by plugins
                // This should not be reached if plugin system works correctly
            }
        }
    }

    fn handle_normal_mode(&mut self, key_event: KeyEvent) {
        // Handle space-based shortcuts
        if self.space_pressed {
            self.space_pressed = false;
            match key_event.code {
                KeyCode::Char('f') if matches!(self.pending_key, Some('f')) => {
                    self.pending_key = None;
                    // Activate fuzzy finder plugin for files
                    if let Some(plugin) = self.plugin_manager.get_mut("fuzzy_finder") {
                        if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<plugins::fuzzy_finder::FuzzyFinderPlugin>() {
                            fuzzy_plugin.activate(plugins::fuzzy_finder::FuzzyFinderType::Files);
                        }
                    }
                    return;
                }
                KeyCode::Char('/') => {
                    // Activate fuzzy finder plugin for grep
                    if let Some(plugin) = self.plugin_manager.get_mut("fuzzy_finder") {
                        if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<plugins::fuzzy_finder::FuzzyFinderPlugin>() {
                            fuzzy_plugin.activate(plugins::fuzzy_finder::FuzzyFinderType::Grep);
                        }
                    }
                    return;
                }
                _ => {
                    // Not a recognized space combo, continue normal processing
                }
            }
        }

        // Track space key for shortcuts
        if let KeyCode::Char(' ') = key_event.code {
            self.space_pressed = true;
            self.pending_key = Some('f'); // Prepare for potential 'ff'
            return;
        }

        // Handle multi-key sequences
        if let Some(pending) = self.pending_key {
            self.pending_key = None; // Clear pending key
            match (pending, key_event.code) {
                ('g', KeyCode::Char('g')) => self.goto_first_line(),
                ('d', KeyCode::Char('d')) => self.delete_line(),
                ('d', KeyCode::Char('w')) => self.delete_word(),
                ('d', KeyCode::Char('$')) => self.delete_to_line_end(),
                ('d', KeyCode::Char('0')) => self.delete_to_line_start(),
                ('y', KeyCode::Char('y')) => self.yank_line(),
                ('y', KeyCode::Char('w')) => self.yank_word(),
                ('y', KeyCode::Char('$')) => self.yank_to_line_end(),
                ('y', KeyCode::Char('0')) => self.yank_to_line_start(),
                ('f', KeyCode::Char(c)) => self.find_char_forward(c),
                ('F', KeyCode::Char(c)) => self.find_char_backward(c),
                _ => {} // Invalid sequence, ignore
            }
            return;
        }

        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            // Navigation
            KeyCode::Char('h') => self.move_cursor_left(),
            KeyCode::Char('j') => self.move_cursor_down(),
            KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Char('l') => self.move_cursor_right(),
            KeyCode::Char('w') => self.move_word_forward(),
            KeyCode::Char('W') => self.move_WORD_forward(),
            KeyCode::Char('b') => self.move_word_backward(),
            KeyCode::Char('B') => self.move_WORD_backward(),
            KeyCode::Char('0') => self.move_to_line_start(),
            KeyCode::Char('$') => self.move_to_line_end(),
            KeyCode::Char('G') => self.goto_last_line(),
            KeyCode::Char('g') => self.pending_key = Some('g'), // Wait for second 'g'
            // Editing
            KeyCode::Char('x') => self.delete_char(),
            KeyCode::Char('d') => self.pending_key = Some('d'), // Wait for motion
            KeyCode::Char('y') => self.pending_key = Some('y'), // Wait for motion
            KeyCode::Char('p') => self.paste_after(),
            KeyCode::Char('P') => self.paste_before(),
            KeyCode::Char('f') => self.pending_key = Some('f'), // Wait for character
            KeyCode::Char('F') => self.pending_key = Some('F'), // Wait for character (backward)
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('r') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => self.redo(),
            // Page scrolling
            KeyCode::Char('f') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => self.page_down(),
            KeyCode::Char('b') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => self.page_up(),
            KeyCode::Char('d') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => self.half_page_down(),
            KeyCode::Char('u') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => self.half_page_up(),
            // Visual modes
            KeyCode::Char('v') => self.enter_visual_mode(),
            KeyCode::Char('V') => self.enter_visual_line_mode(),
            // Command mode
            KeyCode::Char(':') => self.enter_command_mode(),
            // Insert modes
            KeyCode::Char('i') => self.enter_insert_mode(),
            KeyCode::Char('a') => self.append(),
            KeyCode::Char('o') => self.open_line_below(),
            KeyCode::Char('O') => self.open_line_above(),
            // Arrow keys
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Up => self.move_cursor_up(),
            KeyCode::Right => self.move_cursor_right(),
            _ => {}
        }
    }

    fn handle_insert_mode(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.enter_normal_mode(),
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Enter => self.insert_newline(),
            KeyCode::Backspace => self.backspace(),
            _ => {}
        }
    }

    fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
    }

    fn enter_normal_mode(&mut self) {
        self.mode = Mode::Normal;
        self.visual_start = None;
    }

    fn enter_visual_mode(&mut self) {
        self.mode = Mode::Visual;
        self.visual_start = Some((self.cursor.row, self.cursor.col));
    }

    fn enter_visual_line_mode(&mut self) {
        self.mode = Mode::VisualLine;
        self.visual_start = Some((self.cursor.row, self.cursor.col));
    }

    fn enter_command_mode(&mut self) {
        self.mode = Mode::Command;
        self.command_buffer.clear();
    }

    fn handle_command_mode(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
            }
            KeyCode::Enter => self.execute_command(),
            KeyCode::Backspace => {
                self.command_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.command_buffer.trim();

        match cmd {
            "w" => self.save_file(),
            "q" => self.quit_command(),
            "wq" | "x" => {
                self.save_file();
                if !self.modified {
                    self.exit = true;
                }
            }
            "q!" => self.exit = true,
            _ => {
                // Unknown command, just exit command mode
            }
        }

        self.mode = Mode::Normal;
        self.command_buffer.clear();
    }

    fn save_file(&mut self) {
        if let Some(ref filename) = self.filename.clone() {
            let content: String = self.buffer.lines
                .iter()
                .map(|line| line.text.clone())
                .collect::<Vec<String>>()
                .join("\n");

            if fs::write(filename, content).is_ok() {
                self.modified = false;
            }
        }
    }

    fn quit_command(&mut self) {
        if !self.modified {
            self.exit = true;
        }
        // If modified, don't quit (vim behavior)
        // User must use :q! or :wq
    }

    fn handle_visual_mode(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.enter_normal_mode(),
            // Navigation extends selection
            KeyCode::Char('h') => self.move_cursor_left(),
            KeyCode::Char('j') => self.move_cursor_down(),
            KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Char('l') => self.move_cursor_right(),
            KeyCode::Char('w') => self.move_word_forward(),
            KeyCode::Char('W') => self.move_WORD_forward(),
            KeyCode::Char('b') => self.move_word_backward(),
            KeyCode::Char('B') => self.move_WORD_backward(),
            KeyCode::Char('0') => self.move_to_line_start(),
            KeyCode::Char('$') => self.move_to_line_end(),
            KeyCode::Char('G') => self.goto_last_line(),
            // Operations on selection
            KeyCode::Char('d') | KeyCode::Char('x') => self.delete_visual_selection(),
            KeyCode::Char('y') => self.yank_visual_selection(),
            // Switch to line-wise visual
            KeyCode::Char('V') => self.enter_visual_line_mode(),
            // Arrow keys
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Up => self.move_cursor_up(),
            KeyCode::Right => self.move_cursor_right(),
            _ => {}
        }
    }

    fn handle_visual_line_mode(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.enter_normal_mode(),
            // Navigation extends selection
            KeyCode::Char('j') => self.move_cursor_down(),
            KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Char('G') => self.goto_last_line(),
            // Operations on selection
            KeyCode::Char('d') | KeyCode::Char('x') => self.delete_visual_selection(),
            KeyCode::Char('y') => self.yank_visual_selection(),
            // Switch to character-wise visual
            KeyCode::Char('v') => self.enter_visual_mode(),
            // Arrow keys
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Up => self.move_cursor_up(),
            _ => {}
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
            self.cursor.desired_col = self.cursor.col;
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor.row < self.buffer.lines.len() {
            let line_len = self.buffer.lines[self.cursor.row].text.len();
            if self.cursor.col < line_len {
                self.cursor.col += 1;
                self.cursor.desired_col = self.cursor.col;
            }
        }
    }

    fn move_cursor_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.clamp_cursor_to_line();
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor.row < self.buffer.lines.len() - 1 {
            self.cursor.row += 1;
            self.clamp_cursor_to_line();
        }
    }

    fn clamp_cursor_to_line(&mut self) {
        if self.cursor.row < self.buffer.lines.len() {
            let line_len = self.buffer.lines[self.cursor.row].text.len();
            // Try to maintain desired column, but clamp to line length
            self.cursor.col = self.cursor.desired_col.min(line_len);
        }
    }

    fn move_word_forward(&mut self) {
        if self.cursor.row >= self.buffer.lines.len() {
            return;
        }

        let line = &self.buffer.lines[self.cursor.row].text;
        let chars: Vec<char> = line.chars().collect();

        // Skip current word
        while self.cursor.col < chars.len() && !chars[self.cursor.col].is_whitespace() {
            self.cursor.col += 1;
        }

        // Skip whitespace
        while self.cursor.col < chars.len() && chars[self.cursor.col].is_whitespace() {
            self.cursor.col += 1;
        }

        // If at end of line, move to start of next line
        if self.cursor.col >= chars.len() && self.cursor.row < self.buffer.lines.len() - 1 {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }

        self.cursor.desired_col = self.cursor.col;
    }

    fn move_word_backward(&mut self) {
        if self.cursor.row >= self.buffer.lines.len() {
            return;
        }

        // If at start of line, move to end of previous line
        if self.cursor.col == 0 {
            if self.cursor.row > 0 {
                self.cursor.row -= 1;
                self.cursor.col = self.buffer.lines[self.cursor.row].text.len();
                if self.cursor.col > 0 {
                    self.cursor.col -= 1;
                }
            }
            self.cursor.desired_col = self.cursor.col;
            return;
        }

        let line = &self.buffer.lines[self.cursor.row].text;
        let chars: Vec<char> = line.chars().collect();

        // Move back one position
        self.cursor.col -= 1;

        // Skip whitespace
        while self.cursor.col > 0 && chars[self.cursor.col].is_whitespace() {
            self.cursor.col -= 1;
        }

        // Move to start of word
        while self.cursor.col > 0 && !chars[self.cursor.col - 1].is_whitespace() {
            self.cursor.col -= 1;
        }

        self.cursor.desired_col = self.cursor.col;
    }

    fn move_to_line_start(&mut self) {
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
    }

    fn move_to_line_end(&mut self) {
        if self.cursor.row < self.buffer.lines.len() {
            let line_len = self.buffer.lines[self.cursor.row].text.len();
            self.cursor.col = if line_len > 0 { line_len - 1 } else { 0 };
            self.cursor.desired_col = self.cursor.col;
        }
    }

    fn goto_first_line(&mut self) {
        self.cursor.row = 0;
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
    }

    fn goto_last_line(&mut self) {
        if !self.buffer.lines.is_empty() {
            self.cursor.row = self.buffer.lines.len() - 1;
            self.cursor.col = 0;
            self.cursor.desired_col = 0;
        }
    }

    // Scrolling methods
    fn page_down(&mut self) {
        // Calculate viewport height (we'll estimate ~30 lines for now, will be refined in rendering)
        let viewport_height = 30;
        let new_row = (self.cursor.row + viewport_height).min(self.buffer.lines.len().saturating_sub(1));
        self.cursor.row = new_row;
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
        self.scroll_offset = (self.scroll_offset + viewport_height).min(self.buffer.lines.len().saturating_sub(viewport_height));
    }

    fn page_up(&mut self) {
        let viewport_height = 30;
        let new_row = self.cursor.row.saturating_sub(viewport_height);
        self.cursor.row = new_row;
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
        self.scroll_offset = self.scroll_offset.saturating_sub(viewport_height);
    }

    fn half_page_down(&mut self) {
        let viewport_height = 15; // Half of estimated viewport
        let new_row = (self.cursor.row + viewport_height).min(self.buffer.lines.len().saturating_sub(1));
        self.cursor.row = new_row;
        self.clamp_cursor_to_line();
    }

    fn half_page_up(&mut self) {
        let viewport_height = 15; // Half of estimated viewport
        let new_row = self.cursor.row.saturating_sub(viewport_height);
        self.cursor.row = new_row;
        self.clamp_cursor_to_line();
    }

    fn adjust_scroll(&mut self, viewport_height: usize) {
        // Ensure cursor is visible in viewport
        if self.cursor.row < self.scroll_offset {
            // Cursor is above viewport, scroll up
            self.scroll_offset = self.cursor.row;
        } else if self.cursor.row >= self.scroll_offset + viewport_height {
            // Cursor is below viewport, scroll down
            self.scroll_offset = self.cursor.row.saturating_sub(viewport_height - 1);
        }
    }

    fn append(&mut self) {
        // Move cursor right then enter insert mode
        if self.cursor.row < self.buffer.lines.len() {
            let line_len = self.buffer.lines[self.cursor.row].text.len();
            if self.cursor.col < line_len {
                self.cursor.col += 1;
                self.cursor.desired_col = self.cursor.col;
            }
        }
        self.mode = Mode::Insert;
    }

    fn open_line_below(&mut self) {
        // Insert new line below current line and enter insert mode
        self.save_state();
        if self.cursor.row < self.buffer.lines.len() {
            self.buffer.lines.insert(self.cursor.row + 1, YLine::new());
            self.cursor.row += 1;
            self.cursor.col = 0;
            self.cursor.desired_col = 0;
            self.mode = Mode::Insert;
        }
    }

    fn open_line_above(&mut self) {
        // Insert new line above current line and enter insert mode
        self.save_state();
        self.buffer.lines.insert(self.cursor.row, YLine::new());
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
        self.mode = Mode::Insert;
    }

    fn delete_visual_selection(&mut self) {
        if let Some((start_row, start_col)) = self.visual_start {
            self.save_state();

            let end_row = self.cursor.row;
            let end_col = self.cursor.col;

            if self.mode == Mode::VisualLine {
                // Delete entire lines
                let (first_line, last_line) = if start_row <= end_row {
                    (start_row, end_row)
                } else {
                    (end_row, start_row)
                };

                // Delete lines from last to first to maintain indices
                for _ in first_line..=last_line {
                    if first_line < self.buffer.lines.len() {
                        self.buffer.lines.remove(first_line);
                    }
                }

                // Ensure at least one empty line
                if self.buffer.lines.is_empty() {
                    self.buffer.lines.push(YLine::new());
                }

                // Position cursor
                self.cursor.row = first_line.min(self.buffer.lines.len() - 1);
                self.cursor.col = 0;
                self.cursor.desired_col = 0;

            } else {
                // Character-wise visual mode
                let (start_pos, end_pos) = if (start_row, start_col) <= (end_row, end_col) {
                    ((start_row, start_col), (end_row, end_col))
                } else {
                    ((end_row, end_col), (start_row, start_col))
                };

                if start_pos.0 == end_pos.0 {
                    // Single line selection
                    if start_pos.0 < self.buffer.lines.len() {
                        let line = &mut self.buffer.lines[start_pos.0];
                        let chars: Vec<char> = line.text.chars().collect();
                        let mut new_text = String::new();
                        for (i, ch) in chars.iter().enumerate() {
                            if i < start_pos.1 || i > end_pos.1 {
                                new_text.push(*ch);
                            }
                        }
                        line.text = new_text;
                        self.cursor.row = start_pos.0;
                        self.cursor.col = start_pos.1;
                    }
                } else {
                    // Multi-line selection
                    // Get the parts to keep
                    let first_line_text = if start_pos.0 < self.buffer.lines.len() {
                        self.buffer.lines[start_pos.0].text.chars().take(start_pos.1).collect::<String>()
                    } else {
                        String::new()
                    };

                    let last_line_text = if end_pos.0 < self.buffer.lines.len() {
                        self.buffer.lines[end_pos.0].text.chars().skip(end_pos.1 + 1).collect::<String>()
                    } else {
                        String::new()
                    };

                    // Delete the lines in between and at the ends
                    for _ in start_pos.0..=end_pos.0.min(self.buffer.lines.len() - 1) {
                        if start_pos.0 < self.buffer.lines.len() {
                            self.buffer.lines.remove(start_pos.0);
                        }
                    }

                    // Insert the combined line
                    let combined = format!("{}{}", first_line_text, last_line_text);
                    self.buffer.lines.insert(start_pos.0, YLine::from(combined));

                    self.cursor.row = start_pos.0;
                    self.cursor.col = start_pos.1;
                }

                self.cursor.desired_col = self.cursor.col;
            }

            self.enter_normal_mode();
        }
    }

    fn delete_char(&mut self) {
        // Delete character under cursor (x command)
        self.save_state();
        if self.cursor.row < self.buffer.lines.len() {
            let line = &mut self.buffer.lines[self.cursor.row];
            if self.cursor.col < line.text.len() {
                line.text.remove(self.cursor.col);
                // Adjust cursor if at end of line
                if self.cursor.col >= line.text.len() && line.text.len() > 0 {
                    self.cursor.col = line.text.len() - 1;
                } else if line.text.is_empty() {
                    self.cursor.col = 0;
                }
                self.cursor.desired_col = self.cursor.col;
            }
        }
    }

    fn delete_line(&mut self) {
        // Delete entire line (dd command)
        self.save_state();
        if self.cursor.row < self.buffer.lines.len() {
            self.buffer.lines.remove(self.cursor.row);

            // If we deleted the last line, ensure at least one empty line exists
            if self.buffer.lines.is_empty() {
                self.buffer.lines.push(YLine::new());
            }

            // Adjust cursor position
            if self.cursor.row >= self.buffer.lines.len() {
                self.cursor.row = self.buffer.lines.len() - 1;
            }

            self.cursor.col = 0;
            self.cursor.desired_col = 0;
        }
    }

    fn delete_word(&mut self) {
        // Delete from cursor to start of next word (dw command)
        self.save_state();
        if self.cursor.row >= self.buffer.lines.len() {
            return;
        }

        let line = &self.buffer.lines[self.cursor.row].text;
        let chars: Vec<char> = line.chars().collect();
        let start = self.cursor.col;
        let mut end = start;

        // Skip current word
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }

        // Skip whitespace
        while end < chars.len() && chars[end].is_whitespace() {
            end += 1;
        }

        // Delete the range
        if start < chars.len() {
            let mut new_text = String::new();
            for (i, ch) in chars.iter().enumerate() {
                if i < start || i >= end {
                    new_text.push(*ch);
                }
            }
            self.buffer.lines[self.cursor.row].text = new_text;

            // Adjust cursor
            if self.cursor.col >= self.buffer.lines[self.cursor.row].text.len()
                && self.buffer.lines[self.cursor.row].text.len() > 0 {
                self.cursor.col = self.buffer.lines[self.cursor.row].text.len() - 1;
            }
            self.cursor.desired_col = self.cursor.col;
        }
    }

    fn delete_to_line_end(&mut self) {
        // Delete from cursor to end of line (d$ command)
        self.save_state();
        if self.cursor.row < self.buffer.lines.len() {
            let line = &mut self.buffer.lines[self.cursor.row];
            line.text.truncate(self.cursor.col);

            // Adjust cursor if line is now empty or cursor is past end
            if self.cursor.col > 0 && self.cursor.col >= line.text.len() {
                self.cursor.col = line.text.len().saturating_sub(1);
            } else if line.text.is_empty() {
                self.cursor.col = 0;
            }
            self.cursor.desired_col = self.cursor.col;
        }
    }

    fn delete_to_line_start(&mut self) {
        // Delete from start of line to cursor (d0 command)
        self.save_state();
        if self.cursor.row < self.buffer.lines.len() {
            let line = &self.buffer.lines[self.cursor.row];
            let remaining: String = line.text.chars().skip(self.cursor.col).collect();
            self.buffer.lines[self.cursor.row].text = remaining;
            self.cursor.col = 0;
            self.cursor.desired_col = 0;
        }
    }

    fn find_char_forward(&mut self, target: char) {
        // Find next occurrence of character on current line (f command)
        if self.cursor.row < self.buffer.lines.len() {
            let line = &self.buffer.lines[self.cursor.row].text;
            let chars: Vec<char> = line.chars().collect();

            for i in (self.cursor.col + 1)..chars.len() {
                if chars[i] == target {
                    self.cursor.col = i;
                    self.cursor.desired_col = i;
                    return;
                }
            }
        }
    }

    fn find_char_backward(&mut self, target: char) {
        // Find previous occurrence of character on current line (F command)
        if self.cursor.row < self.buffer.lines.len() {
            let line = &self.buffer.lines[self.cursor.row].text;
            let chars: Vec<char> = line.chars().collect();

            for i in (0..self.cursor.col).rev() {
                if chars[i] == target {
                    self.cursor.col = i;
                    self.cursor.desired_col = i;
                    return;
                }
            }
        }
    }

    #[allow(non_snake_case)]
    fn move_WORD_forward(&mut self) {
        // Move to start of next WORD (W command) - whitespace-separated
        if self.cursor.row >= self.buffer.lines.len() {
            return;
        }

        let line = &self.buffer.lines[self.cursor.row].text;
        let chars: Vec<char> = line.chars().collect();

        // Skip non-whitespace (current WORD)
        while self.cursor.col < chars.len() && !chars[self.cursor.col].is_whitespace() {
            self.cursor.col += 1;
        }

        // Skip whitespace
        while self.cursor.col < chars.len() && chars[self.cursor.col].is_whitespace() {
            self.cursor.col += 1;
        }

        // If at end of line, move to start of next line
        if self.cursor.col >= chars.len() && self.cursor.row < self.buffer.lines.len() - 1 {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }

        self.cursor.desired_col = self.cursor.col;
    }

    #[allow(non_snake_case)]
    fn move_WORD_backward(&mut self) {
        // Move to start of previous WORD (B command) - whitespace-separated
        if self.cursor.row >= self.buffer.lines.len() {
            return;
        }

        // If at start of line, move to end of previous line
        if self.cursor.col == 0 {
            if self.cursor.row > 0 {
                self.cursor.row -= 1;
                self.cursor.col = self.buffer.lines[self.cursor.row].text.len();
                if self.cursor.col > 0 {
                    self.cursor.col -= 1;
                }
            }
            self.cursor.desired_col = self.cursor.col;
            return;
        }

        let line = &self.buffer.lines[self.cursor.row].text;
        let chars: Vec<char> = line.chars().collect();

        // Move back one position
        self.cursor.col -= 1;

        // Skip whitespace
        while self.cursor.col > 0 && chars[self.cursor.col].is_whitespace() {
            self.cursor.col -= 1;
        }

        // Move to start of WORD
        while self.cursor.col > 0 && !chars[self.cursor.col - 1].is_whitespace() {
            self.cursor.col -= 1;
        }

        self.cursor.desired_col = self.cursor.col;
    }

    fn insert_char(&mut self, c: char) {
        self.save_state();
        if self.cursor.row < self.buffer.lines.len() {
            let line = &mut self.buffer.lines[self.cursor.row];
            // Insert character at cursor position
            line.text.insert(self.cursor.col, c);
            // Move cursor right
            self.cursor.col += 1;
            self.cursor.desired_col = self.cursor.col;
        }
    }

    fn insert_newline(&mut self) {
        self.save_state();
        if self.cursor.row < self.buffer.lines.len() {
            // Split the current line at cursor position
            let current_line = &self.buffer.lines[self.cursor.row].text;
            let before = current_line[..self.cursor.col].to_string();
            let after = current_line[self.cursor.col..].to_string();

            // Update current line with text before cursor
            self.buffer.lines[self.cursor.row].text = before;

            // Insert new line with text after cursor
            self.buffer.lines.insert(self.cursor.row + 1, YLine::from(after));

            // Move cursor to beginning of new line
            self.cursor.row += 1;
            self.cursor.col = 0;
            self.cursor.desired_col = 0;
        }
    }

    fn backspace(&mut self) {
        self.save_state();
        if self.cursor.col > 0 {
            // Delete character before cursor in current line
            if self.cursor.row < self.buffer.lines.len() {
                self.buffer.lines[self.cursor.row].text.remove(self.cursor.col - 1);
                self.cursor.col -= 1;
                self.cursor.desired_col = self.cursor.col;
            }
        } else if self.cursor.row > 0 {
            // At beginning of line - join with previous line
            let current_line = self.buffer.lines.remove(self.cursor.row);
            self.cursor.row -= 1;
            self.cursor.col = self.buffer.lines[self.cursor.row].text.len();
            self.buffer.lines[self.cursor.row].text.push_str(&current_line.text);
            self.cursor.desired_col = self.cursor.col;
        }
    }

    fn save_state(&mut self) {
        // Save current buffer state to undo stack
        self.undo_stack.push(self.buffer.clone());
        // Clear redo stack when new change is made
        self.redo_stack.clear();
        // Mark as modified
        self.modified = true;
    }

    fn undo(&mut self) {
        if let Some(previous_buffer) = self.undo_stack.pop() {
            // Save current state to redo stack
            self.redo_stack.push(self.buffer.clone());
            // Restore previous buffer state
            self.buffer = previous_buffer;
            // Clamp cursor to valid position
            if self.cursor.row >= self.buffer.lines.len() {
                self.cursor.row = self.buffer.lines.len().saturating_sub(1);
            }
            if self.cursor.row < self.buffer.lines.len() {
                let line_len = self.buffer.lines[self.cursor.row].text.len();
                if self.cursor.col > line_len {
                    self.cursor.col = line_len;
                }
            }
            self.cursor.desired_col = self.cursor.col;
        }
    }

    fn redo(&mut self) {
        if let Some(next_buffer) = self.redo_stack.pop() {
            // Save current state to undo stack
            self.undo_stack.push(self.buffer.clone());
            // Restore next buffer state
            self.buffer = next_buffer;
            // Clamp cursor to valid position
            if self.cursor.row >= self.buffer.lines.len() {
                self.cursor.row = self.buffer.lines.len().saturating_sub(1);
            }
            if self.cursor.row < self.buffer.lines.len() {
                let line_len = self.buffer.lines[self.cursor.row].text.len();
                if self.cursor.col > line_len {
                    self.cursor.col = line_len;
                }
            }
            self.cursor.desired_col = self.cursor.col;
        }
    }

    fn yank_line(&mut self) {
        // Yank entire line (yy command)
        if self.cursor.row < self.buffer.lines.len() {
            let line_text = self.buffer.lines[self.cursor.row].text.clone();
            self.yank_register = Some(YankRegister {
                text: vec![line_text],
                yank_type: YankType::Line,
            });
        }
    }

    fn yank_word(&mut self) {
        // Yank from cursor to start of next word (yw command)
        if self.cursor.row >= self.buffer.lines.len() {
            return;
        }

        let line = &self.buffer.lines[self.cursor.row].text;
        let chars: Vec<char> = line.chars().collect();
        let start = self.cursor.col;
        let mut end = start;

        // Skip current word
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }

        // Skip whitespace
        while end < chars.len() && chars[end].is_whitespace() {
            end += 1;
        }

        if start < chars.len() {
            let yanked_text: String = chars[start..end].iter().collect();
            self.yank_register = Some(YankRegister {
                text: vec![yanked_text],
                yank_type: YankType::Character,
            });
        }
    }

    fn yank_to_line_end(&mut self) {
        // Yank from cursor to end of line (y$ command)
        if self.cursor.row < self.buffer.lines.len() {
            let line = &self.buffer.lines[self.cursor.row];
            let yanked_text: String = line.text.chars().skip(self.cursor.col).collect();
            self.yank_register = Some(YankRegister {
                text: vec![yanked_text],
                yank_type: YankType::Character,
            });
        }
    }

    fn yank_to_line_start(&mut self) {
        // Yank from start of line to cursor (y0 command)
        if self.cursor.row < self.buffer.lines.len() {
            let line = &self.buffer.lines[self.cursor.row];
            let yanked_text: String = line.text.chars().take(self.cursor.col).collect();
            self.yank_register = Some(YankRegister {
                text: vec![yanked_text],
                yank_type: YankType::Character,
            });
        }
    }

    fn yank_visual_selection(&mut self) {
        if let Some((start_row, start_col)) = self.visual_start {
            let end_row = self.cursor.row;
            let end_col = self.cursor.col;

            if self.mode == Mode::VisualLine {
                // Yank entire lines
                let (first_line, last_line) = if start_row <= end_row {
                    (start_row, end_row)
                } else {
                    (end_row, start_row)
                };

                let mut yanked_lines = Vec::new();
                for row in first_line..=last_line {
                    if row < self.buffer.lines.len() {
                        yanked_lines.push(self.buffer.lines[row].text.clone());
                    }
                }

                self.yank_register = Some(YankRegister {
                    text: yanked_lines,
                    yank_type: YankType::Line,
                });
            } else {
                // Character-wise visual mode
                let (start_pos, end_pos) = if (start_row, start_col) <= (end_row, end_col) {
                    ((start_row, start_col), (end_row, end_col))
                } else {
                    ((end_row, end_col), (start_row, start_col))
                };

                if start_pos.0 == end_pos.0 {
                    // Single line selection
                    if start_pos.0 < self.buffer.lines.len() {
                        let line = &self.buffer.lines[start_pos.0];
                        let chars: Vec<char> = line.text.chars().collect();
                        let yanked_text: String = chars[start_pos.1..=end_pos.1.min(chars.len().saturating_sub(1))]
                            .iter()
                            .collect();
                        self.yank_register = Some(YankRegister {
                            text: vec![yanked_text],
                            yank_type: YankType::Character,
                        });
                    }
                } else {
                    // Multi-line selection
                    let mut yanked_lines = Vec::new();

                    // First line (from start_col to end)
                    if start_pos.0 < self.buffer.lines.len() {
                        let first_line_text: String = self.buffer.lines[start_pos.0]
                            .text
                            .chars()
                            .skip(start_pos.1)
                            .collect();
                        yanked_lines.push(first_line_text);
                    }

                    // Middle lines (entire lines)
                    for row in (start_pos.0 + 1)..end_pos.0 {
                        if row < self.buffer.lines.len() {
                            yanked_lines.push(self.buffer.lines[row].text.clone());
                        }
                    }

                    // Last line (from start to end_col)
                    if end_pos.0 < self.buffer.lines.len() && end_pos.0 != start_pos.0 {
                        let last_line_text: String = self.buffer.lines[end_pos.0]
                            .text
                            .chars()
                            .take(end_pos.1 + 1)
                            .collect();
                        yanked_lines.push(last_line_text);
                    }

                    self.yank_register = Some(YankRegister {
                        text: yanked_lines,
                        yank_type: YankType::Character,
                    });
                }
            }

            self.enter_normal_mode();
        }
    }

    fn paste_after(&mut self) {
        if let Some(ref register) = self.yank_register.clone() {
            self.save_state();

            match register.yank_type {
                YankType::Line => {
                    // Paste lines after current line
                    let insert_row = self.cursor.row + 1;
                    for (i, line_text) in register.text.iter().enumerate() {
                        self.buffer.lines.insert(insert_row + i, YLine::from(line_text.clone()));
                    }
                    // Move cursor to first pasted line
                    if insert_row < self.buffer.lines.len() {
                        self.cursor.row = insert_row;
                        self.cursor.col = 0;
                        self.cursor.desired_col = 0;
                    }
                }
                YankType::Character => {
                    if self.cursor.row < self.buffer.lines.len() {
                        if register.text.len() == 1 {
                            // Single line paste - insert after cursor
                            let line = &mut self.buffer.lines[self.cursor.row];
                            let insert_pos = (self.cursor.col + 1).min(line.text.len());
                            line.text.insert_str(insert_pos, &register.text[0]);
                            self.cursor.col = insert_pos;
                            self.cursor.desired_col = self.cursor.col;
                        } else {
                            // Multi-line character paste
                            let current_line = &self.buffer.lines[self.cursor.row].text;
                            let before = current_line.chars().take(self.cursor.col + 1).collect::<String>();
                            let after = current_line.chars().skip(self.cursor.col + 1).collect::<String>();

                            // Replace current line with first part + first yanked line
                            self.buffer.lines[self.cursor.row].text = format!("{}{}", before, register.text[0]);

                            // Insert middle yanked lines
                            for i in 1..(register.text.len() - 1) {
                                self.buffer.lines.insert(self.cursor.row + i, YLine::from(register.text[i].clone()));
                            }

                            // Insert last yanked line + remainder
                            if register.text.len() > 1 {
                                let last_yanked = &register.text[register.text.len() - 1];
                                self.buffer.lines.insert(
                                    self.cursor.row + register.text.len() - 1,
                                    YLine::from(format!("{}{}", last_yanked, after))
                                );
                            }

                            self.cursor.row += register.text.len() - 1;
                            self.cursor.col = register.text[register.text.len() - 1].len();
                            self.cursor.desired_col = self.cursor.col;
                        }
                    }
                }
            }
        }
    }

    fn paste_before(&mut self) {
        if let Some(ref register) = self.yank_register.clone() {
            self.save_state();

            match register.yank_type {
                YankType::Line => {
                    // Paste lines before current line
                    let insert_row = self.cursor.row;
                    for (i, line_text) in register.text.iter().enumerate() {
                        self.buffer.lines.insert(insert_row + i, YLine::from(line_text.clone()));
                    }
                    // Move cursor to first pasted line
                    self.cursor.row = insert_row;
                    self.cursor.col = 0;
                    self.cursor.desired_col = 0;
                }
                YankType::Character => {
                    if self.cursor.row < self.buffer.lines.len() {
                        if register.text.len() == 1 {
                            // Single line paste - insert at cursor
                            let line = &mut self.buffer.lines[self.cursor.row];
                            line.text.insert_str(self.cursor.col, &register.text[0]);
                            self.cursor.desired_col = self.cursor.col;
                        } else {
                            // Multi-line character paste
                            let current_line = &self.buffer.lines[self.cursor.row].text;
                            let before = current_line.chars().take(self.cursor.col).collect::<String>();
                            let after = current_line.chars().skip(self.cursor.col).collect::<String>();

                            // Replace current line with first part + first yanked line
                            self.buffer.lines[self.cursor.row].text = format!("{}{}", before, register.text[0]);

                            // Insert middle yanked lines
                            for i in 1..(register.text.len() - 1) {
                                self.buffer.lines.insert(self.cursor.row + i, YLine::from(register.text[i].clone()));
                            }

                            // Insert last yanked line + remainder
                            if register.text.len() > 1 {
                                let last_yanked = &register.text[register.text.len() - 1];
                                self.buffer.lines.insert(
                                    self.cursor.row + register.text.len() - 1,
                                    YLine::from(format!("{}{}", last_yanked, after))
                                );
                            }

                            self.cursor.row += register.text.len() - 1;
                            self.cursor.col = register.text[register.text.len() - 1].len();
                            self.cursor.desired_col = self.cursor.col;
                        }
                    }
                }
            }
        }
    }

    fn enter_fuzzy_finder(&mut self, finder_type: FuzzyFinderType) {
        self.mode = Mode::FuzzyFinder;
        self.fuzzy_finder_type = Some(finder_type.clone());
        self.fuzzy_query.clear();
        self.fuzzy_selected = 0;

        // Run initial search
        match finder_type {
            FuzzyFinderType::Files => self.run_rg_files(),
            FuzzyFinderType::Grep => {
                self.fuzzy_results.clear(); // Empty until query is entered
            }
        }
    }

    fn handle_fuzzy_finder_mode(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.fuzzy_finder_type = None;
                self.fuzzy_query.clear();
                self.fuzzy_results.clear();
                self.fuzzy_selected = 0;
            }
            KeyCode::Enter => {
                self.open_selected_result();
            }
            KeyCode::Char(c) => {
                self.fuzzy_query.push(c);
                self.update_fuzzy_results();
            }
            KeyCode::Backspace => {
                self.fuzzy_query.pop();
                self.update_fuzzy_results();
            }
            KeyCode::Down | KeyCode::Char('j') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => {
                if self.fuzzy_selected < self.fuzzy_results.len().saturating_sub(1) {
                    self.fuzzy_selected += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => {
                if self.fuzzy_selected > 0 {
                    self.fuzzy_selected -= 1;
                }
            }
            _ => {}
        }
    }

    fn run_rg_files(&mut self) {
        // Use ripgrep to find files
        let output = Command::new("rg")
            .args(&["--files", "--hidden", "--glob", "!.git"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let files = String::from_utf8_lossy(&output.stdout);
                self.fuzzy_results = files.lines().map(|s| s.to_string()).collect();
            }
        }
    }

    fn run_rg_grep(&mut self, query: &str) {
        if query.is_empty() {
            self.fuzzy_results.clear();
            return;
        }

        // Use ripgrep to search text in files
        let output = Command::new("rg")
            .args(&[
                "--line-number",
                "--column",
                "--no-heading",
                "--color=never",
                "--hidden",
                "--glob", "!.git",
                query
            ])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let results = String::from_utf8_lossy(&output.stdout);
                self.fuzzy_results = results.lines().take(100).map(|s| s.to_string()).collect();
            } else {
                self.fuzzy_results.clear();
            }
        }
    }

    fn update_fuzzy_results(&mut self) {
        if let Some(ref finder_type) = self.fuzzy_finder_type.clone() {
            match finder_type {
                FuzzyFinderType::Files => {
                    // Filter files by query
                    self.run_rg_files();
                    if !self.fuzzy_query.is_empty() {
                        let query = self.fuzzy_query.to_lowercase();
                        self.fuzzy_results.retain(|f| f.to_lowercase().contains(&query));
                    }
                    self.fuzzy_results.truncate(100); // Limit results
                    self.fuzzy_selected = 0;
                }
                FuzzyFinderType::Grep => {
                    let query = self.fuzzy_query.clone();
                    self.run_rg_grep(&query);
                    self.fuzzy_selected = 0;
                }
            }
        }
    }

    fn open_selected_result(&mut self) {
        if self.fuzzy_selected < self.fuzzy_results.len() {
            let selected = &self.fuzzy_results[self.fuzzy_selected];

            match self.fuzzy_finder_type.as_ref() {
                Some(FuzzyFinderType::Files) => {
                    // Open the file
                    if let Ok(app) = App::from_file(selected) {
                        *self = app;
                    }
                }
                Some(FuzzyFinderType::Grep) => {
                    // Parse grep result: filename:line:col:text
                    let parts: Vec<&str> = selected.splitn(4, ':').collect();
                    if parts.len() >= 3 {
                        let filename = parts[0];
                        let line_num = parts[1].parse::<usize>().unwrap_or(1);

                        // Open file and jump to line
                        if let Ok(mut app) = App::from_file(filename) {
                            app.cursor.row = line_num.saturating_sub(1).min(app.buffer.lines.len().saturating_sub(1));
                            app.cursor.col = 0;
                            app.cursor.desired_col = 0;
                            *self = app;
                        }
                    }
                }
                None => {}
            }
        }

        // Exit fuzzy finder mode
        self.mode = Mode::Normal;
        self.fuzzy_finder_type = None;
        self.fuzzy_query.clear();
        self.fuzzy_results.clear();
        self.fuzzy_selected = 0;
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Build title with filename and modified status
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

        let char_num = self.cursor.get_character_number(&self.buffer);
        let position_info = Title::from(
            format!(
                " LN:{} CL:{} CHR:{} ",
                self.cursor.row + 1,  // Display as 1-indexed
                self.cursor.col + 1,  // Display as 1-indexed
                char_num
            )
            .yellow()
            .bold()
        );

        let mode_text = match self.mode {
            Mode::Normal => "-- NORMAL --",
            Mode::Insert => "-- INSERT --",
            Mode::Visual => "-- VISUAL --",
            Mode::VisualLine => "-- VISUAL LINE --",
            Mode::Command => "-- COMMAND --",
            Mode::FuzzyFinder => "-- FINDER --",
        };

        let instructions = if self.mode == Mode::Command {
            // In command mode, show the command buffer
            Title::from(Line::from(vec![
                ":".into(),
                self.command_buffer.clone().yellow(),
            ]))
        } else {
            Title::from(Line::from(vec![
                " ".into(),
                mode_text.green().bold(),
                " | Transform ".into(),
                "<F1>".blue().bold(),
                " Operation ".into(),
                "<F2>".blue().bold(),
                " File ".into(),
                "<F3> ".blue().bold(),
            ]))
        };
        let block = Block::default()
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
            .border_set(border::THICK);

        // Calculate viewport height (inner area after borders)
        let viewport_height = area.height.saturating_sub(2) as usize; // -2 for borders

        let buffer_lines: Vec<Line> = self.buffer.lines
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(viewport_height)
            .map(|(row, yline)| {
                if let Some((start_row, start_col)) = self.visual_start {
                    if self.mode == Mode::Visual || self.mode == Mode::VisualLine {
                        // Calculate selection range
                        let end_row = self.cursor.row;
                        let end_col = self.cursor.col;

                        if self.mode == Mode::VisualLine {
                            // Line-wise visual: highlight entire lines
                            let (first_line, last_line) = if start_row <= end_row {
                                (start_row, end_row)
                            } else {
                                (end_row, start_row)
                            };

                            if row >= first_line && row <= last_line {
                                let span = Span::styled(yline.text.clone(), Style::default().black().on_white());
                                return Line::from(vec![span]);
                            }
                        } else {
                            // Character-wise visual: highlight characters
                            let (start_pos, end_pos) = if (start_row, start_col) <= (end_row, end_col) {
                                ((start_row, start_col), (end_row, end_col))
                            } else {
                                ((end_row, end_col), (start_row, start_col))
                            };

                            if row >= start_pos.0 && row <= end_pos.0 {
                                let chars: Vec<char> = yline.text.chars().collect();
                                let mut spans = Vec::new();

                                for (col, ch) in chars.iter().enumerate() {
                                    let should_highlight = if row == start_pos.0 && row == end_pos.0 {
                                        col >= start_pos.1 && col <= end_pos.1
                                    } else if row == start_pos.0 {
                                        col >= start_pos.1
                                    } else if row == end_pos.0 {
                                        col <= end_pos.1
                                    } else {
                                        true
                                    };

                                    if should_highlight {
                                        spans.push(Span::styled(ch.to_string(), Style::default().black().on_white()));
                                    } else {
                                        spans.push(Span::raw(ch.to_string()));
                                    }
                                }

                                return Line::from(spans);
                            }
                        }
                    }
                }
                Line::from(yline.text.clone())
            })
            .collect();

        let buffer_text = Text::from(buffer_lines);

        Paragraph::new(buffer_text)
            .left_aligned()
            .block(block)
            .render(area, buf);

        // Render active plugins
        let ctx = plugins::PluginContext {
            buffer: &mut self.buffer.clone(), // Clone to satisfy borrow checker
            cursor: &mut self.cursor.clone(),
            mode: &mut self.mode.clone(),
            filename: &self.filename,
            modified: &mut false, // Read-only for rendering
        };
        self.plugin_manager.render(area, buf, &ctx);
    }
}

impl App {
    fn render_fuzzy_finder(&self, area: Rect, buf: &mut Buffer) {
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
        let title_text = match self.fuzzy_finder_type {
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
            let query_text = format!("> {}", self.fuzzy_query);
            let query_line = Line::from(Span::styled(query_text, Style::default().fg(Color::Yellow)));
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

            let visible_results: Vec<Line> = self.fuzzy_results
                .iter()
                .enumerate()
                .skip(self.fuzzy_selected.saturating_sub(10))
                .take(results_area.height as usize)
                .map(|(idx, result)| {
                    let display_text = if result.len() > results_area.width as usize {
                        format!("{}...", &result[..results_area.width.saturating_sub(3) as usize])
                    } else {
                        result.clone()
                    };

                    if idx == self.fuzzy_selected {
                        Line::from(Span::styled(
                            format!("> {}", display_text),
                            Style::default().fg(Color::Black).bg(Color::White)
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
}
