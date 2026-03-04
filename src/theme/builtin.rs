use ratatui::style::Color;
use super::{SyntaxColors, Theme, UiColors};

/// Monokai classic (default)
fn monokai() -> Theme {
    Theme {
        name: "monokai".into(),
        syntax: SyntaxColors {
            keyword:          Color::Rgb(0xF9, 0x26, 0x72), // pink
            function:         Color::Rgb(0xA6, 0xE2, 0x2E), // green
            function_macro:   Color::Rgb(0x66, 0xD9, 0xEF), // blue
            type_:            Color::Rgb(0x66, 0xD9, 0xEF), // blue
            type_builtin:     Color::Rgb(0x66, 0xD9, 0xEF), // blue italic
            string:           Color::Rgb(0xE6, 0xDB, 0x74), // yellow
            number:           Color::Rgb(0xAE, 0x81, 0xFF), // purple
            comment:          Color::Rgb(0x75, 0x71, 0x5E), // gray-brown
            constant_builtin: Color::Rgb(0xAE, 0x81, 0xFF), // purple
            variable_builtin: Color::Rgb(0xFD, 0x97, 0x1F), // orange
            operator:         Color::Rgb(0xF9, 0x26, 0x72), // pink
            default:          Color::Rgb(0xF8, 0xF8, 0xF2), // white
        },
        ui: UiColors {
            background:          Color::Rgb(0x27, 0x28, 0x22),
            foreground:          Color::Rgb(0xF8, 0xF8, 0xF2),
            line_number_fg:      Color::Rgb(0x90, 0x90, 0x8A),
            visual_selection_fg: Color::Rgb(0xF8, 0xF8, 0xF2),
            visual_selection_bg: Color::Rgb(0x49, 0x48, 0x3E),
            secondary_cursor_bg: Color::Rgb(0x49, 0x48, 0x3E),
            border_active:       Color::Rgb(0xA6, 0xE2, 0x2E),
            border_inactive:     Color::Rgb(0x75, 0x71, 0x5E),
            status_mode_normal:  Color::Rgb(0xA6, 0xE2, 0x2E), // green
            status_mode_insert:  Color::Rgb(0x66, 0xD9, 0xEF), // blue
            status_mode_visual:  Color::Rgb(0xFD, 0x97, 0x1F), // orange
            status_mode_command: Color::Rgb(0xF9, 0x26, 0x72), // pink
            status_position_fg:  Color::Rgb(0xE6, 0xDB, 0x74), // yellow
            status_keybind_fg:   Color::Rgb(0x66, 0xD9, 0xEF), // blue
            status_title_fg:     Color::Rgb(0xF8, 0xF8, 0xF2),
            popup_border:        Color::Rgb(0x66, 0xD9, 0xEF),
            popup_query:         Color::Rgb(0xE6, 0xDB, 0x74),
            popup_selected_fg:   Color::Rgb(0x27, 0x28, 0x22),
            popup_selected_bg:   Color::Rgb(0xA6, 0xE2, 0x2E),
            ghost_text:          Color::Rgb(0x75, 0x71, 0x5E), // gray-brown (same as comments)
        },
    }
}

/// Gruvbox dark
fn gruvbox_dark() -> Theme {
    Theme {
        name: "gruvbox-dark".into(),
        syntax: SyntaxColors {
            keyword:          Color::Rgb(0xFB, 0x49, 0x34),
            function:         Color::Rgb(0xFA, 0xBD, 0x2F),
            function_macro:   Color::Rgb(0x8E, 0xC0, 0x7C),
            type_:            Color::Rgb(0x83, 0xA5, 0x98),
            type_builtin:     Color::Rgb(0x83, 0xA5, 0x98),
            string:           Color::Rgb(0xB8, 0xBB, 0x26),
            number:           Color::Rgb(0xD3, 0x86, 0x9B),
            comment:          Color::Rgb(0x92, 0x83, 0x74),
            constant_builtin: Color::Rgb(0xD3, 0x86, 0x9B),
            variable_builtin: Color::Rgb(0x8E, 0xC0, 0x7C),
            operator:         Color::Rgb(0xFE, 0x80, 0x19),
            default:          Color::Rgb(0xEB, 0xDB, 0xB2),
        },
        ui: UiColors {
            background:          Color::Rgb(0x28, 0x28, 0x28),
            foreground:          Color::Rgb(0xEB, 0xDB, 0xB2),
            line_number_fg:      Color::Rgb(0x66, 0x5C, 0x54),
            visual_selection_fg: Color::Rgb(0x28, 0x28, 0x28),
            visual_selection_bg: Color::Rgb(0xFA, 0xBD, 0x2F),
            secondary_cursor_bg: Color::Rgb(0x50, 0x49, 0x45),
            border_active:       Color::Rgb(0xEB, 0xDB, 0xB2),
            border_inactive:     Color::Rgb(0x66, 0x5C, 0x54),
            status_mode_normal:  Color::Rgb(0xB8, 0xBB, 0x26),
            status_mode_insert:  Color::Rgb(0x83, 0xA5, 0x98),
            status_mode_visual:  Color::Rgb(0xFA, 0xBD, 0x2F),
            status_mode_command: Color::Rgb(0xFB, 0x49, 0x34),
            status_position_fg:  Color::Rgb(0xFA, 0xBD, 0x2F),
            status_keybind_fg:   Color::Rgb(0x83, 0xA5, 0x98),
            status_title_fg:     Color::Rgb(0xEB, 0xDB, 0xB2),
            popup_border:        Color::Rgb(0x83, 0xA5, 0x98),
            popup_query:         Color::Rgb(0xFA, 0xBD, 0x2F),
            popup_selected_fg:   Color::Rgb(0x28, 0x28, 0x28),
            popup_selected_bg:   Color::Rgb(0xB8, 0xBB, 0x26),
            ghost_text:          Color::Rgb(0x66, 0x5C, 0x54), // fg3
        },
    }
}

/// Catppuccin Mocha
fn catppuccin_mocha() -> Theme {
    Theme {
        name: "catppuccin-mocha".into(),
        syntax: SyntaxColors {
            keyword:          Color::Rgb(0xCB, 0xA6, 0xF7), // mauve
            function:         Color::Rgb(0x89, 0xB4, 0xFA), // blue
            function_macro:   Color::Rgb(0x94, 0xE2, 0xD5), // teal
            type_:            Color::Rgb(0xF9, 0xE2, 0xAF), // yellow
            type_builtin:     Color::Rgb(0xF9, 0xE2, 0xAF),
            string:           Color::Rgb(0xA6, 0xE3, 0xA1), // green
            number:           Color::Rgb(0xFA, 0xB3, 0x87), // peach
            comment:          Color::Rgb(0x6C, 0x70, 0x86), // overlay0
            constant_builtin: Color::Rgb(0xFA, 0xB3, 0x87),
            variable_builtin: Color::Rgb(0x94, 0xE2, 0xD5),
            operator:         Color::Rgb(0x89, 0xDC, 0xEB), // sky
            default:          Color::Rgb(0xCD, 0xD6, 0xF4), // text
        },
        ui: UiColors {
            background:          Color::Rgb(0x1E, 0x1E, 0x2E), // base
            foreground:          Color::Rgb(0xCD, 0xD6, 0xF4), // text
            line_number_fg:      Color::Rgb(0x58, 0x5B, 0x70), // surface2
            visual_selection_fg: Color::Rgb(0x1E, 0x1E, 0x2E),
            visual_selection_bg: Color::Rgb(0x89, 0xB4, 0xFA),
            secondary_cursor_bg: Color::Rgb(0x45, 0x47, 0x5A), // surface1
            border_active:       Color::Rgb(0xCD, 0xD6, 0xF4),
            border_inactive:     Color::Rgb(0x58, 0x5B, 0x70),
            status_mode_normal:  Color::Rgb(0xA6, 0xE3, 0xA1), // green
            status_mode_insert:  Color::Rgb(0x89, 0xB4, 0xFA), // blue
            status_mode_visual:  Color::Rgb(0xF9, 0xE2, 0xAF), // yellow
            status_mode_command: Color::Rgb(0xF3, 0x8B, 0xA8), // red
            status_position_fg:  Color::Rgb(0xF9, 0xE2, 0xAF),
            status_keybind_fg:   Color::Rgb(0x89, 0xB4, 0xFA),
            status_title_fg:     Color::Rgb(0xCD, 0xD6, 0xF4),
            popup_border:        Color::Rgb(0x89, 0xB4, 0xFA),
            popup_query:         Color::Rgb(0xF9, 0xE2, 0xAF),
            popup_selected_fg:   Color::Rgb(0x1E, 0x1E, 0x2E),
            popup_selected_bg:   Color::Rgb(0xA6, 0xE3, 0xA1),
            ghost_text:          Color::Rgb(0x58, 0x5B, 0x70), // surface2
        },
    }
}

/// Default dark (matches the original hardcoded colors)
fn default_dark() -> Theme {
    Theme {
        name: "dark".into(),
        syntax: SyntaxColors {
            keyword:          Color::Red,
            function:         Color::Yellow,
            function_macro:   Color::Cyan,
            type_:            Color::Blue,
            type_builtin:     Color::Blue,
            string:           Color::Green,
            number:           Color::Magenta,
            comment:          Color::DarkGray,
            constant_builtin: Color::Magenta,
            variable_builtin: Color::Cyan,
            operator:         Color::Red,
            default:          Color::Reset,
        },
        ui: UiColors {
            background:          Color::Reset,
            foreground:          Color::Reset,
            line_number_fg:      Color::DarkGray,
            visual_selection_fg: Color::Black,
            visual_selection_bg: Color::White,
            secondary_cursor_bg: Color::DarkGray,
            border_active:       Color::Reset,
            border_inactive:     Color::DarkGray,
            status_mode_normal:  Color::Green,
            status_mode_insert:  Color::Blue,
            status_mode_visual:  Color::Yellow,
            status_mode_command: Color::Red,
            status_position_fg:  Color::Yellow,
            status_keybind_fg:   Color::Blue,
            status_title_fg:     Color::Reset,
            popup_border:        Color::Cyan,
            popup_query:         Color::Yellow,
            popup_selected_fg:   Color::Black,
            popup_selected_bg:   Color::White,
            ghost_text:          Color::DarkGray,
        },
    }
}

pub fn all() -> Vec<Theme> {
    vec![monokai(), gruvbox_dark(), catppuccin_mocha(), default_dark()]
}
