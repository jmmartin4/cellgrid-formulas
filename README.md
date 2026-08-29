# cellgrid-formulas

A CSV export from a spreadsheet doesn't stop being a spreadsheet - cells still
hold text like `=SUM(A1:A3)`, and something reading that file needs to know
that `A1` means column 0, row 0, not just an opaque string. This crate is the
part of that problem that has nothing to do with any particular tool: parsing
A1-style cell addresses, loading a grid of raw cell text, and breaking
formula strings into tokens.

It's a library, not a CLI. The reason it exists is a detail that's easy to
get wrong by accident: a grid loader that only works on `std::fs::File` ends
up unusable in a pipeline (`cat export.csv | your-tool`), so `Grid` is built
on `Read` from the start and files, stdin, and in-memory buffers all go
through the same code path.

## Usage

```rust
use cellgrid_formulas::{CellRef, Grid};

// from a file on disk
let grid = Grid::from_path("export.csv")?;

// from a pipe, e.g. `cat export.csv | your-tool`
let grid = Grid::from_stdin()?;

// from any other Read, useful in tests
let grid = Grid::from_reader("1,2,3\n=SUM(A1:C1),,".as_bytes())?;

let b1 = CellRef::parse("B1").unwrap();
if let Some(value) = grid.get(b1) {
    println!("{value}");
}
```

Formula text is parsed into an expression tree with the usual operator
precedence, and `eval_cell` walks that tree against a `Grid`, resolving
any cell references it depends on along the way:

```rust
use cellgrid_formulas::{eval_cell, CellRef, Grid};

let grid = Grid::from_reader("1,2,3\n=SUM(A1:C1)+10".as_bytes())?;
let total = eval_cell(&grid, CellRef::new(0, 1))?;
assert_eq!(total, 16.0);
```

A blank or missing cell evaluates to 0 in arithmetic but is skipped by
`AVERAGE`, `MIN`, `MAX`, and `COUNT` so missing data doesn't masquerade as a
real zero. A cell whose formula (directly or transitively) refers back to
itself comes back as `EvalError::CircularReference` instead of looping.

## Status

- [x] A1-style cell address parsing, both directions (`CellRef::parse`, `Display`)
- [x] Loading a grid from any `Read` (file, stdin, buffer)
- [x] Formula tokenizer (numbers, cell refs, ranges, arithmetic operators, identifiers)
- [x] Operator precedence and an actual expression parser
- [x] Evaluating formulas against a grid, with `SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNT`
- [x] Detecting circular references between cells
- [ ] Grid loading that handles quoted CSV fields

## License

MIT, see [LICENSE](LICENSE).
