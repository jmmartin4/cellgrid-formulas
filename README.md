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

// walk a range without going through a formula at all
use cellgrid_formulas::RangeIter;
let (start, end) = CellRef::parse_range("A1:C3").unwrap();
for cell in RangeIter::new(start, end) {
    println!("{cell}");
}
```

Formula text is parsed into an expression tree with the usual operator
precedence, and `eval_cell` walks that tree against a `Grid`, resolving
any cell references it depends on along the way, and returns a `Value` -
either a number or text:

```rust
use cellgrid_formulas::{eval_cell, CellRef, Grid, Value};

let grid = Grid::from_reader("1,2,3\n=SUM(A1:C1)+10".as_bytes())?;
let total = eval_cell(&grid, CellRef::new(0, 1))?;
assert_eq!(total, Value::Number(16.0));

let grid = Grid::from_reader("Jane,Doe\n=A1&\" \"&B1".as_bytes())?;
let name = eval_cell(&grid, CellRef::new(0, 1))?;
assert_eq!(name, Value::Text("Jane Doe".to_string()));
```

A cell that doesn't parse as a number is text rather than an evaluation
error, so a formula can mix arithmetic and labels the way a real spreadsheet
does. Arithmetic operators (`+ - * /`) require numeric operands and fail
with `EvalError::NotANumber` on text; `&` concatenates either kind by
turning both sides into text first.

A blank or missing cell evaluates to 0 in arithmetic but is skipped by
`AVERAGE`, `MIN`, `MAX`, and `COUNT` so missing data doesn't masquerade as a
real zero. A cell whose formula (directly or transitively) refers back to
itself comes back as `EvalError::CircularReference` instead of looping.

`IF(condition, then, else)` only evaluates the branch it takes, and treats
any nonzero condition as true. The comparison operators (`= <> < > <= >=`)
produce `1` or `0`, so `IF(A1>10, "over", "ok")` works the same way a cell
holding a 0/1 flag already did. Numbers compare numerically and text
compares lexicographically; a number is always considered less than any
text, so a comparison always has an answer even when the two sides turn out
to hold different kinds of value.

## Status

- [x] A1-style cell address parsing, both directions (`CellRef::parse`, `Display`)
- [x] Loading a grid from any `Read` (file, stdin, buffer)
- [x] Formula tokenizer (numbers, cell refs, ranges, arithmetic operators, identifiers)
- [x] Operator precedence and an actual expression parser
- [x] Evaluating formulas against a grid, with `SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNT`, `IF`
- [x] Detecting circular references between cells
- [x] Grid loading that handles quoted CSV fields
- [x] Integration tests covering Grid + parser + eval end to end
- [x] `Value` enum (number/text) and `&` string concatenation
- [x] Comparison operators (`= <> < > <= >=`) for use in `IF` conditions
- [x] `RangeIter` and `CellRef::parse_range` for `A1:C3`-style expansion outside a formula

## License

MIT, see [LICENSE](LICENSE).
