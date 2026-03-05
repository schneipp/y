<p align="center">
  <h1 align="center">y</h1>
  <p align="center">A fast, modern terminal editor built in Rust.</p>
  <p align="center">Vim keybindings. LSP intelligence. Zero config.</p>
</p>

---

**y** is a terminal text editor that combines the speed of vim with the smartness of modern IDEs. It starts in milliseconds, understands your code through LSP and tree-sitter, and gets out of your way.

## Features

### Vim-First Editing
Full modal editing with the keybindings your fingers already know. Normal, Insert, Visual, Visual Line, and Command modes. Motions, operators, text objects — it all works.

### LSP Integration
Built-in language server support with auto-detection for 8 languages out of the box:

| Language | Server |
|----------|--------|
| Rust | rust-analyzer |
| TypeScript/JavaScript | typescript-language-server |
| Python | pyright |
| Go | gopls |
| C/C++ | clangd |
| Lua | lua-language-server |
| Zig | zls |
| Bash | bash-language-server |

- **Autocomplete** — popup with fuzzy filtering as you type
- **Ghost text** — inline preview of the top suggestion
- **Go to definition** — `gd` to jump to a symbol's definition
- **Jump back** — `Ctrl+O` to return to where you were
- **Format on save** — LSP formatting applied automatically on `:w`
- **Progress tracking** — see indexing status in the status bar

Toggle servers with `:lspsetup`, `F2` settings, or debug with `:lspinfo`.

### Smart Indentation
Press Enter and the cursor lands exactly where it should:
- Preserves indentation from the current line
- Auto-indents after `{`, `(`, `[`, `:`
- Splits bracket pairs: `{|}` becomes three properly indented lines
- Detects your indent style (tabs, 2-space, 4-space) from the file

### Multi-Cursor Editing
VSCode-style multi-cursor that actually works:

1. **`Ctrl+N`** on a word — selects it
2. **`Ctrl+N`** again — selects the next occurrence
3. Keep going — wraps around the file
4. **`c`** — delete all selections, type the replacement everywhere at once
5. **`d`** — delete all selections
6. **`Esc`** — back to one cursor

Also supports `Ctrl+Up/Down` for column cursors.

### Split Views
`:sp` and `:vs` for horizontal and vertical splits. `Ctrl+w` prefix for navigation (`h/j/k/l`), `Ctrl+w q` to close. Each split can show a different buffer.

### Syntax Highlighting
Tree-sitter powered parsing for accurate, fast highlighting. No regex hacks.

### Themes
Four built-in themes, switchable on the fly:

- **Monokai** (default)
- **Gruvbox Dark**
- **Catppuccin Mocha**
- **Dark**

Switch with `<space>ft` (picker), `F2` settings, or `:theme <name>`. Your choice persists across sessions.

### Fuzzy Finder
`<space>ff` to find files, `<space>/` to grep across the project. Powered by ripgrep.

### Plugin System
JavaScript plugin runtime via Deno. Extend the editor with JS plugins that have access to buffers, cursors, and the filesystem.

### Settings Dialog
Press `F2` to open the settings dialog — available in both Vim and Normie modes. Toggle editor mode, pick a theme, and enable/disable LSP servers without touching the config file.

### Persistent Config
Settings saved to `~/.config/y/config.toml`. Theme, LSP servers, editor mode, keybinding overrides — everything remembered.

## Install

```bash
curl -sSf https://raw.githubusercontent.com/schneipp/y/main/installer.sh | sh
```

Or manually:

```bash
git clone https://github.com/schneipp/y.git
cd y
cargo build --release
cp target/release/y ~/.local/bin/
```

## Usage

```bash
y                    # new buffer
y src/main.rs        # open a file
```

### Key Reference

| Key | Mode | Action |
|-----|------|--------|
| `h/j/k/l` | Normal | Move cursor |
| `i/a/o/O` | Normal | Enter insert mode |
| `v/V` | Normal | Visual / Visual Line |
| `d/c/y` | Visual | Delete / Change / Yank selection |
| `Ctrl+N` | Normal/Visual | Select word / next occurrence |
| `Ctrl+N/P` | Insert | Navigate autocomplete popup |
| `Enter` | Insert | Accept completion (or newline) |
| `Ctrl+L` | Insert | Accept ghost text suggestion |
| `w/b/W/B` | Normal | Word motions |
| `0/^/$` | Normal | Line start / first char / line end |
| `gg/G` | Normal | First / last line |
| `Ctrl+D/U` | Normal | Half-page down/up |
| `Ctrl+F/B` | Normal | Full page down/up |
| `f/F` | Normal | Find char forward/backward |
| `gd` | Normal | Go to definition (LSP) |
| `Ctrl+O` | Normal | Jump back |
| `u/Ctrl+R` | Normal | Undo / Redo |
| `p/P` | Normal | Paste after/before |
| `:w` | Command | Save (with LSP format) |
| `:q` / `:q!` | Command | Quit / Force quit |
| `:wq` / `:x` | Command | Save and quit |
| `:e <file>` | Command | Open file |
| `:sp` / `:vs` | Command | Split horizontal / vertical |
| `:theme <name>` | Command | Switch theme |
| `:lspsetup` | Command | Toggle language servers |
| `:lspinfo` | Command | LSP debug info |
| `<space>ff` | Normal | Fuzzy find files |
| `<space>/` | Normal | Grep project |
| `<space>bb` | Normal | Buffer picker |
| `<space>ft` | Normal | Theme picker |
| `Ctrl+W s/v` | Normal | Split horizontal / vertical |
| `Ctrl+W h/j/k/l` | Normal | Navigate splits |
| `Ctrl+W q` | Normal | Close split |
| `F1` | Any | Keybindings help |
| `F2` | Any | Settings dialog |

## Philosophy

- **Start fast** — no startup splash, no loading bars
- **Vim grammar** — if you know vim, you know y
- **Smart defaults** — LSP, tree-sitter, and sensible indentation without a 200-line config
- **Honest code** — ~3k lines of Rust, no abstraction astronautics

## License

MIT
