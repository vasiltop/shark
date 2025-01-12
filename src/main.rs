use std::{
    any::Any,
    cmp,
    fs::File,
    io::{self, BufWriter, Stdout, Write},
};

use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue, style,
    style::{Color::*, Print},
    terminal,
};

use ropey::Rope;

use clap::Parser;
use tree_sitter::Node;

#[derive(Parser, Debug)]
struct Args {
    filename: String,
}

enum CursorMovement {
    Up,
    Down,
    Left,
    Right,
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
        const COLORS: [style::Color; 12] = [
            Red,
            DarkRed,
            Green,
            DarkGreen,
            Yellow,
            DarkYellow,
            Blue,
            DarkBlue,
            Magenta,
            DarkMagenta,
            Cyan,
            DarkCyan,
        ];

        execute!(
            self.stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0),
        )?;

        let mut parser = tree_sitter::Parser::new();

        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();

        let tree = parser.parse(self.text.to_string(), None);

        let mut nodes = Vec::new();

        if let Some(tree) = tree {
            nodes.append(&mut Self::expand_nodes(tree.root_node()));

            let mut prev_end = 0;

            for node in nodes {
                let index = self.get_index_from_pos((
                    node.start_position().column as u16,
                    node.start_position().row as u16,
                ))?;

                if index > prev_end {
                    queue!(
                        self.stdout,
                        style::SetForegroundColor(style::Color::White),
                        Print(self.text.slice(prev_end..index))
                    )?;
                }

                let diff = node.end_position().column - node.start_position().column;
                let end = index + diff;
                prev_end = end;

                queue!(
                    self.stdout,
                    style::SetForegroundColor(COLORS[(node.kind_id() % 12) as usize]),
                    Print(self.text.slice(index..end).to_string())
                )?;
            }
        }

        execute!(
            self.stdout,
            cursor::MoveTo(self.cursor_pos.0, self.cursor_pos.1)
        )?;

        self.stdout.flush()?;
        Ok(())
    }

    fn expand_nodes(node: Node) -> Vec<Node> {
        let mut nodes = Vec::new();

        if node.child_count() == 0 {
            nodes.push(node);
        }

        for n in node.children(&mut node.walk()) {
            let children = Self::expand_nodes(n);
            for child in children {
                nodes.push(child);
            }
        }

        nodes
    }

    fn close(&mut self) -> std::io::Result<()> {
        execute!(self.stdout, terminal::LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    fn get_index_from_pos(&self, pos: (u16, u16)) -> std::io::Result<usize> {
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

    fn get_current_line_length(&self) -> usize {
        for (i, line) in self.text.lines().enumerate() {
            if i == self.cursor_pos.1.into() {
                return line.len_chars().saturating_sub(2);
            }
        }

        0
    }

    fn attempt_cursor_move(&mut self, movement: CursorMovement) {
        match movement {
            CursorMovement::Up => self.cursor_pos.1 = self.cursor_pos.1.saturating_sub(1),
            CursorMovement::Down => {
                if self.cursor_pos.1 < (self.text.lines().len() - 2) as u16 {
                    self.cursor_pos.1 += 1;
                }
            }
            CursorMovement::Right => {
                if self.cursor_pos.0 < self.get_current_line_length() as u16 {
                    self.cursor_pos.0 += 1;
                }
            }
            CursorMovement::Left => self.cursor_pos.0 = self.cursor_pos.0.saturating_sub(1),
        }
    }

    fn handle_events(&mut self) -> std::io::Result<bool> {
        let event = read()?;

        match event {
            Event::Key(event) if event.kind == KeyEventKind::Press => match event.code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Enter => {
                    self.text
                        .insert(self.get_index_from_pos(cursor::position()?)?, "\r\n");
                    self.cursor_pos.0 = 0;
                    self.cursor_pos.1 += 1;
                }
                KeyCode::Up => self.attempt_cursor_move(CursorMovement::Up),
                KeyCode::Down => self.attempt_cursor_move(CursorMovement::Down),
                KeyCode::Left => self.attempt_cursor_move(CursorMovement::Left),
                KeyCode::Right => self.attempt_cursor_move(CursorMovement::Right),
                KeyCode::Backspace => {
                    if self.cursor_pos != (0, 0) {
                        let idx = self.get_index_from_pos(cursor::position()?)?;
                        if self.cursor_pos.0 > 0 {
                            self.text.remove(idx - 1..idx);
                            self.attempt_cursor_move(CursorMovement::Left)
                        } else if self.get_current_line_length() == 0 {
                            self.text.remove(idx..idx + 2);
                            self.attempt_cursor_move(CursorMovement::Up);
                            self.cursor_pos.0 = self.get_current_line_length() as u16;
                        } else {
                            self.attempt_cursor_move(CursorMovement::Up);
                            self.cursor_pos.0 = self.get_current_line_length() as u16;
                            self.text.remove(idx - 2..idx);
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if c == 's' && event.modifiers == KeyModifiers::CONTROL {
                        self.save();
                    } else {
                        self.text
                            .insert_char(self.get_index_from_pos(cursor::position()?)?, c);
                        self.cursor_pos.0 += 1;
                    }
                }
                _ => {}
            },
            _ => {}
        };

        self.cursor_pos.0 = cmp::min(self.cursor_pos.0, self.get_current_line_length() as u16);
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
