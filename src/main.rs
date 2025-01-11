use std::{
    fs::File,
    io::{self, Stdout, Write},
};

use clap::Parser;
use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEventKind},
    execute, queue,
    style::Print,
    terminal,
};
use ropey::Rope;

#[derive(Parser, Debug)]
struct Args {
    filename: String,
}

struct Editor {
    text: Rope,
    stdout: Stdout,
}

impl Editor {
    fn new(mut stdout: Stdout, text: Rope) -> std::io::Result<Self> {
        execute!(stdout, terminal::EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;

        Ok(Self { stdout, text })
    }

    fn update_display(&mut self) -> std::io::Result<()> {
        execute!(
            self.stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        for line in self.text.lines() {
            queue!(self.stdout, Print(line))?;
        }

        self.stdout.flush()?;

        Ok(())
    }

    fn close(&mut self) -> std::io::Result<()> {
        execute!(self.stdout, terminal::LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
}

fn main() -> std::io::Result<()> {
    let stdout = io::stdout();
    let args = Args::parse();
    let text = Rope::from_reader(File::open(args.filename)?)?;

    let mut editor = Editor::new(stdout, text)?;
    editor.update_display()?;

    loop {
        let event = read()?;

        match event {
            Event::Key(event) if event.kind == KeyEventKind::Press => {
                if event.code == KeyCode::Esc {
                    break;
                }
            }
            _ => {}
        }
    }

    editor.close()?;

    Ok(())
}
