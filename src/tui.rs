use std::io::{self, stdout, BufWriter, Stdout};

use crossterm::{cursor::SetCursorStyle, execute, terminal::*};
use ratatui::prelude::*;

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<BufWriter<Stdout>>>;

/// Initialize the terminal with buffered output for single-syscall frame writes
pub fn init() -> io::Result<Tui> {
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(BufWriter::with_capacity(64 * 1024, stdout()));
    Terminal::new(backend)
}

/// Restore the terminal to its original state
pub fn restore() -> io::Result<()> {
    execute!(stdout(), SetCursorStyle::DefaultUserShape, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
