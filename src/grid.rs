use crate::address::CellRef;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
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

    /// Reads comma-separated rows from any source. This is the one place
    /// that knows how to turn bytes into cells, so files, stdin, and
    /// in-memory buffers all go through the same path.
    pub fn from_reader<R: Read>(reader: R) -> io::Result<Grid> {
        let mut grid = Grid::new();
        let buffered = BufReader::new(reader);
        for (row_idx, line) in buffered.lines().enumerate() {
            let line = line?;
            for (col_idx, raw) in line.split(',').enumerate() {
                let value = raw.trim();
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
}
