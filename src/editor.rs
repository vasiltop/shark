use std::{
    fs::File,
    io::{BufWriter, Stdout, Write},
};

use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{self, Color::*, Print},
    terminal::{self, ClearType},
};
use ropey::Rope;
use tree_sitter::Node;

pub struct Editor {
    rope: Rope,
    stdout: Stdout,
    filename: String,
    scroll: usize,
}

enum CursorMovement {
    Up,
    Down,
    Left,
    Right,
}

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

impl Editor {
    pub fn new(stdout: Stdout, mut rope: Rope, filename: String) -> Self {
        let mut indices = Vec::new();

        for (i, c) in rope.chars().enumerate() {
            if c == '\n' {
                indices.push(i);
            }
        }

        for (ref mut offset, i) in indices.into_iter().enumerate() {
            rope.insert_char(i + *offset, '\r');
            *offset += 1;
        }

        Self {
            stdout,
            rope,
            filename,
            scroll: 0,
        }
    }

    pub fn init(&mut self) -> std::io::Result<()> {
        execute!(
            self.stdout,
            terminal::EnterAlternateScreen,
            cursor::EnableBlinking,
            cursor::SetCursorStyle::BlinkingBar,
            cursor::MoveTo(0, 0)
        )?;
        terminal::enable_raw_mode()?;
        self.redraw()?;

        Ok(())
    }

    pub fn close(&mut self) -> std::io::Result<()> {
        execute!(self.stdout, terminal::LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    fn save(&mut self) {
        let mut file = BufWriter::new(File::create(&self.filename).unwrap());
        let bytes = self.rope.bytes().filter(|c| *c != b'\r');

        for b in bytes {
            file.write_all(&[b]).unwrap();
        }

        file.flush().unwrap();
    }

    pub fn step(&mut self) -> std::io::Result<bool> {
        let event = read()?;

        match event {
            Event::Key(event) if event.kind == KeyEventKind::Press => match event.code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Up => self.attempt_cursor_move(CursorMovement::Up)?,
                KeyCode::Down => self.attempt_cursor_move(CursorMovement::Down)?,
                KeyCode::Left => self.attempt_cursor_move(CursorMovement::Left)?,
                KeyCode::Right => self.attempt_cursor_move(CursorMovement::Right)?,
                KeyCode::Char(c) => {
                    if c == 's' && event.modifiers == KeyModifiers::CONTROL {
                        self.save();
                    } else {
                        self.rope.insert_char(self.get_cursor_index()?, c);
                        self.attempt_cursor_move(CursorMovement::Right)?;
                        self.redraw()?;
                    }
                }
                KeyCode::Enter => {
                    self.rope.insert(self.get_cursor_index()?, "\r\n");
                    self.attempt_cursor_move(CursorMovement::Down)?;
                    execute!(self.stdout, cursor::MoveToColumn(0))?;
                    self.redraw()?;
                }
                KeyCode::Backspace => {
                    let pos = cursor::position()?;
                    let idx = self.get_cursor_index()?;

                    if pos.0 > 0 {
                        self.rope.remove(idx - 1..idx);
                        self.attempt_cursor_move(CursorMovement::Left)?;
                    } else if self.get_current_line_len()? == 0 && self.get_line_number()? != 0 {
                        if pos.1 == 0 {
                            self.scroll -= 1;
                        }

                        self.rope.remove(idx..idx + 2);
                        self.attempt_cursor_move(CursorMovement::Up)?;
                        let line_length = self.get_current_line_len()?;
                        execute!(self.stdout, cursor::MoveToColumn(line_length as u16))?;
                    } else if self.get_line_number()? != 0 {
                        if pos.1 == 0 {
                            self.scroll -= 1;
                        }

                        self.attempt_cursor_move(CursorMovement::Up)?;
                        let line_length = self.get_current_line_len()?;
                        execute!(self.stdout, cursor::MoveToColumn(line_length as u16))?;
                        self.rope.remove(idx - 2..idx);
                    }

                    self.redraw()?;
                }
                _ => {}
            },
            _ => {}
        }

        let line_len = self.get_current_line_len()? as u16;

        execute!(
            self.stdout,
            cursor::MoveTo(
                std::cmp::min(cursor::position()?.0, line_len),
                cursor::position()?.1
            )
        )?;

        Ok(true)
    }

    fn get_cursor_index(&self) -> std::io::Result<usize> {
        let mut pos = cursor::position()?;
        pos.1 += self.scroll as u16;
        Ok(self.get_rope_index((pos.0 as usize, pos.1 as usize)))
    }

    // pos represents the position from the start of the file, not the viewport
    fn get_rope_index(&self, pos: (usize, usize)) -> usize {
        let mut count = 0;

        for (i, line) in self.rope.lines().enumerate() {
            if i >= pos.1 {
                count += pos.0;
                break;
            } else {
                count += line.len_chars();
            }
        }

        count
    }

    fn redraw(&mut self) -> std::io::Result<()> {
        execute!(
            self.stdout,
            cursor::Hide,
            terminal::Clear(ClearType::All),
            cursor::SavePosition,
            cursor::MoveTo(0, 0),
        )?;
        let mut parser = tree_sitter::Parser::new();

        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();

        let tree = parser.parse(self.rope.to_string(), None).unwrap();

        let mut nodes = Vec::new();
        nodes.append(&mut Self::expand_node(tree.root_node()));

        let mut last_pos = self.get_rope_index((0, self.scroll));

        for node in nodes {
            if node.start_position().row < self.scroll {
                continue;
            }

            if node.start_position().row > self.scroll + terminal::size()?.1 as usize - 1 {
                continue;
            }

            let index =
                self.get_rope_index((node.start_position().column, node.start_position().row));

            if index > last_pos {
                queue!(self.stdout, Print(self.rope.slice(last_pos..index)))?;
            }

            let diff = node.end_position().column - node.start_position().column;
            let end = index + diff;

            queue!(
                self.stdout,
                crossterm::style::SetForegroundColor(COLORS[(node.kind_id() % 12) as usize]),
                Print(self.rope.slice(index..end).to_string())
            )?;

            last_pos = end;
        }

        execute!(self.stdout, cursor::RestorePosition, cursor::Show)?;

        self.stdout.flush()?;

        Ok(())
    }

    fn expand_node(node: Node) -> Vec<Node> {
        let mut nodes = Vec::new();

        if node.child_count() == 0 {
            nodes.push(node);
        }

        for n in node.children(&mut node.walk()) {
            let children = Self::expand_node(n);
            for child in children {
                nodes.push(child);
            }
        }

        nodes
    }

    fn get_visible_lines_len(&self) -> std::io::Result<usize> {
        let mut size = 0;
        for i in 0..self.rope.len_lines() - 1 {
            if i >= self.scroll && i < self.scroll + (terminal::size()?.1 as usize) {
                size += 1;
            }
        }

        Ok(size)
    }

    fn get_current_line_len(&self) -> std::io::Result<usize> {
        Ok(self
            .rope
            .get_line(self.get_line_number()?)
            .unwrap()
            .to_string()
            .len()
            - 2)
    }

    fn get_line_number(&self) -> std::io::Result<usize> {
        Ok(cursor::position()?.1 as usize + self.scroll)
    }

    fn attempt_cursor_move(&mut self, movement: CursorMovement) -> std::io::Result<()> {
        match movement {
            CursorMovement::Up => {
                if cursor::position()?.1 == 0 {
                    self.scroll = self.scroll.saturating_sub(1);
                }

                execute!(self.stdout, cursor::MoveUp(1))?;
                self.redraw()?;
            }
            CursorMovement::Down => {
                if self.get_line_number()? < self.rope.lines().len() - 2 {
                    if cursor::position()?.1 == terminal::size()?.1 - 1 {
                        self.scroll += 1;
                    }
                    execute!(self.stdout, cursor::MoveDown(1))?;

                    self.redraw()?;
                }
            }
            CursorMovement::Left => {
                execute!(self.stdout, cursor::MoveLeft(1))?;
            }
            CursorMovement::Right => {
                if cursor::position()?.0 < self.get_current_line_len()? as u16 {
                    execute!(self.stdout, cursor::MoveRight(1))?;
                }
            }
        }

        self.redraw()?;
        Ok(())
    }
}
