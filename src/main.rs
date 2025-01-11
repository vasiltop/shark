use std::{
    fs::File,
    io::{self, BufWriter, Stdout, Write},
};

use clap::Parser;

use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEventKind, KeyModifiers},
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
    cursor_pos: (u16, u16),
    filename: String,
}

impl Editor {
    fn new(mut stdout: Stdout, mut text: Rope, filename: String) -> std::io::Result<Self> {
        execute!(stdout, terminal::EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;

        let mut indices = Vec::new();

        for (i, c) in text.chars().enumerate() {
            if c == '\n' {
                indices.push(i);
            }
        }

        for (ref mut offset, i) in indices.into_iter().enumerate() {
            text.insert_char(i + *offset, '\r');
            *offset += 1;
        }

        Ok(Self {
            stdout,
            text,
            cursor_pos: (0, 0),
            filename,
        })
    }

    fn update_display(&mut self) -> std::io::Result<()> {
        execute!(
            self.stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0),
        )?;

        for line in self.text.lines() {
            queue!(self.stdout, Print(line))?;
        }

        execute!(
            self.stdout,
            cursor::MoveTo(self.cursor_pos.0, self.cursor_pos.1)
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
                break;
            } else {
                count += line.len_chars();
            }
        }

        Ok(count)
    }

    fn save(&mut self) {
        let mut file = BufWriter::new(File::create(&self.filename).unwrap());
        let bytes = self.text.bytes().filter(|c| *c != b'\r');

        for b in bytes {
            file.write_all(&[b]).unwrap();
        }

        file.flush().unwrap();
    }

    fn handle_events(&mut self) -> std::io::Result<bool> {
        let event = read()?;

        match event {
            Event::Key(event) if event.kind == KeyEventKind::Press => {
                if event.code == KeyCode::Esc {
                    return Ok(false);
                }

                if event.code == KeyCode::Enter {
                    self.text.insert(self.get_cursor_index()?, "\r\n");
                    self.cursor_pos.0 = 0;
                    self.cursor_pos.1 += 1;
                }

                if event.code == KeyCode::Up {
                    self.cursor_pos.1 -= 1;
                }

                if event.code == KeyCode::Down {
                    self.cursor_pos.1 += 1;
                }

                if event.code == KeyCode::Right {
                    self.cursor_pos.0 += 1;
                }

                if event.code == KeyCode::Left {
                    self.cursor_pos.0 -= 1;
                }

                if event.code == KeyCode::Delete {
                    let idx = self.get_cursor_index()?;
                    self.text.remove(idx..idx + 1);
                }

                if event.code == KeyCode::Backspace {
                    let idx = self.get_cursor_index()?;
                    self.text.remove(idx - 1..idx);
                    self.cursor_pos.0 -= 1;
                }

                if event.code == KeyCode::Char('s') && event.modifiers == KeyModifiers::CONTROL {
                    self.save();
                } else if let KeyCode::Char(c) = event.code {
                    self.text.insert_char(self.get_cursor_index()?, c);
                    self.cursor_pos.0 += 1;
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

    let file = File::open(&args.filename)?;
    let text = Rope::from_reader(&file)?;

    let mut editor = Editor::new(stdout, text, args.filename)?;
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
