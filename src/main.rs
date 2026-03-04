//This is the Y editor.
//Y another editor you ask?
//Because this is the Y editor.
use std::io;
use std::env;

mod tui;
mod commands;
mod plugins;
pub mod mode;
pub mod buffer;
pub mod cursor;
pub mod view;
pub mod layout;
pub mod editor;
pub mod render;
pub mod input;
pub mod operations;
pub mod theme;

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
