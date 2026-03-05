```

  _   _
  \ \ / /
   \ v /
    | |
    |_|    (why editor)

```

# y

A terminal editor for people who want vim's power without vim's complexity. Think microvim. Think chadnano. Think "I just want to edit code fast."

Built in Rust. Starts instantly. Vim grammar baked in. LSP and tree-sitter out of the box. No 200-line config file. And if vim scares you — there's a **normie mode** with `Ctrl+S`, `Ctrl+F`, `Ctrl+Z` and all that. No judgement.

## Install

```bash
curl -sSf https://raw.githubusercontent.com/schneipp/y/main/installer.sh | sh
```

Or grab a [pre-built binary](https://github.com/schneipp/y/releases/latest), or build from source:

```bash
git clone https://github.com/schneipp/y.git && cd y && cargo build --release
```

## Why y?

Because every editor is either too much or too little.

Nano is simple but you outgrow it in a week. Vim is powerful but you spend a month configuring it. VS Code is nice until you SSH into a server. Neovim is great once you've written 400 lines of Lua.

**y** is the middle ground. You open it, it works. Vim keybindings are there. LSP autocomplete is there. Syntax highlighting is there. No plugins to install, no package managers to wrangle, no config to write.

## What's in the box

- **Modal editing** — Normal, Insert, Visual, Visual Line, Command modes. The vim motions you know.
- **LSP support** — Autocomplete, go-to-definition, format-on-save for 9 languages. Auto-detected, one-command install via `:lspsetup`.
- **Tree-sitter highlighting** — Real parsing, not regex.
- **Multi-cursor** — `Ctrl+N` to select word, again for next match. `c` to change all at once.
- **Splits** — `:sp`, `:vs`, `Ctrl+w` navigation.
- **File tree** — `<Space>e`, NeoTree-style with git status and icons.
- **Git client** — `<Space>g` to stage, commit, push without leaving the editor.
- **Fuzzy finder** — `<Space>ff` for files, `<Space>/` for grep.
- **Themes** — Monokai, Gruvbox, Catppuccin, Dark. `<Space>ft` to switch.
- **Settings** — `F2` to toggle everything. No config file needed.
- **Normie mode** — For people who don't speak vim. `Ctrl+S` save, `Ctrl+F` find, arrow keys.

## Quick reference

| Key | What it does |
|-----|-------------|
| `h/j/k/l` | Move |
| `i/a/o` | Insert mode |
| `v/V` | Visual select |
| `w/b` | Word jump |
| `gg/G` | Top/bottom |
| `gd` | Go to definition |
| `Ctrl+O` | Jump back |
| `u/Ctrl+R` | Undo/redo |
| `/` | Search |
| `<Space>e` | File tree |
| `<Space>g` | Git |
| `<Space>ff` | Find files |
| `:w` `:q` `:wq` | Save, quit, both |
| `F1` | Full keybinding list |
| `F2` | Settings |

## License

MIT