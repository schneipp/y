# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Y Editor is a terminal-based text editor built in Rust using the Ratatui TUI framework and Crossterm for terminal manipulation. The project is in early development stages.

## Build and Development Commands

```bash
# Build the project
cargo build

# Run the editor (new file)
cargo run

# Run the editor with a file
cargo run -- filename.txt

# Build and run in release mode
cargo build --release
cargo run --release -- filename.txt

# Check code without building
cargo check

# Run tests
cargo test

# Run a specific test
cargo test test_name

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Architecture

### Core Components

**Mode System**
- `Mode` enum: Normal, Insert, Visual, VisualLine, Command, and FuzzyFinder modes (vim-style)
- Start in Normal mode by default
- 'i' to enter Insert mode, 'v' for Visual, 'V' for Visual Line, ':' for Command
- Space-based shortcuts: `<space>ff` for file finder, `<space>/` for grep search
- Esc to return to Normal mode from any mode
- `visual_start: Option<(usize, usize)>` - tracks where visual selection began
- `command_buffer: String` - stores command mode input (:w, :q, etc.)
- `fuzzy_finder_type: Option<FuzzyFinderType>` - tracks finder type (Files or Grep)
- `fuzzy_query: String` - stores fuzzy finder search query
- `fuzzy_results: Vec<String>` - stores search results from ripgrep
- `fuzzy_selected: usize` - index of currently selected result

**Cursor System**
- `Cursor` struct with `row`, `col`, and `desired_col` fields
- `desired_col` maintains column position when moving through lines of varying length
- Visual cursor rendered in the terminal at buffer position
- Position indicator shown in UI: "LN:1 CL:1 CHR:0" (line, column, character number)

**Undo/Redo System**
- `undo_stack: Vec<YBuffer>` - stores previous buffer states
- `redo_stack: Vec<YBuffer>` - stores redone states for Ctrl+r
- `save_state()` method called before any buffer modification
- Undo (u) restores previous state, moves current to redo stack
- Redo (Ctrl+r) restores next state, moves current to undo stack
- Making new changes clears the redo stack
- `YBuffer` and `YLine` implement Clone for efficient state copying

**App Structure**
- `App` is the main application struct that manages the event loop and rendering
- Contains: buffer, mode, cursor, and exit flag
- Implements the Ratatui `Widget` trait for rendering the UI
- Event handling pattern: `handle_events()` → `handle_key_event()` → mode-specific handlers (`handle_normal_mode()` / `handle_insert_mode()`)

**Buffer System**
- `YBuffers`: Collection of buffers (not yet used)
- `YBuffer`: Represents a single text buffer containing lines
- `YLine`: Represents a single line of text with optional styling
- Buffer is integrated into App and displayed in the UI

**TUI Module (tui.rs)**
- `Tui` type alias for `Terminal<CrosstermBackend<Stdout>>`
- `init()`: Enters alternate screen and enables raw mode
- `restore()`: Cleans up terminal state on exit
- Always call `restore()` before exiting to prevent terminal corruption

**Command System (commands/)**
- `YCommand` trait defines the interface for editor commands
- Methods: `register_command()`, `get_argment_list()`, `execute()`
- `E` command is a placeholder implementation (src/commands/e.rs:3-16)
- Command system is designed but not yet integrated with the main event loop

### Current State

Functional vim-style modal editor with comprehensive navigation, editing, undo/redo, visual selection, and file I/O:
- **Modal editing:** Normal, Insert, Visual (character-wise), Visual Line, and Command modes
- **Basic navigation:** h/j/k/l for left/down/up/right navigation
- **Word motions:** w (word forward), b (word backward), W (WORD forward), B (WORD backward)
- **Line motions:** 0 (line start), $ (line end)
- **Buffer motions:** gg (first line), G (last line)
- **Character search:** f{char} (find forward), F{char} (find backward)
- **Insert commands:** i (insert), a (append), o (open below), O (open above)
- **Delete commands:** x (delete char), dd (delete line), dw (delete word), d$ (delete to line end), d0 (delete to line start)
- **Visual mode:** v (character-wise visual), V (line-wise visual)
  - Navigation in visual mode extends selection
  - d/x deletes selection
  - y yanks (copies) selection
  - Visual highlighting shows selected text (black on white)
  - Switch between v and V while in visual mode
- **Yank/Paste:** Full copy/paste support with register
  - yy (yank line), yw (yank word), y$ (yank to line end), y0 (yank to line start)
  - y in visual mode yanks selection
  - p (paste after), P (paste before)
  - Line-wise and character-wise yank types preserved
- **Undo/Redo:** u (undo), Ctrl+r (redo) - unlimited undo stack
- **File operations:** Load file on startup, save with :w, quit with :q
  - :w (write/save file)
  - :q (quit - warns if unsaved)
  - :wq or :x (write and quit)
  - :q! (force quit without saving)
  - Filename shown in title bar
  - [+] indicator shows modified status
- **Fuzzy Finder (Telescope-like):** Quick file navigation and text search
  - `<space>ff` - Opens file finder using ripgrep (rg --files)
  - `<space>/` - Opens text search using ripgrep with line numbers
  - Type to filter/search results in real-time
  - Ctrl+j/Down - Move selection down
  - Ctrl+k/Up - Move selection up
  - Enter - Open selected file or jump to grep result
  - Esc - Cancel and return to Normal mode
  - Popup UI: Centered, 80% width, 60% height, cyan border
  - File finder: Shows up to 100 files from current directory (respects .gitignore)
  - Grep search: Shows filename:line:col:text, opens file at exact line
  - Currently integrated directly in main.rs (planned for plugin refactor)
- **Text editing:** Character insertion, Enter for newlines, Backspace for deletion
- **Multi-key sequences:** gg, dd, dw, d$, d0, yy, yw, y$, y0, f{char}, F{char} (via pending_key state)
- Arrow keys also supported as aliases
- Buffer rendering with cursor position display
- Mode indicator in UI (bottom-left)
- Position indicator in UI (bottom-right)

### Development Patterns

**Vim Motion Implementation (IMPORTANT)**
- ALWAYS implement vim motions (hjkl) directly, not arrow keys first
- This is a vim-style editor - vim keybindings are primary
- Arrow keys can be added as aliases, but vim keys are the standard
- Multi-key sequences (like gg, dd, dw, f{char}) use the `pending_key: Option<char>` field in App
- When implementing new multi-key commands, check `pending_key` first in the handler
- Delete operations (d) use pending_key to wait for motion (w, $, 0, d, etc.)
- Character search (f/F) uses pending_key to capture the target character
- W and B are WORD motions (whitespace-separated), different from w and b (punctuation-aware)

**Undo/Redo Implementation Pattern**
- ALL buffer-modifying operations MUST call `self.save_state()` before making changes
- This includes: insert_char, insert_newline, backspace, delete operations, open line commands
- Undo/redo automatically handles cursor position clamping when buffer size changes
- When adding new editing commands, always add `save_state()` at the beginning

**Visual Mode Implementation**
- Visual mode tracks selection start position in `visual_start: Option<(usize, usize)>`
- Character-wise visual (v): highlights individual characters across lines
- Line-wise visual (V): highlights entire lines
- Selection rendering uses Span with black text on white background
- Navigation commands extend the selection from visual_start to cursor position
- Delete operations in visual mode handle both single-line and multi-line selections
- Always call `enter_normal_mode()` after visual operations to clear visual_start

**Yank/Paste System**
- `yank_register: Option<YankRegister>` - stores yanked text with type information
- `YankRegister` contains `text: Vec<String>` and `yank_type: YankType`
- `YankType` enum: `Character` (within lines) or `Line` (whole lines)
- Line-wise yanks: yy, visual line selection - paste as complete lines
- Character-wise yanks: yw, y$, y0, visual char selection - paste inline
- Paste operations (p/P) respect the yank type:
  - Line yanks insert entire lines before/after current line
  - Character yanks insert text at/after cursor position
- Both single-line and multi-line pastes supported for each type
- Paste operations call `save_state()` for undo support

**File System**
- `filename: Option<String>` - current file being edited
- `modified: bool` - tracks if buffer has unsaved changes
- Load file on startup: `cargo run -- filename.txt`
- File doesn't exist: creates new buffer with filename set
- Command mode (`:` commands):
  - `:w` - save file (only if filename is set)
  - `:q` - quit (blocked if modified)
  - `:wq` or `:x` - save and quit
  - `:q!` - force quit without saving
- Title bar shows filename and [+] if modified
- Command mode shows `:` prompt with typed command
- All buffer modifications set `modified = true`
- Saving file sets `modified = false`

**Fuzzy Finder System (Current Implementation)**
- `FuzzyFinderType` enum: `Files` or `Grep` - determines search type
- `space_pressed: bool` - tracks if space key was pressed for space-based shortcuts
- Space-based shortcuts in Normal mode:
  - `<space>f` → pending, then `f` → enters Files finder
  - `<space>/` → enters Grep finder
- `enter_fuzzy_finder()` - initializes finder state and runs ripgrep
- `run_rg_files()` - executes `rg --files --hidden --glob !.git` to find files
- `run_rg_grep(query)` - executes `rg --line-number --column --no-heading` for text search
- `handle_fuzzy_finder_mode()` - processes input in fuzzy finder:
  - Character input updates query and re-runs search (for Grep mode)
  - File mode filters results client-side for better performance
  - Backspace removes last character from query
  - Enter opens selected result via `open_selected_result()`
- `open_selected_result()` - handles selection:
  - Files: opens file using `App::from_file()`
  - Grep: parses `filename:line:col:text` format and jumps to line
- `render_fuzzy_finder()` - renders popup UI with Block widget
  - Uses Span for styled text (yellow query, black-on-white selection)
  - Query line shows `> {query}` at top
  - Results show `> item` for selected, `  item` for others
  - Scrolling: shows 10 items before/after selection (centered)
- Pattern: All styling must use `Span::styled()` with `Style::default()`, not chained methods
- Pattern: Block widgets consumed by `.render()`, call `.inner()` first if needed

When adding new commands:
1. Create a new module in `src/commands/`
2. Implement the `YCommand` trait
3. Export from `src/commands/mod.rs`
4. Register in the main app (registration system TBD)

When modifying the UI:
- The `Widget` implementation for `App` renders the main editor view
- Uses Ratatui's declarative widget system
- Terminal drawing happens in the event loop via `terminal.draw(|frame| ...)`
- Cursor position is set via `frame.set_cursor(x, y)` in `render_frame()`

## Future Architecture: Plugin System

### Motivation
The fuzzy finder is currently integrated directly into main.rs, adding ~300 lines of code to the App struct and its methods. As more features are added (LSP, git integration, file explorer, etc.), the main.rs file will become unmaintainable. A plugin system will allow features to be:
- Developed independently in separate modules
- Enabled/disabled by users
- Tested in isolation
- Maintained without affecting core editor functionality

### Proposed Plugin Architecture

**Plugin Trait**
```rust
pub trait Plugin {
    /// Plugin name for identification
    fn name(&self) -> &str;

    /// Initialize plugin (load config, setup state, etc.)
    fn init(&mut self, ctx: &mut PluginContext) -> Result<(), PluginError>;

    /// Handle key events, return true if event was consumed
    fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool;

    /// Render plugin UI (if active)
    fn render(&self, area: Rect, buf: &mut Buffer, ctx: &PluginContext);

    /// Plugin is currently active (has focus)
    fn is_active(&self) -> bool;

    /// Cleanup on exit
    fn cleanup(&mut self) -> Result<(), PluginError>;
}
```

**PluginContext**
```rust
pub struct PluginContext<'a> {
    pub buffer: &'a mut YBuffer,
    pub cursor: &'a mut Cursor,
    pub mode: &'a mut Mode,
    pub filename: Option<String>,
    pub modified: bool,
    // Methods for common operations
    pub open_file: fn(&str) -> Result<App, std::io::Error>,
    pub save_state: fn(),
    // Event bus for plugin communication
    pub events: &'a mut EventBus,
}
```

**Plugin Manager**
```rust
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    active_plugin: Option<usize>,
}

impl PluginManager {
    /// Register a plugin
    pub fn register(&mut self, plugin: Box<dyn Plugin>);

    /// Distribute key event to active plugin or all plugins
    pub fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool;

    /// Render all active plugins
    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &PluginContext);

    /// Activate a plugin by name
    pub fn activate(&mut self, name: &str) -> Result<(), PluginError>;

    /// Deactivate current plugin
    pub fn deactivate(&mut self);
}
```

### Refactoring Plan: Fuzzy Finder as Plugin

**Step 1: Create Plugin Module Structure**
```
src/
  plugins/
    mod.rs              # Plugin trait and manager
    fuzzy_finder/
      mod.rs            # FuzzyFinderPlugin struct
      files.rs          # File finder implementation
      grep.rs           # Grep finder implementation
      ui.rs             # Rendering logic
```

**Step 2: Extract Fuzzy Finder State**
Move from App struct to FuzzyFinderPlugin:
- `fuzzy_finder_type: Option<FuzzyFinderType>`
- `fuzzy_query: String`
- `fuzzy_results: Vec<String>`
- `fuzzy_selected: usize`
- `space_pressed: bool` (or handle in plugin)

**Step 3: Implement Plugin Trait**
```rust
pub struct FuzzyFinderPlugin {
    finder_type: Option<FuzzyFinderType>,
    query: String,
    results: Vec<String>,
    selected: usize,
    active: bool,
}

impl Plugin for FuzzyFinderPlugin {
    fn name(&self) -> &str { "fuzzy_finder" }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        // Handle Ctrl+j, Ctrl+k, Enter, Esc, character input
        // Return true if consumed, false otherwise
    }

    fn render(&self, area: Rect, buf: &mut Buffer, ctx: &PluginContext) {
        // Render popup UI (current render_fuzzy_finder logic)
    }

    // ... other methods
}
```

**Step 4: Register Plugin in Main**
```rust
fn main() {
    let mut app = App::default();
    app.plugin_manager.register(Box::new(FuzzyFinderPlugin::new()));
    // ... event loop
}
```

**Step 5: Update Event Handling**
Replace direct fuzzy finder handling with:
```rust
// In handle_normal_mode():
if self.space_pressed {
    if key.code == KeyCode::Char('f') {
        self.plugin_manager.activate("fuzzy_finder")?;
        // Set finder type to Files
    }
}

// In main event loop:
let mut ctx = PluginContext { /* ... */ };
if self.plugin_manager.handle_key(key, &mut ctx) {
    return Ok(()); // Plugin consumed the event
}
// ... continue with normal handling
```

### Benefits
- **Modularity:** Each plugin is self-contained with its own state and logic
- **Testability:** Plugins can be unit tested independently
- **Maintainability:** Core editor stays focused, plugins handle specific features
- **Extensibility:** New plugins can be added without modifying core
- **Performance:** Plugins can be loaded on-demand or disabled entirely
- **Separation of Concerns:** Clear boundaries between core editor and features

### Future Plugins
Once the plugin system is in place, other features can be implemented as plugins:
- **LSP Plugin:** Language server protocol integration
- **Git Plugin:** Git status, blame, diff in gutter
- **File Explorer:** Tree view file browser (like NERDTree)
- **Debugger Plugin:** DAP (Debug Adapter Protocol) integration
- **Terminal Plugin:** Embedded terminal (like vim-floaterm)
- **Snippet Plugin:** Code snippet expansion
- **Session Plugin:** Save/restore editor sessions
- **Syntax Plugin:** Custom syntax highlighting beyond default
