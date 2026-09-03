use crate::address::CellRef;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// A sparse grid of cell values, keyed by address. Values are kept as raw
/// text: a cell holding "=SUM(A1:A3)" is stored as that literal string, not
/// evaluated, so the grid works the same whether or not the source has
/// formulas in it.
#[derive(Debug, Default)]
pub struct Grid {
    cells: HashMap<CellRef, String>,
}

impl Grid {
    pub fn new() -> Self {
        Grid {
            cells: HashMap::new(),
        }
    }

    /// Reads CSV rows from any source. This is the one place that knows how
    /// to turn bytes into cells, so files, stdin, and in-memory buffers all
    /// go through the same path.
    ///
    /// Fields may be quoted with `"`, which lets them contain commas or
    /// newlines that would otherwise be read as delimiters; a literal quote
    /// inside a quoted field is written as `""`. Unquoted fields are
    /// trimmed of surrounding whitespace; quoted fields are taken verbatim.
    pub fn from_reader<R: Read>(mut reader: R) -> io::Result<Grid> {
        let mut content = String::new();
        reader.read_to_string(&mut content)?;

        let mut grid = Grid::new();
        for (row_idx, row) in parse_csv(&content).into_iter().enumerate() {
            for (col_idx, value) in row.into_iter().enumerate() {
                if value.is_empty() {
                    continue;
                }
                grid.set(CellRef::new(col_idx as u32, row_idx as u32), value);
            }
        }
        Ok(grid)
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> io::Result<Grid> {
        let file = File::open(path)?;
        Grid::from_reader(file)
    }

    /// Convenience wrapper for the `some-tool | your-binary` pattern.
    pub fn from_stdin() -> io::Result<Grid> {
        Grid::from_reader(io::stdin())
    }

    pub fn get(&self, cell: CellRef) -> Option<&str> {
        self.cells.get(&cell).map(|s| s.as_str())
    }

    pub fn set(&mut self, cell: CellRef, value: impl Into<String>) {
        self.cells.insert(cell, value.into());
    }

    pub fn is_formula(&self, cell: CellRef) -> bool {
        self.get(cell).map_or(false, |v| v.starts_with('='))
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// Splits CSV text into rows of fields, honoring `"`-quoted fields (which
/// may embed commas, newlines, and `""`-escaped quotes). This is a small
/// hand-rolled state machine rather than a line-then-split approach because
/// a quoted field can legitimately contain the row separator.
fn parse_csv(input: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    macro_rules! end_field {
        () => {
            row.push(if quoted {
                std::mem::take(&mut field)
            } else {
                std::mem::take(&mut field).trim().to_string()
            });
            quoted = false;
        };
    }
    macro_rules! end_row {
        () => {
            end_field!();
            rows.push(std::mem::take(&mut row));
        };
    }

    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => in_quotes = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !quoted => {
                in_quotes = true;
                quoted = true;
            }
            ',' => {
                end_field!();
            }
            '\n' => {
                end_row!();
            }
            '\r' => {
                if chars.peek() != Some(&'\n') {
                    end_row!();
                }
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || quoted || !row.is_empty() {
        end_row!();
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_values_from_a_buffer() {
        let data = "1,2,3\n=SUM(A1:C1),,\n";
        let grid = Grid::from_reader(data.as_bytes()).unwrap();
        assert_eq!(grid.get(CellRef::new(0, 0)), Some("1"));
        assert_eq!(grid.get(CellRef::new(2, 0)), Some("3"));
        assert_eq!(grid.get(CellRef::new(0, 1)), Some("=SUM(A1:C1)"));
    }

    #[test]
    fn skips_blank_cells() {
        let grid = Grid::from_reader("a,,c".as_bytes()).unwrap();
        assert_eq!(grid.len(), 2);
        assert_eq!(grid.get(CellRef::new(1, 0)), None);
    }

    #[test]
    fn recognizes_formula_cells() {
        let grid = Grid::from_reader("=A1+1,42".as_bytes()).unwrap();
        assert!(grid.is_formula(CellRef::new(0, 0)));
        assert!(!grid.is_formula(CellRef::new(1, 0)));
    }

    #[test]
    fn quoted_field_can_contain_a_comma() {
        let grid = Grid::from_reader("\"Smith, John\",42".as_bytes()).unwrap();
        assert_eq!(grid.get(CellRef::new(0, 0)), Some("Smith, John"));
        assert_eq!(grid.get(CellRef::new(1, 0)), Some("42"));
    }

    #[test]
    fn doubled_quote_is_a_literal_quote() {
        let grid = Grid::from_reader("\"she said \"\"hi\"\"\"".as_bytes()).unwrap();
        assert_eq!(grid.get(CellRef::new(0, 0)), Some("she said \"hi\""));
    }

    #[test]
    fn quoted_field_can_contain_a_newline() {
        let grid = Grid::from_reader("\"line1\nline2\",b\nc,d".as_bytes()).unwrap();
        assert_eq!(grid.get(CellRef::new(0, 0)), Some("line1\nline2"));
        assert_eq!(grid.get(CellRef::new(1, 0)), Some("b"));
        assert_eq!(grid.get(CellRef::new(0, 1)), Some("c"));
        assert_eq!(grid.get(CellRef::new(1, 1)), Some("d"));
    }

    #[test]
    fn quoted_field_is_not_trimmed() {
        let grid = Grid::from_reader("\"  padded  \",unquoted  ".as_bytes()).unwrap();
        assert_eq!(grid.get(CellRef::new(0, 0)), Some("  padded  "));
        assert_eq!(grid.get(CellRef::new(1, 0)), Some("unquoted"));
    }

    #[test]
    fn handles_crlf_line_endings() {
        let grid = Grid::from_reader("1,2\r\n3,4\r\n".as_bytes()).unwrap();
        assert_eq!(grid.get(CellRef::new(1, 0)), Some("2"));
        assert_eq!(grid.get(CellRef::new(0, 1)), Some("3"));
        assert_eq!(grid.len(), 4);
    }
}
