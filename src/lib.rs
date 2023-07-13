use crossterm::QueueableCommand;
use rand::Rng;
use std::{
    cmp::Ordering,
    io::{ErrorKind, Write},
    iter::zip,
};

const SIZE_USIZE: usize = 4;
const SIZE: u16 = 4;
const MAX_DIGIT_WIDTH: u16 = 5;

/// A number to go into a single square on the 2048 board.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Square(u8);

/// A whole board of 2048
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Board {
    rows: [[Option<Square>; SIZE_USIZE]; SIZE_USIZE],
}

/// A user move that can be applied to a board.
#[derive(Copy, Clone, Debug)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
}

impl Square {
    pub fn inc(self) -> Self {
        Square(self.0 + 1)
    }

    pub fn color(self) -> (crossterm::style::Color, bool) {
        let (r, g, b, is_dark) = match self.0 {
            0 => (238, 228, 218, true),   // 2
            1 => (237, 224, 200, true),   // 4
            2 => (242, 177, 121, false),  // 8
            3 => (245, 149, 99, false),   // 16
            4 => (246, 124, 95, false),   // 32
            5 => (246, 94, 59, false),    // 64
            6 => (237, 207, 114, true),   // 128
            7 => (237, 204, 97, false),   // 256
            8 => (237, 200, 80, false),   // 512
            9 => (237, 197, 63, false),   // 1024
            10 => (237, 194, 68, false),  // 2048
            11 => (181, 134, 180, false), // 4096
            12 => (168, 97, 171, false),  // 8192
            13 => (160, 72, 163, false),  // 16 384
            14 => (128, 0, 128, false),   // 32 768
            15 => (96, 0, 70, false),     // 65 536
            _ => unreachable!(),          // not possible to create these in gameplay
        };
        let background = crossterm::style::Color::Rgb { r, g, b };
        (background, is_dark)
    }
}

impl Board {
    /// Create a new default board.
    pub fn new() -> Self {
        Board {
            rows: [[None; 4]; 4],
        }
    }

    fn coord_iter(direction: Move, offset: usize) -> impl Iterator<Item = (usize, usize)> {
        let steps = match direction {
            Move::Up | Move::Left => [0, 1, 2, 3].into_iter(),
            Move::Down | Move::Right => [3, 2, 1, 0].into_iter(),
        };
        steps.map(move |i| match direction {
            Move::Up | Move::Down => (offset, i),
            Move::Left | Move::Right => (i, offset),
        })
    }

    fn collapse(input: impl Iterator<Item = Option<Square>>) -> impl Iterator<Item = Square> {
        let only_cells = input.filter_map(|x| x);
        struct Collapser<I> {
            inner: I,
            last_seen: Option<Square>,
        }

        impl<I: Iterator<Item = Square>> Iterator for Collapser<I> {
            type Item = Square;

            fn next(&mut self) -> Option<Self::Item> {
                if let Some(last) = self.last_seen.take() {
                    match self.inner.next() {
                        Some(item) if item == last => Some(item.inc()),
                        Some(other) => {
                            self.last_seen = Some(other);
                            Some(last)
                        }
                        None => Some(last),
                    }
                } else {
                    match self.inner.next() {
                        Some(next) => {
                            self.last_seen = Some(next);
                            self.next()
                        }
                        None => return None,
                    }
                }
            }
        }

        Collapser {
            inner: only_cells,
            last_seen: None,
        }
    }

    pub fn apply_move(self, direction: Move) -> Self {
        let mut output = Board {
            rows: [[None; 4]; 4],
        };
        for offset in 0..4 {
            let existing = Self::coord_iter(direction, offset).map(|(x, y)| self.rows[y][x]);
            let collapsed = Self::collapse(existing);
            let mut write_coords = Self::coord_iter(direction, offset);
            for cell in collapsed {
                let (x, y) = write_coords.next().expect("Too many cells post-collapse");
                output.rows[y][x] = Some(cell);
            }
        }

        output
    }

    /// Attempts to add a new square to the board.
    pub fn add_square(&mut self, rng: &mut impl Rng) {
        let coords = (0..4).flat_map(|y| (0..4).map(move |x| (x, y)));
        let free_spaces = coords
            .filter(|&(x, y)| self.rows[y][x].is_none())
            .collect::<Vec<_>>();
        if free_spaces.is_empty() {
            return;
        }

        let space_choice = rng.gen_range(0..free_spaces.len());
        let new_cell = if rng.gen() { Square(1) } else { Square(0) };
        let (x, y) = free_spaces[space_choice];
        self.rows[y][x] = Some(new_cell);
    }
}

/// A wrapper around crossterm + stdout that puts boards on the screen
pub struct Renderer<Output: Write> {
    output: Output,
    size: (u16, u16),
    cursor_row: u16,
    /// What's currently on the screen, if anything
    old_board: Option<Board>,
}

impl<Output: Write> Renderer<Output> {
    /// Create a renderer from a stdout handle.
    pub fn new(mut output: Output) -> crossterm::Result<Self> {
        // Before we enter raw mode, push the screen down 4 rows so that we have space to play our
        // game at the bottom of the screen.
        for _ in 0..=SIZE {
            writeln!(output)?;
        }
        crossterm::terminal::enable_raw_mode()?;
        let mut renderer = Renderer {
            output,
            size: (0, 0),
            cursor_row: SIZE + 1,
            old_board: None,
        };

        renderer.output.queue(crossterm::cursor::Hide)?;

        let size = crossterm::terminal::size()?;
        renderer.resize(size)?;

        Ok(renderer)
    }

    /// Handle a resize event - note that to finish handling the resize event you will also need to
    /// redraw the board.
    pub fn resize(&mut self, new_size: (u16, u16)) -> crossterm::Result<()> {
        self.size = new_size;
        self.old_board = None;

        Ok(())
    }

    fn draw_cell(&mut self, cell: Square) -> crossterm::Result<()> {
        let (bg, is_dark) = cell.color();
        self.output
            .queue(crossterm::style::SetBackgroundColor(bg))?;
        if is_dark {
            self.output.queue(crossterm::style::SetForegroundColor(
                crossterm::style::Color::Black,
            ))?;
        } else {
            self.output
                .queue(crossterm::style::SetForegroundColor(
                    crossterm::style::Color::White,
                ))?
                .queue(crossterm::style::SetAttribute(
                    crossterm::style::Attribute::Bold,
                ))?;
        }
        write!(self.output, "{:5}", 2 << cell.0)?;
        self.output.queue(crossterm::style::ResetColor)?;
        Ok(())
    }

    /// Mark the game as over
    pub fn lose(&mut self) -> crossterm::Result<()> {
        let string = "Game over";
        self.output
            .queue(crossterm::cursor::MoveDown(1 + SIZE - self.cursor_row))?
            .queue(crossterm::cursor::MoveToColumn(
                (SIZE * MAX_DIGIT_WIDTH - string.len() as u16) / 2,
            ))?;
        write!(self.output, "{}", string)?;
        self.output.flush()
    }

    /// Draw the current board on the screen.
    pub fn draw_board(&mut self, board: &Board) -> crossterm::Result<()> {
        if self.size.0 < SIZE * MAX_DIGIT_WIDTH || self.size.1 < SIZE {
            return Err(crossterm::ErrorKind::new(
                ErrorKind::Other,
                "Window too small to draw the game board",
            ));
        }

        if let Some(old_board) = self.old_board {
            for (row_id, rows) in zip(old_board.rows, board.rows).enumerate() {
                for (col_id, (old, new)) in zip(rows.0, rows.1).enumerate() {
                    if old == new {
                        continue;
                    }

                    let screen_row = row_id as u16;
                    let screen_col = MAX_DIGIT_WIDTH * (col_id as u16);

                    match screen_row.cmp(&self.cursor_row) {
                        Ordering::Less => self
                            .output
                            .queue(crossterm::cursor::MoveUp(self.cursor_row - screen_row))?,
                        Ordering::Equal => &mut self.output,
                        Ordering::Greater => self
                            .output
                            .queue(crossterm::cursor::MoveDown(screen_row - self.cursor_row))?,
                    };
                    self.cursor_row = screen_row;

                    self.output
                        .queue(crossterm::cursor::MoveToColumn(screen_col))?;
                    if let Some(cell) = new {
                        self.draw_cell(cell)?;
                    } else {
                        write!(self.output, "     ")?; // Deliberately write spaces instead of move
                    }
                }
            }
        } else {
            if self.cursor_row != 0 {
                self.output
                    .queue(crossterm::cursor::MoveUp(self.cursor_row))?;
            }
            for row in &board.rows {
                self.output.queue(crossterm::cursor::MoveDown(1))?;
                self.output.queue(crossterm::cursor::MoveToColumn(0))?;

                let mut first_cell = true;
                for cell in row {
                    if first_cell {
                        first_cell = false;
                    } else {
                    }

                    if let Some(cell) = cell {
                        self.draw_cell(*cell)?;
                    } else {
                        self.output
                            .queue(crossterm::cursor::MoveRight(MAX_DIGIT_WIDTH))?;
                    }
                }
            }
            self.cursor_row = SIZE - 1;
        }

        self.old_board = Some(*board);
        self.output.flush()
    }
}

impl<Output: Write> Drop for Renderer<Output> {
    fn drop(&mut self) {
        self.output.flush().ok();
        crossterm::terminal::disable_raw_mode().ok();
        self.output.queue(crossterm::cursor::Show).ok();
        writeln!(self.output).ok();
        self.output.flush().ok();
    }
}
