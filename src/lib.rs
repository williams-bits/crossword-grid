//! Crossword grid representation: parsing, symmetry checks, and slot numbering.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Write as _;

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
pub enum PuzError {
    /// .puz stores width and height as single bytes, so grids must be
    /// between 1 and 255 in each dimension.
    UnsupportedDimensions { width: usize, height: usize },
    IncompleteSolution { row: usize, col: usize },
    NonAsciiLetter { row: usize, col: usize, ch: char },
    /// .puz stores the clue count as a 16-bit field.
    TooManyClues { count: usize },
}

impl fmt::Display for PuzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PuzError::UnsupportedDimensions { width, height } => write!(
                f,
                "grid is {width}x{height}, but .puz requires both dimensions between 1 and 255"
            ),
            PuzError::IncompleteSolution { row, col } => write!(
                f,
                ".puz needs a solution letter for every white cell, but ({row}, {col}) is empty"
            ),
            PuzError::NonAsciiLetter { row, col, ch } => write!(
                f,
                ".puz only supports ASCII letters, but ({row}, {col}) has '{ch}'"
            ),
            PuzError::TooManyClues { count } => write!(
                f,
                "grid has {count} clued entries, but .puz can only store up to {}",
                u16::MAX
            ),
        }
    }
}

impl std::error::Error for PuzError {}

/// The cyclic checksum used throughout the .puz format: rotate right one
/// bit, carrying the low bit into the top, then add the next byte.
fn puz_checksum(data: &[u8], seed: u16) -> u16 {
    let mut sum = seed;
    for &b in data {
        sum = if sum & 1 == 1 { (sum >> 1) + 0x8000 } else { sum >> 1 };
        sum = sum.wrapping_add(b as u16);
    }
    sum
}

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

    /// Render the grid as ipuz JSON (see http://www.ipuz.org). ipuz is a
    /// plain JSON format, so this writes it by hand rather than pulling in
    /// a JSON crate.
    pub fn to_ipuz(&self) -> String {
        let numbers: HashMap<(usize, usize), u32> = self
            .slots()
            .into_iter()
            .map(|slot| ((slot.row, slot.col), slot.number))
            .collect();

        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"version\": \"http://ipuz.org/v2\",\n");
        out.push_str("  \"kind\": [\"http://ipuz.org/crossword#1\"],\n");
        let _ = writeln!(
            out,
            "  \"dimensions\": {{\"width\": {}, \"height\": {}}},",
            self.width, self.height
        );

        out.push_str("  \"puzzle\": [\n");
        for row in 0..self.height {
            out.push_str("    [");
            for col in 0..self.width {
                if col > 0 {
                    out.push_str(", ");
                }
                if self.is_black(row, col) {
                    out.push_str("\"#\"");
                } else if let Some(number) = numbers.get(&(row, col)) {
                    let _ = write!(out, "{number}");
                } else {
                    out.push('0');
                }
            }
            out.push(']');
            out.push_str(if row + 1 < self.height { ",\n" } else { "\n" });
        }
        out.push_str("  ],\n");

        out.push_str("  \"solution\": [\n");
        for row in 0..self.height {
            out.push_str("    [");
            for col in 0..self.width {
                if col > 0 {
                    out.push_str(", ");
                }
                match self.cell(row, col) {
                    Cell::Black => out.push_str("\"#\""),
                    Cell::Filled(c) => {
                        let _ = write!(out, "\"{c}\"");
                    }
                    Cell::Empty => out.push_str("null"),
                }
            }
            out.push(']');
            out.push_str(if row + 1 < self.height { ",\n" } else { "\n" });
        }
        out.push_str("  ]\n");
        out.push('}');
        out
    }

    /// Render the grid as an Across Lite .puz file, ready to be solved.
    ///
    /// Every white cell must already hold its solution letter (that letter
    /// becomes the answer key); the player-facing grid written into the
    /// file is left blank, since .puz's "current state" board is meant to
    /// be filled in by whoever solves the puzzle, not by this library.
    /// Title, author, copyright, and clue text are all empty, since the
    /// grid has no metadata or clue storage yet.
    pub fn to_puz(&self) -> Result<Vec<u8>, PuzError> {
        if self.width == 0 || self.height == 0 || self.width > 255 || self.height > 255 {
            return Err(PuzError::UnsupportedDimensions {
                width: self.width,
                height: self.height,
            });
        }

        let mut solution = Vec::with_capacity(self.width * self.height);
        let mut fill = Vec::with_capacity(self.width * self.height);
        for row in 0..self.height {
            for col in 0..self.width {
                match self.cell(row, col) {
                    Cell::Black => {
                        solution.push(b'.');
                        fill.push(b'.');
                    }
                    Cell::Empty => return Err(PuzError::IncompleteSolution { row, col }),
                    Cell::Filled(ch) => {
                        if !ch.is_ascii_alphabetic() {
                            return Err(PuzError::NonAsciiLetter { row, col, ch });
                        }
                        solution.push(ch as u8);
                        fill.push(b'-');
                    }
                }
            }
        }

        let slot_count = self.slots().len();
        if slot_count > u16::MAX as usize {
            return Err(PuzError::TooManyClues { count: slot_count });
        }
        let num_clues = slot_count as u16;

        let mut cib = [0u8; 8];
        cib[0] = self.width as u8;
        cib[1] = self.height as u8;
        cib[2..4].copy_from_slice(&num_clues.to_le_bytes());
        cib[4..6].copy_from_slice(&1u16.to_le_bytes());
        cib[6..8].copy_from_slice(&0u16.to_le_bytes());

        let c_cib = puz_checksum(&cib, 0);
        let c_sol = puz_checksum(&solution, 0);
        let c_grid = puz_checksum(&fill, 0);

        // Title, author, copyright, and notes are all empty; clues are all
        // empty and, being zero-length, don't change the running checksum.
        let mut c_part = puz_checksum(b"\0", 0);
        c_part = puz_checksum(b"\0", c_part);
        c_part = puz_checksum(b"\0", c_part);
        c_part = puz_checksum(b"\0", c_part);

        let mut c_full = c_cib;
        c_full = puz_checksum(&solution, c_full);
        c_full = puz_checksum(&fill, c_full);
        c_full = puz_checksum(b"\0", c_full);
        c_full = puz_checksum(b"\0", c_full);
        c_full = puz_checksum(b"\0", c_full);
        c_full = puz_checksum(b"\0", c_full);

        // The masked checksum bytes are XORed against "ICHEATED", a fixed
        // string every .puz reader checks for as a sanity marker.
        let low_masks = [
            0x49 ^ (c_cib & 0xFF) as u8,
            0x43 ^ (c_sol & 0xFF) as u8,
            0x48 ^ (c_grid & 0xFF) as u8,
            0x45 ^ (c_part & 0xFF) as u8,
        ];
        let high_masks = [
            0x41 ^ ((c_cib >> 8) & 0xFF) as u8,
            0x54 ^ ((c_sol >> 8) & 0xFF) as u8,
            0x45 ^ ((c_grid >> 8) & 0xFF) as u8,
            0x44 ^ ((c_part >> 8) & 0xFF) as u8,
        ];

        let mut out = Vec::new();
        out.extend_from_slice(&c_full.to_le_bytes());
        out.extend_from_slice(b"ACROSS&DOWN\0");
        out.extend_from_slice(&c_cib.to_le_bytes());
        out.extend_from_slice(&low_masks);
        out.extend_from_slice(&high_masks);
        out.extend_from_slice(b"1.3\0");
        out.extend_from_slice(&[0u8; 2]); // reserved
        out.extend_from_slice(&[0u8; 2]); // scrambled checksum (0: unscrambled)
        out.extend_from_slice(&[0u8; 12]); // reserved
        out.push(self.width as u8);
        out.push(self.height as u8);
        out.extend_from_slice(&num_clues.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // puzzle type: normal
        out.extend_from_slice(&0u16.to_le_bytes()); // scrambled tag: unscrambled
        out.extend_from_slice(&solution);
        out.extend_from_slice(&fill);
        out.push(0); // title
        out.push(0); // author
        out.push(0); // copyright
        for _ in 0..num_clues {
            out.push(0);
        }
        out.push(0); // notes

        Ok(out)
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

/// A pool of candidate fill words, grouped by length so the solver can look
/// up matches for a slot without scanning the whole list.
#[derive(Debug, Clone, Default)]
pub struct WordList {
    by_length: HashMap<usize, Vec<String>>,
}

impl WordList {
    /// Build a word list from any source of strings. Entries that aren't
    /// pure ASCII letters are dropped rather than rejected outright, since a
    /// real word list (e.g. a dictionary file) will always have a few lines
    /// with punctuation or numbers mixed in.
    pub fn new<I, S>(words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut by_length: HashMap<usize, Vec<String>> = HashMap::new();
        for word in words {
            let word = word.as_ref();
            if word.is_empty() || !word.chars().all(|c| c.is_ascii_alphabetic()) {
                continue;
            }
            let upper: String = word.chars().map(|c| c.to_ascii_uppercase()).collect();
            by_length.entry(upper.chars().count()).or_default().push(upper);
        }
        WordList { by_length }
    }

    fn candidates(&self, len: usize) -> &[String] {
        self.by_length.get(&len).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FillError {
    /// No combination of words from the list satisfies every slot and every
    /// crossing constraint.
    NoSolution,
}

impl fmt::Display for FillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FillError::NoSolution => write!(f, "no fill satisfies every slot and crossing"),
        }
    }
}

impl std::error::Error for FillError {}

fn slot_cells(slot: &Slot) -> Vec<(usize, usize)> {
    (0..slot.len)
        .map(|i| match slot.direction {
            Direction::Across => (slot.row, slot.col + i),
            Direction::Down => (slot.row + i, slot.col),
        })
        .collect()
}

/// Fill `slots[index..]` by backtracking: try each word of the right length
/// that agrees with whatever letters are already sitting in the slot's
/// cells (either given by the caller or placed by an earlier, crossing
/// slot), place it, recurse, and undo on failure. `used` tracks words
/// already placed so the same entry doesn't appear twice in one grid.
fn backtrack_fill(
    grid: &mut Grid,
    slots: &[Slot],
    index: usize,
    word_list: &WordList,
    used: &mut HashSet<String>,
) -> bool {
    let Some(slot) = slots.get(index) else {
        return true;
    };
    let cells = slot_cells(slot);
    let pattern: Vec<Option<char>> = cells
        .iter()
        .map(|&(row, col)| match grid.cell(row, col) {
            Cell::Filled(ch) => Some(ch),
            Cell::Empty => None,
            Cell::Black => unreachable!("slot cells are never black"),
        })
        .collect();

    for word in word_list.candidates(slot.len) {
        if used.contains(word) {
            continue;
        }
        let letters: Vec<char> = word.chars().collect();
        let matches = pattern
            .iter()
            .zip(&letters)
            .all(|(existing, ch)| existing.map_or(true, |e| e == *ch));
        if !matches {
            continue;
        }

        let mut placed = Vec::new();
        for (&(row, col), &ch) in cells.iter().zip(&letters) {
            if let Cell::Empty = grid.cell(row, col) {
                grid.set(row, col, Cell::Filled(ch));
                placed.push((row, col));
            }
        }
        used.insert(word.clone());

        if backtrack_fill(grid, slots, index + 1, word_list, used) {
            return true;
        }

        used.remove(word);
        for (row, col) in placed {
            grid.set(row, col, Cell::Empty);
        }
    }

    false
}

impl Grid {
    /// Fill every empty white cell using words from `word_list`, respecting
    /// any letters already placed and every across/down crossing. Returns a
    /// new grid on success; the receiver is left untouched either way.
    pub fn fill(&self, word_list: &WordList) -> Result<Grid, FillError> {
        let mut grid = self.clone();
        let slots = grid.slots();
        let mut used = HashSet::new();
        if backtrack_fill(&mut grid, &slots, 0, word_list, &mut used) {
            Ok(grid)
        } else {
            Err(FillError::NoSolution)
        }
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
    fn ipuz_puzzle_grid_marks_blacks_and_numbers() {
        let grid = Grid::from_rows(&["...", ".#.", "..."]).unwrap();
        let json = grid.to_ipuz();
        assert!(json.contains("\"dimensions\": {\"width\": 3, \"height\": 3}"));
        assert!(json.contains("[1, 0, 2]"));
        assert!(json.contains("[0, \"#\", 0]"));
        assert!(json.contains("[3, 0, 0]"));
    }

    #[test]
    fn ipuz_solution_uses_letters_and_null_for_blanks() {
        let grid = Grid::from_rows(&["ab#", "c..", "###"]).unwrap();
        let json = grid.to_ipuz();
        assert!(json.contains("[\"A\", \"B\", \"#\"]"));
        assert!(json.contains("[\"C\", null, null]"));
        assert!(json.contains("[\"#\", \"#\", \"#\"]"));
    }

    #[test]
    fn puz_header_has_magic_string_and_dimensions() {
        let grid = Grid::from_rows(&["cat", "ago", "tot"]).unwrap();
        let bytes = grid.to_puz().unwrap();
        assert_eq!(&bytes[0x02..0x0E], b"ACROSS&DOWN\0");
        assert_eq!(bytes[0x2C], 3); // width
        assert_eq!(bytes[0x2D], 3); // height
    }

    #[test]
    fn puz_boards_hold_solution_and_blank_fill() {
        let grid = Grid::from_rows(&["ca#", "ago", "#ot"]).unwrap();
        let bytes = grid.to_puz().unwrap();
        let board_len = 9;
        let solution = &bytes[0x34..0x34 + board_len];
        let fill = &bytes[0x34 + board_len..0x34 + 2 * board_len];
        assert_eq!(solution, b"CA.AGO.OT");
        assert_eq!(fill, b"--.---.--");
    }

    #[test]
    fn puz_rejects_incomplete_solution() {
        let grid = Grid::from_rows(&["..#", "...", "#.."]).unwrap();
        let err = grid.to_puz().unwrap_err();
        assert_eq!(err, PuzError::IncompleteSolution { row: 0, col: 0 });
    }

    #[test]
    fn puz_clue_count_matches_slot_count() {
        let grid = Grid::from_rows(&["cat", "ago", "tot"]).unwrap();
        let bytes = grid.to_puz().unwrap();
        let num_clues = u16::from_le_bytes([bytes[0x2E], bytes[0x2F]]);
        assert_eq!(num_clues as usize, grid.slots().len());
    }

    #[test]
    fn display_round_trips_through_parsing() {
        let grid = sample();
        let text = grid.to_string();
        let rows: Vec<&str> = text.lines().collect();
        let reparsed = Grid::from_rows(&rows).unwrap();
        assert_eq!(grid, reparsed);
    }

    #[test]
    fn fills_grid_from_word_list() {
        let grid = Grid::from_rows(&["...", ".#.", "..."]).unwrap();
        let words = WordList::new(["cat", "cab", "tin", "bun"]);
        let filled = grid.fill(&words).unwrap();
        for row in 0..filled.height() {
            for col in 0..filled.width() {
                if (row, col) == (1, 1) {
                    assert_eq!(filled.cell(row, col), Cell::Black);
                } else {
                    assert!(matches!(filled.cell(row, col), Cell::Filled(_)));
                }
            }
        }
    }

    #[test]
    fn fill_respects_existing_letters() {
        let grid = Grid::from_rows(&["c..", ".#.", "..."]).unwrap();
        let words = WordList::new(["cat", "cab", "tin", "bun"]);
        let filled = grid.fill(&words).unwrap();
        assert_eq!(filled.cell(0, 0), Cell::Filled('C'));
        assert_eq!(filled.cell(0, 1), Cell::Filled('A'));
        assert_eq!(filled.cell(0, 2), Cell::Filled('T'));
    }

    #[test]
    fn fill_avoids_duplicate_words() {
        // The two across entries don't cross, so without a uniqueness check
        // both would happily fill in as "cat" from the same single-word list.
        let grid = Grid::from_rows(&["...", "###", "..."]).unwrap();
        let words = WordList::new(["cat"]);
        assert_eq!(grid.fill(&words).unwrap_err(), FillError::NoSolution);
    }

    #[test]
    fn fill_fails_without_a_matching_word() {
        let grid = Grid::from_rows(&["...", "...", "..."]).unwrap();
        let words = WordList::new(["ab", "cd"]);
        assert_eq!(grid.fill(&words).unwrap_err(), FillError::NoSolution);
    }

    #[test]
    fn word_list_drops_non_alphabetic_entries() {
        let words = WordList::new(["cat", "c-t", "", "dog2"]);
        assert_eq!(words.candidates(3), ["CAT"]);
        assert!(words.candidates(4).is_empty());
    }
}
