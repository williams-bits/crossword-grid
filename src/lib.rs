//! Crossword grid representation: parsing, symmetry checks, and slot numbering.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Black,
    Empty,
    Filled(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Across,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub number: u32,
    pub row: usize,
    pub col: usize,
    pub len: usize,
    pub direction: Direction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridError {
    Empty,
    RaggedRow { row: usize, expected: usize, found: usize },
    BadChar { row: usize, col: usize, ch: char },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridError::Empty => write!(f, "grid has no rows"),
            GridError::RaggedRow { row, expected, found } => write!(
                f,
                "row {row} has {found} columns, expected {expected}"
            ),
            GridError::BadChar { row, col, ch } => {
                write!(f, "unrecognized character '{ch}' at row {row}, col {col}")
            }
        }
    }
}

impl std::error::Error for GridError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Grid {
            width,
            height,
            cells: vec![Cell::Empty; width * height],
        }
    }

    /// Parse rows of text into a grid. `#` marks a black square, `.` marks an
    /// empty (unfilled) white square, and any other letter is a filled cell.
    pub fn from_rows(rows: &[&str]) -> Result<Self, GridError> {
        let height = rows.len();
        if height == 0 {
            return Err(GridError::Empty);
        }
        let width = rows[0].chars().count();
        let mut cells = Vec::with_capacity(width * height);
        for (row, line) in rows.iter().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            if chars.len() != width {
                return Err(GridError::RaggedRow {
                    row,
                    expected: width,
                    found: chars.len(),
                });
            }
            for (col, ch) in chars.into_iter().enumerate() {
                let cell = match ch {
                    '#' => Cell::Black,
                    '.' => Cell::Empty,
                    c if c.is_alphabetic() => Cell::Filled(c.to_ascii_uppercase()),
                    c => return Err(GridError::BadChar { row, col, ch: c }),
                };
                cells.push(cell);
            }
        }
        Ok(Grid { width, height, cells })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    fn index(&self, row: usize, col: usize) -> usize {
        row * self.width + col
    }

    pub fn cell(&self, row: usize, col: usize) -> Cell {
        self.cells[self.index(row, col)]
    }

    pub fn set(&mut self, row: usize, col: usize, cell: Cell) {
        let idx = self.index(row, col);
        self.cells[idx] = cell;
    }

    fn is_black(&self, row: usize, col: usize) -> bool {
        matches!(self.cell(row, col), Cell::Black)
    }

    /// Standard American crossword rule: the grid reads the same if you
    /// rotate it 180 degrees, so no entry can be shorter than 3 letters
    /// stranded against a wall of black squares.
    pub fn has_rotational_symmetry(&self) -> bool {
        for row in 0..self.height {
            for col in 0..self.width {
                let mirror_row = self.height - 1 - row;
                let mirror_col = self.width - 1 - col;
                if self.is_black(row, col) != self.is_black(mirror_row, mirror_col) {
                    return false;
                }
            }
        }
        true
    }

    fn across_len(&self, row: usize, col: usize) -> usize {
        let mut len = 0;
        let mut c = col;
        while c < self.width && !self.is_black(row, c) {
            len += 1;
            c += 1;
        }
        len
    }

    fn down_len(&self, row: usize, col: usize) -> usize {
        let mut len = 0;
        let mut r = row;
        while r < self.height && !self.is_black(r, col) {
            len += 1;
            r += 1;
        }
        len
    }

    /// Number every across and down entry following the usual crossword
    /// convention: scan left to right, top to bottom, and assign the next
    /// number to any white cell that starts an across and/or down entry of
    /// length 2 or more.
    pub fn slots(&self) -> Vec<Slot> {
        let mut slots = Vec::new();
        let mut number = 0u32;
        for row in 0..self.height {
            for col in 0..self.width {
                if self.is_black(row, col) {
                    continue;
                }
                let starts_across = (col == 0 || self.is_black(row, col - 1))
                    && self.across_len(row, col) > 1;
                let starts_down =
                    (row == 0 || self.is_black(row - 1, col)) && self.down_len(row, col) > 1;

                if !starts_across && !starts_down {
                    continue;
                }
                number += 1;
                if starts_across {
                    slots.push(Slot {
                        number,
                        row,
                        col,
                        len: self.across_len(row, col),
                        direction: Direction::Across,
                    });
                }
                if starts_down {
                    slots.push(Slot {
                        number,
                        row,
                        col,
                        len: self.down_len(row, col),
                        direction: Direction::Down,
                    });
                }
            }
        }
        slots
    }
}

impl fmt::Display for Grid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in 0..self.height {
            for col in 0..self.width {
                let ch = match self.cell(row, col) {
                    Cell::Black => '#',
                    Cell::Empty => '.',
                    Cell::Filled(c) => c,
                };
                write!(f, "{ch}")?;
            }
            if row + 1 < self.height {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Grid {
        Grid::from_rows(&["...#.", "...#.", ".....", ".#...", ".#..."]).unwrap()
    }

    #[test]
    fn parses_rows() {
        let grid = sample();
        assert_eq!(grid.width(), 5);
        assert_eq!(grid.height(), 5);
        assert_eq!(grid.cell(0, 3), Cell::Black);
    }

    #[test]
    fn rejects_ragged_rows() {
        let err = Grid::from_rows(&["...", ".."]).unwrap_err();
        assert_eq!(
            err,
            GridError::RaggedRow { row: 1, expected: 3, found: 2 }
        );
    }

    #[test]
    fn detects_asymmetry() {
        let symmetric = Grid::from_rows(&["#..", "...", "..#"]).unwrap();
        assert!(symmetric.has_rotational_symmetry());

        let lopsided = Grid::from_rows(&["#..", "...", "..."]).unwrap();
        assert!(!lopsided.has_rotational_symmetry());
    }

    #[test]
    fn numbers_slots_in_reading_order() {
        let grid = Grid::from_rows(&["...", ".#.", "..."]).unwrap();
        let slots = grid.slots();
        let first = &slots[0];
        assert_eq!(first.number, 1);
        assert_eq!((first.row, first.col), (0, 0));
        assert!(slots.iter().any(|s| s.direction == Direction::Down && s.number == 1));
    }

    #[test]
    fn skips_single_letter_stubs() {
        // A cell boxed in on three sides so the "entry" would be length 1
        // should not get its own number for that direction.
        let grid = Grid::from_rows(&["##.", "...", "..."]).unwrap();
        let slots = grid.slots();
        assert!(slots.iter().all(|s| s.len > 1));
    }

    #[test]
    fn display_round_trips_through_parsing() {
        let grid = sample();
        let text = grid.to_string();
        let rows: Vec<&str> = text.lines().collect();
        let reparsed = Grid::from_rows(&rows).unwrap();
        assert_eq!(grid, reparsed);
    }
}
