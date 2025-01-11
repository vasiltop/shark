use std::{
    fs::File,
    io::{self, Write},
};

use clap::Parser;
use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEventKind},
    execute, terminal,
};
use ropey::Rope;

#[derive(Parser, Debug)]
struct Args {
    filename: String,
}

fn main() -> std::io::Result<()> {
    let mut stdout = io::stdout();

    let args = Args::parse();

    execute!(stdout, terminal::EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    let text = Rope::from_reader(File::open(args.filename)?);

    loop {
        let event = read()?;

        match event {
            Event::Key(event) if event.kind == KeyEventKind::Press => {
                print!("{:?}", event);
                stdout.flush()?;

                if event.code == KeyCode::Esc {
                    break;
                }
            }
            _ => {}
        }
    }

    execute!(stdout, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}
