mod builtin;

use ratatui::style::Color;

/// Syntax highlighting colors
#[derive(Debug, Clone)]
pub struct SyntaxColors {
    pub keyword: Color,
    pub function: Color,
    pub function_macro: Color,
    pub type_: Color,
    pub type_builtin: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub constant_builtin: Color,
    pub variable_builtin: Color,
    pub operator: Color,
    pub default: Color,
}

/// UI element colors
#[derive(Debug, Clone)]
pub struct UiColors {
    pub background: Color,
    pub foreground: Color,
    pub line_number_fg: Color,
    pub visual_selection_fg: Color,
    pub visual_selection_bg: Color,
    pub secondary_cursor_bg: Color,
    pub border_active: Color,
    pub border_inactive: Color,
    pub status_mode_normal: Color,
    pub status_mode_insert: Color,
    pub status_mode_visual: Color,
    pub status_mode_command: Color,
    pub status_position_fg: Color,
    pub status_keybind_fg: Color,
    pub status_title_fg: Color,
    // Fuzzy finder popup
    pub popup_border: Color,
    pub popup_query: Color,
    pub popup_selected_fg: Color,
    pub popup_selected_bg: Color,
}

/// A complete color theme
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub syntax: SyntaxColors,
    pub ui: UiColors,
}

/// Manages available themes and the active selection
pub struct ThemeManager {
    themes: Vec<Theme>,
    current_idx: usize,
}

impl ThemeManager {
    pub fn new() -> Self {
        let themes = builtin::all();
        Self {
            themes,
            current_idx: 0, // monokai is first
        }
    }

    pub fn current(&self) -> &Theme {
        &self.themes[self.current_idx]
    }

    pub fn set_theme(&mut self, name: &str) -> Result<(), String> {
        if let Some(idx) = self.themes.iter().position(|t| t.name == name) {
            self.current_idx = idx;
            Ok(())
        } else {
            Err(format!("Unknown theme: {}", name))
        }
    }

    pub fn list(&self) -> Vec<&str> {
        self.themes.iter().map(|t| t.name.as_str()).collect()
    }
}

impl std::fmt::Debug for ThemeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThemeManager")
            .field("current", &self.themes[self.current_idx].name)
            .finish()
    }
}
