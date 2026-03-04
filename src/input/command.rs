use crossterm::event::{KeyCode, KeyEvent};

use crate::editor::Editor;
use crate::mode::Mode;

impl Editor {
    pub fn handle_command_mode(&mut self, key_event: KeyEvent) {
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

    pub fn execute_command(&mut self) {
        let cmd = self.command_buffer.clone();
        let cmd = cmd.trim();

        if cmd.starts_with("e ") {
            let filename = cmd[2..].trim();
            if !filename.is_empty() {
                self.open_file_in_view(filename);
            }
        } else if cmd.starts_with("theme ") {
            let name = cmd[6..].trim();
            if !name.is_empty() {
                self.switch_theme(name);
            }
        } else {
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
                "sp" => self.split_horizontal(),
                "vs" => self.split_vertical(),
                "lspsetup" => {
                    self.show_lsp_setup();
                    self.command_buffer.clear();
                    return;
                }
                "lspinfo" => {
                    self.show_lsp_info();
                    self.command_buffer.clear();
                    return;
                }
                _ => {}
            }
        }

        self.mode = Mode::Normal;
        self.command_buffer.clear();
    }
}
