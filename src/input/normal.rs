use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::Editor;
use crate::mode::Mode;

impl Editor {
    pub fn handle_normal_mode(&mut self, key_event: KeyEvent) {
        // Handle space-based shortcuts (two-key sequences: <space>X or <space>XY)
        if self.space_pressed {
            self.space_pressed = false;
            match key_event.code {
                KeyCode::Char('f') => {
                    // <space>f starts the <space>ff sequence
                    self.pending_key = Some('f');
                    return;
                }
                KeyCode::Char('b') => {
                    // <space>b starts the <space>bb sequence
                    self.pending_key = Some('b');
                    return;
                }
                KeyCode::Char('/') => {
                    if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
                        if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                            fuzzy_plugin.activate(crate::plugins::js_fuzzy_finder::FuzzyFinderType::Grep);
                            self.mode = Mode::FuzzyFinder;
                        }
                    }
                    return;
                }
                _ => {
                    // Not a recognized space combo, fall through
                }
            }
        }

        if let KeyCode::Char(' ') = key_event.code {
            self.space_pressed = true;
            return;
        }

        // Handle Ctrl+w prefix for split commands
        if self.ctrl_w_pressed {
            self.ctrl_w_pressed = false;
            match key_event.code {
                KeyCode::Char('s') => {
                    self.split_horizontal();
                    return;
                }
                KeyCode::Char('v') => {
                    self.split_vertical();
                    return;
                }
                KeyCode::Char('w') => {
                    self.focus_next_view();
                    return;
                }
                KeyCode::Char('h') => {
                    self.focus_direction_left();
                    return;
                }
                KeyCode::Char('j') => {
                    self.focus_direction_down();
                    return;
                }
                KeyCode::Char('k') => {
                    self.focus_direction_up();
                    return;
                }
                KeyCode::Char('l') => {
                    self.focus_direction_right();
                    return;
                }
                KeyCode::Char('q') => {
                    self.close_current_view();
                    return;
                }
                _ => {}
            }
        }

        // Ctrl+w prefix
        if key_event.code == KeyCode::Char('w') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
            self.ctrl_w_pressed = true;
            return;
        }

        // Handle multi-key sequences
        if let Some(pending) = self.pending_key {
            self.pending_key = None;
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
                // <space>ff — fuzzy find files
                ('f', KeyCode::Char('f')) => {
                    if let Some(plugin) = self.plugin_manager.get_mut("js_fuzzy_finder") {
                        if let Some(fuzzy_plugin) = plugin.as_any_mut().downcast_mut::<crate::plugins::js_fuzzy_finder::JsFuzzyFinderPlugin>() {
                            fuzzy_plugin.activate(crate::plugins::js_fuzzy_finder::FuzzyFinderType::Files);
                            self.mode = Mode::FuzzyFinder;
                        }
                    }
                }
                // <space>ft — theme picker
                ('f', KeyCode::Char('t')) => {
                    self.show_theme_picker();
                }
                // <space>bb — buffer picker
                ('b', KeyCode::Char('b')) => {
                    self.show_buffer_picker();
                }
                // f<char> — find character forward (vim motion)
                ('f', KeyCode::Char(c)) => self.find_char_forward(c),
                ('F', KeyCode::Char(c)) => self.find_char_backward(c),
                _ => {}
            }
            return;
        }

        match key_event.code {
            // Ctrl-modified commands
            KeyCode::Char('r') if key_event.modifiers.contains(KeyModifiers::CONTROL) => self.redo(),
            KeyCode::Char('f') if key_event.modifiers.contains(KeyModifiers::CONTROL) => self.page_down(),
            KeyCode::Char('b') if key_event.modifiers.contains(KeyModifiers::CONTROL) => self.page_up(),
            KeyCode::Char('d') if key_event.modifiers.contains(KeyModifiers::CONTROL) => self.half_page_down(),
            KeyCode::Char('u') if key_event.modifiers.contains(KeyModifiers::CONTROL) => self.half_page_up(),
            // Multi-cursor: select word under cursor then find next (like VSCode Ctrl+D)
            KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.select_word_or_next_match();
            }
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
            KeyCode::Char('^') => self.move_to_first_non_whitespace(),
            KeyCode::Char('_') => self.move_to_first_non_whitespace(),
            KeyCode::Char('$') => self.move_to_line_end(),
            KeyCode::Char('G') => self.goto_last_line(),
            KeyCode::Char('g') => self.pending_key = Some('g'),
            // Editing
            KeyCode::Char('x') => self.delete_char(),
            KeyCode::Char('d') => self.pending_key = Some('d'),
            KeyCode::Char('y') => self.pending_key = Some('y'),
            KeyCode::Char('p') => self.paste_after(),
            KeyCode::Char('P') => self.paste_before(),
            KeyCode::Char('f') => self.pending_key = Some('f'),
            KeyCode::Char('F') => self.pending_key = Some('F'),
            KeyCode::Char('%') => self.goto_matching_bracket(),
            KeyCode::Char('u') => self.undo(),
            // Visual modes
            KeyCode::Char('v') => self.enter_visual_mode(),
            KeyCode::Char('V') => self.enter_visual_line_mode(),
            // Command mode
            KeyCode::Char(':') => self.enter_command_mode(),
            // Search
            KeyCode::Char('/') => self.enter_search_mode(),
            KeyCode::Char('n') => self.search_next(),
            KeyCode::Char('N') => self.search_prev(),
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
}
