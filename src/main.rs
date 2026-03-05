//This is the Y editor.
//Y another editor you ask?
//Because this is the Y editor.
use std::env;
use std::io;

pub mod buffer;
mod commands;
pub mod completion;
pub mod config;
pub mod cursor;
pub mod editor;
pub mod input;
pub mod layout;
pub mod lsp;
pub mod mode;
pub mod operations;
mod plugins;
pub mod render;
pub mod theme;
mod tui;
pub mod view;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut terminal = tui::init()?;
    let mut editor = if args.len() > 1 {
        editor::Editor::from_file(&args[1])?
    } else {
        editor::Editor::default()
    };

    let result = editor.run(&mut terminal);
    tui::restore()?;
    result
}
