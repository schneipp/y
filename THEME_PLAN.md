# Theme Support Implementation Plan for Y Editor

## 1. Overview

This document describes a plan for adding user-configurable color themes to the Y
editor. The goal is to replace every hardcoded `Color::*` value in the rendering
pipeline with a lookup into a `Theme` struct, allow themes to be defined in TOML
files, ship several built-in themes, and let the user switch themes at runtime
with `:theme <name>`.

---

## 2. Theme Data Structure

### 2.1 Syntax token colors

A theme must provide a color for every tree-sitter capture name that
`SyntaxHighlighter::color_for_capture()` currently handles, plus a sensible
default for unknown captures.

```
SyntaxColors {
    keyword:          Color,   // "keyword", "keyword.operator"
    function:         Color,   // "function", "function.call"
    function_macro:   Color,   // "function.macro"
    type_:            Color,   // "type"
    type_builtin:     Color,   // "type.builtin"
    string:           Color,   // "string"
    number:           Color,   // "number"
    comment:          Color,   // "comment"
    constant_builtin: Color,   // "constant.builtin"
    variable_builtin: Color,   // "variable.builtin"
    operator:         Color,   // future: operators
    default:          Color,   // fallback for unrecognized captures
}
```

### 2.2 UI element colors

Every hardcoded color in `buffer_widget.rs` and `status_bar.rs` must become
configurable.

```
UiColors {
    // Buffer area
    background:            Color,
    foreground:            Color,       // plain text
    line_number_fg:        Color,       // line-number gutter (currently DarkGray)
    line_number_bg:        Color,

    // Visual selection (currently black-on-white)
    visual_selection_fg:   Color,
    visual_selection_bg:   Color,

    // Multi-cursor secondary cursors (currently bg DarkGray)
    secondary_cursor_bg:   Color,

    // Status bar / border
    border_active:         Color,       // active pane border  (currently Color::Reset)
    border_inactive:       Color,       // inactive pane border (currently DarkGray)
    status_bar_bg:         Color,
    status_bar_fg:         Color,
    status_mode_normal:    Color,       // mode indicator colors (currently green)
    status_mode_insert:    Color,
    status_mode_visual:    Color,
    status_mode_command:   Color,
    status_position_fg:    Color,       // LN/CL/CHR text (currently yellow)
    status_keybind_fg:     Color,       // keybind hints (currently blue)
    status_title_fg:       Color,       // title bar text

    // Cursor (terminal cursor style could be extended later)
    cursor_fg:             Color,
    cursor_bg:             Color,
}
```

### 2.3 Top-level Theme struct

```
pub struct Theme {
    pub name:   String,
    pub syntax: SyntaxColors,
    pub ui:     UiColors,
}
```

All three structs derive `Clone`, `Debug`, `serde::Deserialize`, and
`serde::Serialize`.  `ratatui::style::Color` already implements Serialize /
Deserialize if the `serde` feature is enabled on the `ratatui` crate
(see dependency change below).

---

## 3. Theme File Format (TOML)

### 3.1 Why TOML

TOML is the idiomatic Rust configuration format; the project already depends on
`serde`, and adding `toml` is a single extra dependency. TOML is also more
pleasant to hand-edit than JSON.

### 3.2 Example file: `themes/gruvbox-dark.toml`

```toml
name = "gruvbox-dark"

[syntax]
keyword          = "#fb4934"
function         = "#fabd2f"
function_macro   = "#8ec07c"
type_            = "#83a598"
type_builtin     = "#83a598"
string           = "#b8bb26"
number           = "#d3869b"
comment          = "#928374"
constant_builtin = "#d3869b"
variable_builtin = "#8ec07c"
operator         = "#fe8019"
default          = "#ebdbb2"

[ui]
background            = "#282828"
foreground            = "#ebdbb2"
line_number_fg        = "#665c54"
line_number_bg        = "#282828"
visual_selection_fg   = "#282828"
visual_selection_bg   = "#fabd2f"
secondary_cursor_bg   = "#504945"
border_active         = "#ebdbb2"
border_inactive       = "#665c54"
status_bar_bg         = "#3c3836"
status_bar_fg         = "#ebdbb2"
status_mode_normal    = "#b8bb26"
status_mode_insert    = "#83a598"
status_mode_visual    = "#fabd2f"
status_mode_command   = "#fb4934"
status_position_fg    = "#fabd2f"
status_keybind_fg     = "#83a598"
status_title_fg       = "#ebdbb2"
cursor_fg             = "#282828"
cursor_bg             = "#ebdbb2"
```

Colors are represented as hex RGB strings (`"#rrggbb"`).  The deserialization
layer (section 4.3) will parse these into `ratatui::style::Color::Rgb(r, g, b)`.
Named ratatui colors (`"Red"`, `"DarkGray"`, etc.) will also be accepted as a
convenience.

### 3.3 File locations searched (in order)

1. `$XDG_CONFIG_HOME/y/themes/*.toml`  (user custom themes)
2. Built-in themes compiled into the binary via `include_str!`

---

## 4. Loading and Storage

### 4.1 New dependency

Add to `Cargo.toml`:

```toml
toml = "0.8"
```

Enable the `serde` feature on `ratatui`:

```toml
ratatui = { version = "0.26.1", features = ["serde"] }
```

### 4.2 ThemeManager

A new struct responsible for loading, caching, and switching themes.

```
pub struct ThemeManager {
    themes: HashMap<String, Theme>,   // all known themes (built-in + discovered)
    current: String,                  // name of the active theme
}
```

Methods:

- `new() -> Self` -- loads built-in themes, scans XDG dir, selects "dark" as
  default.
- `load_builtin_themes(&mut self)` -- deserializes the `include_str!` TOML
  blobs for each shipped theme.
- `load_user_themes(&mut self)` -- walks `$XDG_CONFIG_HOME/y/themes/` for
  `*.toml` files; logs and skips files that fail to parse.
- `set_theme(&mut self, name: &str) -> Result<(), String>` -- switches the
  active theme.
- `current_theme(&self) -> &Theme` -- returns a reference to the active theme.
- `list_themes(&self) -> Vec<&str>` -- returns available theme names (for
  future tab completion in command mode).

### 4.3 Color deserialization

Implement a custom serde deserializer (or a newtype wrapper `ThemeColor`) that
accepts:

- Hex strings: `"#282828"` -> `Color::Rgb(0x28, 0x28, 0x28)`
- Named colors: `"Red"`, `"DarkGray"`, `"Reset"` -> corresponding
  `ratatui::style::Color` variants
- Indexed colors: `{ "indexed": 243 }` -> `Color::Indexed(243)`

This keeps the TOML human-friendly while supporting 256-color terminals.

### 4.4 Default / fallback theme

A `Theme::default_dark()` function returns a hardcoded dark theme that matches
the current color scheme of Y so the editor looks identical if no theme is
configured. This is the ultimate fallback even if all file loading fails.

---

## 5. Wiring the Theme into the Syntax Highlighter

### 5.1 Current state

`SyntaxHighlighter::color_for_capture()` (line 140 of
`src/plugins/syntax_highlighter/mod.rs`) contains a hardcoded match statement
that maps capture names to `Color` values.

`SyntaxHighlighter` stores the resolved colors in `line_highlights` as
`Vec<Vec<(usize, usize, Color)>>`.

### 5.2 Plan

1. Add a field `syntax_colors: SyntaxColors` to `SyntaxHighlighter`.

2. Change `color_for_capture(&self, capture_name: &str) -> Color` to look up
   `self.syntax_colors` instead of the hardcoded match:

   ```
   fn color_for_capture(&self, capture_name: &str) -> Color {
       match capture_name {
           "function" | "function.call" => self.syntax_colors.function,
           "function.macro"             => self.syntax_colors.function_macro,
           "type" | "type.builtin"      => self.syntax_colors.type_,
           "string"                     => self.syntax_colors.string,
           "number"                     => self.syntax_colors.number,
           "comment"                    => self.syntax_colors.comment,
           "keyword" | "keyword.operator" => self.syntax_colors.keyword,
           "constant.builtin"           => self.syntax_colors.constant_builtin,
           "variable.builtin"           => self.syntax_colors.variable_builtin,
           _                            => self.syntax_colors.default,
       }
   }
   ```

3. Add a public method `set_syntax_colors(&mut self, colors: SyntaxColors)` so
   the `ThemeManager` (or the `Editor` itself) can push new colors when the
   theme changes.

4. When `set_syntax_colors` is called, invalidate `line_highlights` so the next
   `parse_buffer` recomputes colors with the new palette.

### 5.3 Highlight cache format -- no change

The cache type `Vec<Vec<(usize, usize, Color)>>` stays the same.
`BufferWidget::build_highlighted_spans()` already reads `Color` from the cache,
so it needs no change for syntax colors.

---

## 6. Wiring the Theme into the Buffer Widget

### 6.1 Current hardcoded colors in `buffer_widget.rs`

| Location (approx line) | Current value | Theme field |
|---|---|---|
| Line 141 `Style::default().fg(Color::DarkGray)` | line numbers | `ui.line_number_fg` |
| Line 57-58 `Style::default().black().on_white()` | visual selection | `ui.visual_selection_fg`, `ui.visual_selection_bg` |
| Line 161-163 `Style::default().black().on_white()` | visual line selection | same |
| Line 227 `Style::default().bg(Color::DarkGray)` | secondary cursor | `ui.secondary_cursor_bg` |
| Line 217 `Style::default()` | plain text | `ui.foreground` |

### 6.2 Plan

1. Add a `theme: &'a Theme` field to `BufferWidget`.

2. Replace every hardcoded `Color::*` / `Style::*` with a lookup into
   `self.theme.ui.*`.

3. The `Editor::render_frame()` method already constructs `BufferWidget`; it
   will pass `&self.theme_manager.current_theme()` (or a `&Theme` stored on
   `Editor`).

---

## 7. Wiring the Theme into the Status Bar

### 7.1 Current hardcoded colors in `status_bar.rs`

| Location | Current value | Theme field |
|---|---|---|
| Line 34 `.bold()` on title | title text, no explicit fg | `ui.status_title_fg` |
| Line 43 `.yellow().bold()` | position info | `ui.status_position_fg` |
| Line 64 `.green().bold()` | mode text (always green) | `ui.status_mode_normal` etc. |
| Line 66-68 `.blue().bold()` | keybind hints | `ui.status_keybind_fg` |
| Line 75-76 `Color::Reset` | active border | `ui.border_active` |
| Line 78 `Color::DarkGray` | inactive border | `ui.border_inactive` |

### 7.2 Plan

1. Add a `theme: &'a Theme` field to `StatusBar`.

2. Replace each hardcoded color call with the appropriate `self.theme.ui.*`
   lookup.

3. The mode indicator color will be context-sensitive:
   ```
   let mode_color = match self.mode {
       Mode::Normal     => self.theme.ui.status_mode_normal,
       Mode::Insert     => self.theme.ui.status_mode_insert,
       Mode::Visual | Mode::VisualLine => self.theme.ui.status_mode_visual,
       Mode::Command    => self.theme.ui.status_mode_command,
       Mode::FuzzyFinder => self.theme.ui.status_mode_normal,
   };
   ```

---

## 8. Storing the Theme on the Editor

### 8.1 Option A: ThemeManager on Editor (recommended)

Add `pub theme_manager: ThemeManager` to the `Editor` struct.  Initialize it in
`Editor::default()` and `Editor::from_file()`.

The `render_frame()` method obtains `&Theme` via
`self.theme_manager.current_theme()` and passes it down to `BufferWidget` and
`StatusBar`.

### 8.2 Why not on PluginManager?

Themes are not a plugin -- they are core editor state that cuts across every
rendering call. The plugin system is designed for event-handling extensions, not
for configuration that every widget needs. Keeping `ThemeManager` on `Editor`
mirrors how `mode`, `filename`, etc. are stored.

### 8.3 Propagating to SyntaxHighlighter

When the theme changes (at startup or via `:theme`), `Editor` must push the new
`SyntaxColors` into the `SyntaxHighlighter` plugin:

```
if let Some(plugin) = self.plugin_manager.get_mut("syntax_highlighter") {
    if let Some(hl) = plugin.as_any_mut()
        .downcast_mut::<SyntaxHighlighter>()
    {
        hl.set_syntax_colors(theme.syntax.clone());
    }
}
```

This is the same downcast pattern already used in `buffer_widget.rs` line 201
and `editor.rs` line 314.

---

## 9. Runtime Theme Switching: `:theme <name>`

### 9.1 Command parsing

In `src/input/command.rs`, the `execute_command()` match currently handles
`"w"`, `"q"`, `"wq"`, `"q!"`, `"sp"`, `"vs"`.

Add a new arm:

```
_ if cmd.starts_with("theme ") => {
    let name = cmd.strip_prefix("theme ").unwrap().trim();
    self.switch_theme(name);
}
```

### 9.2 `Editor::switch_theme()`

A new method on `Editor`:

```
pub fn switch_theme(&mut self, name: &str) {
    match self.theme_manager.set_theme(name) {
        Ok(()) => {
            let theme = self.theme_manager.current_theme().clone();
            // Push syntax colors to the highlighter
            self.apply_theme_to_plugins(&theme);
        }
        Err(msg) => {
            // Optionally show error in status bar (future work: message line)
        }
    }
}
```

### 9.3 Future: tab completion

The `list_themes()` method on `ThemeManager` makes it straightforward to add tab
completion for `:theme ` in a future pass. This plan does not implement tab
completion.

---

## 10. Built-in Themes

Ship four themes compiled into the binary. Each is a TOML string embedded via
`include_str!`.

### 10.1 `dark` (default)

Matches the current hardcoded colors exactly so existing users see no visual
change.

| Token | Color |
|---|---|
| keyword | Red |
| function | Yellow |
| function_macro | Cyan |
| type | Blue |
| string | Green |
| number | Magenta |
| comment | DarkGray |
| constant_builtin | Magenta |
| variable_builtin | Cyan |
| UI foreground | Reset (terminal default) |
| UI background | Reset |
| line numbers | DarkGray |
| visual selection | black on white |
| border (active) | Reset |
| border (inactive) | DarkGray |
| mode indicator | Green |
| position info | Yellow |
| keybind hints | Blue |

### 10.2 `light`

A light-background variant. Syntax colors are adjusted for readability on a
white or light-gray terminal background.

| Token | Color |
|---|---|
| keyword | `#af0000` (dark red) |
| function | `#875f00` (dark yellow/brown) |
| function_macro | `#005f5f` (dark teal) |
| type | `#0000af` (dark blue) |
| string | `#005f00` (dark green) |
| number | `#8700af` (dark purple) |
| comment | `#808080` (gray) |
| UI background | Reset |
| UI foreground | `#000000` |
| visual selection | white on `#0060c0` |

### 10.3 `gruvbox-dark`

Based on the gruvbox palette (see TOML example in section 3.2).

### 10.4 `catppuccin-mocha`

Based on the Catppuccin Mocha palette:

| Token | Color |
|---|---|
| keyword | `#cba6f7` (mauve) |
| function | `#89b4fa` (blue) |
| function_macro | `#94e2d5` (teal) |
| type | `#f9e2af` (yellow) |
| string | `#a6e3a1` (green) |
| number | `#fab387` (peach) |
| comment | `#6c7086` (overlay0) |
| UI background | `#1e1e2e` (base) |
| UI foreground | `#cdd6f4` (text) |
| visual selection | `#1e1e2e` on `#89b4fa` |
| line numbers | `#585b70` (surface2) |
| border active | `#cdd6f4` |
| border inactive | `#585b70` |

---

## 11. New File Layout

```
src/
  theme/
    mod.rs              -- pub mod types; pub mod manager; pub mod builtin;
    types.rs            -- Theme, SyntaxColors, UiColors, ThemeColor serde
    manager.rs          -- ThemeManager: load, switch, list
    builtin.rs          -- include_str!() for each built-in theme; register fn

themes/                 -- TOML source files for built-in themes
  dark.toml             -- (compiled into binary via include_str!)
  light.toml
  gruvbox-dark.toml
  catppuccin-mocha.toml
```

In `src/main.rs`, add `pub mod theme;`.

No other existing modules are moved or renamed. The only existing files that
are modified are:

| File | Change |
|---|---|
| `Cargo.toml` | Add `toml = "0.8"`, enable `ratatui/serde` feature |
| `src/main.rs` | Add `pub mod theme;` |
| `src/editor.rs` | Add `theme_manager: ThemeManager` field; init in constructors; pass `&Theme` to widgets; add `switch_theme()` / `apply_theme_to_plugins()` methods |
| `src/render/buffer_widget.rs` | Add `theme: &'a Theme` field; replace all hardcoded `Color` values |
| `src/render/status_bar.rs` | Add `theme: &'a Theme` field; replace all hardcoded `Color` values |
| `src/plugins/syntax_highlighter/mod.rs` | Add `syntax_colors: SyntaxColors` field; change `color_for_capture()`; add `set_syntax_colors()` |
| `src/input/command.rs` | Add `:theme <name>` arm in `execute_command()` |

---

## 12. Implementation Order

Work should be done in this sequence to keep the editor compiling and functional
after each step.

### Step 1: Define theme types (no behavioral change)

- Create `src/theme/types.rs` with `Theme`, `SyntaxColors`, `UiColors`,
  `ThemeColor` serde wrapper.
- Create `src/theme/mod.rs`.
- Add `toml` dep and `ratatui/serde` feature to `Cargo.toml`.
- Add `pub mod theme;` to `main.rs`.
- Confirm it compiles.

### Step 2: Create built-in theme TOML files + ThemeManager

- Write `themes/dark.toml` matching current hardcoded colors.
- Write `themes/light.toml`, `themes/gruvbox-dark.toml`,
  `themes/catppuccin-mocha.toml`.
- Implement `src/theme/builtin.rs` with `include_str!`.
- Implement `src/theme/manager.rs` with `ThemeManager`.
- Write unit tests: parse each built-in TOML, round-trip serialize.

### Step 3: Wire ThemeManager into Editor

- Add `theme_manager` field to `Editor`.
- Initialize in `Editor::default()` and `Editor::from_file()`.
- No rendering changes yet -- just plumbing.

### Step 4: Wire theme into SyntaxHighlighter

- Add `syntax_colors` field to `SyntaxHighlighter`.
- Change `color_for_capture()` to use it.
- Add `set_syntax_colors()` method.
- At `Editor` initialization, push `theme.syntax` into the highlighter.

### Step 5: Wire theme into BufferWidget

- Add `theme` field to `BufferWidget`.
- Replace all hardcoded colors.
- Update `Editor::render_frame()` to pass `&theme`.

### Step 6: Wire theme into StatusBar

- Add `theme` field to `StatusBar`.
- Replace all hardcoded colors.
- Update `Editor::render_frame()` to pass `&theme`.

### Step 7: Add `:theme` command

- Add the command parsing arm in `execute_command()`.
- Implement `Editor::switch_theme()`.
- Test switching at runtime.

### Step 8: User theme directory scanning

- Implement `ThemeManager::load_user_themes()`.
- Wire it into `ThemeManager::new()`.

---

## 13. Testing Strategy

- **Unit tests** in `src/theme/types.rs`: parse hex colors, named colors,
  round-trip serde.
- **Unit tests** in `src/theme/manager.rs`: load built-in, set/get, list,
  error on unknown name.
- **Integration test**: construct `Editor::default()`, verify
  `theme_manager.current_theme().name == "dark"`, switch to "gruvbox-dark",
  verify syntax colors changed on the highlighter.
- **Manual test**: open a Rust file, visually verify each built-in theme looks
  correct, switch themes with `:theme gruvbox-dark` at runtime.

---

## 14. Future Extensions (out of scope for this plan)

- Per-language syntax color overrides in theme files.
- Background color fill for the full buffer area (requires clearing the ratatui
  buffer cells with the theme background).
- Cursor shape/blink configuration per mode in the theme.
- `:theme` tab completion and a theme preview picker.
- Saving the selected theme to a config file so it persists across sessions.
- Status line layout customization (beyond colors).
- Underline, italic, bold style attributes on syntax tokens.
