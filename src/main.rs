use std::{
    fs::File,
    io::{self, Stdout, Write},
};

use clap::Parser;
use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEvent, KeyEventKind},
    execute, queue,
    style::{self, Print},
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
    cursor_prev: (u16, u16),
}

impl Editor {
    fn new(mut stdout: Stdout, mut text: Rope) -> std::io::Result<Self> {
        execute!(stdout, terminal::EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;

        let mut indices = Vec::new();

        for (i, c) in text.chars().enumerate() {
            if c == '\n' {
                indices.push(i + 1);
            }
        }

        for (ref mut offset, i) in indices.into_iter().enumerate() {
            text.insert_char(i + *offset, '\r');
            *offset += 1;
        }

        Ok(Self {
            stdout,
            text,
            cursor_prev: (0, 0),
        })
    }

    fn update_display(&mut self) -> std::io::Result<()> {
        execute!(
            self.stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0),
        )?;

        for line in self.text.lines() {
            queue!(self.stdout, style::Print(line))?;
        }

        execute!(
            self.stdout,
            cursor::MoveTo(self.cursor_prev.0, self.cursor_prev.1)
        )?;

        self.stdout.flush()?;
        Ok(())
    }

    fn close(&mut self) -> std::io::Result<()> {
        execute!(self.stdout, terminal::LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    fn get_cursor_index(&self) -> std::io::Result<usize> {
        let pos = cursor::position()?;

        let mut count = 0;
        for (i, line) in self.text.lines().enumerate() {
            if i == pos.1.into() {
                count += pos.0 as usize;
            } else {
                count += line.len_chars();
            }
        }

        Ok(count)
    }

    fn handle_events(&mut self) -> std::io::Result<bool> {
        let event = read()?;

        match event {
            Event::Key(event) if event.kind == KeyEventKind::Press => {
                if event.code == KeyCode::Esc {
                    return Ok(false);
                }

                if event.code == KeyCode::Enter {
                    self.text.insert(self.get_cursor_index()?, "\n");
                }

                if let KeyCode::Char(c) = event.code {
                    self.text.insert_char(self.get_cursor_index()?, c);
                }
            }
            _ => {}
        };
        Ok(true)
    }
}

fn main() -> std::io::Result<()> {
    let stdout = io::stdout();
    let args = Args::parse();
    let text = Rope::from_reader(File::open(args.filename)?)?;

    let mut editor = Editor::new(stdout, text)?;
    editor.update_display()?;

    loop {
        if !editor.handle_events()? {
            break;
        }

        editor.update_display()?;
    }

    editor.close()?;

    Ok(())
}
