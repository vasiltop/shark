use std::io;

use crossterm::{execute, terminal};

fn main() -> std::io::Result<()> {
    let mut stdout = io::stdout();

    execute!(stdout, terminal::EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::Clear(terminal::ClearType::All))?;

    Ok(())
}
