# crossword-grid

A small Rust library for working with crossword grids: parsing a grid from
plain text, checking the standard 180-degree rotational symmetry rule, and
numbering across/down entries the way a published crossword would.

This isn't a solver or a clue database. It's the boring, easy-to-get-wrong
groundwork that a grid editor or a puzzle generator needs before it can do
anything interesting: knowing which cells are black, which cells start a
numbered entry, and how long each entry is.

## Usage

```rust
use crossword_grid::{Grid, Direction};

let grid = Grid::from_rows(&[
    "cat#dog",
    "arena..",
    "b......",
    "###.###",
    "......b",
    "..anode",
    "top#pen",
])?;

assert!(grid.has_rotational_symmetry());

for slot in grid.slots() {
    let label = match slot.direction {
        Direction::Across => "Across",
        Direction::Down => "Down",
    };
    println!("{} {} ({} letters)", slot.number, label, slot.len);
}
```

`Grid::from_rows` accepts `#` for a black square, `.` for an empty white
square, and any letter for an already-filled square. `Grid` implements
`Display`, so printing a grid gives back the same text format.

## Numbering rule

A white cell gets a number if it starts an across entry (nothing but a wall
or the edge to its left, and at least one white cell to its right) or a down
entry (nothing but a wall or the edge above it, and at least one white cell
below). Numbers are assigned in reading order, left to right and top to
bottom, matching the convention used in print crosswords.

## Status

Early skeleton. Parsing, symmetry checking, and slot numbering work and are
tested. Not yet handled: rendering a grid to a file format, generating grids
from a word list, or clue management.

## License

MIT, see LICENSE.
