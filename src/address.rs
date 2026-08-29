use std::fmt;

/// A spreadsheet cell address, stored zero-based internally so it can be used
/// directly as a grid index. `CellRef::parse("A1")` gives col 0, row 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellRef {
    pub col: u32,
    pub row: u32,
}

impl CellRef {
    pub fn new(col: u32, row: u32) -> Self {
        CellRef { col, row }
    }

    /// Parses A1-style references such as "A1", "b12", or "AA7".
    /// Returns None for anything that isn't letters followed by a positive
    /// number, so callers can fall back to treating text as a plain value.
    pub fn parse(s: &str) -> Option<CellRef> {
        let s = s.trim();
        let split_at = s.find(|c: char| c.is_ascii_digit())?;
        let (col_part, row_part) = s.split_at(split_at);
        if col_part.is_empty() || row_part.is_empty() {
            return None;
        }
        if !col_part.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        if !row_part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let row: u32 = row_part.parse().ok()?;
        if row == 0 {
            return None;
        }

        let mut col: u32 = 0;
        for c in col_part.chars() {
            let letter_value = (c.to_ascii_uppercase() as u32) - ('A' as u32) + 1;
            col = col * 26 + letter_value;
        }

        Some(CellRef {
            col: col - 1,
            row: row - 1,
        })
    }
}

/// Expands a rectangular range into every cell it covers, in row-major
/// order. Accepts corners in either order, so `A1:C3` and `C3:A1` expand
/// to the same set of cells.
pub fn expand_range(a: CellRef, b: CellRef) -> Vec<CellRef> {
    let (col_start, col_end) = if a.col <= b.col {
        (a.col, b.col)
    } else {
        (b.col, a.col)
    };
    let (row_start, row_end) = if a.row <= b.row {
        (a.row, b.row)
    } else {
        (b.row, a.row)
    };

    let mut cells = Vec::with_capacity(((row_end - row_start + 1) * (col_end - col_start + 1)) as usize);
    for row in row_start..=row_end {
        for col in col_start..=col_end {
            cells.push(CellRef::new(col, row));
        }
    }
    cells
}

impl fmt::Display for CellRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut col = self.col + 1;
        let mut letters = Vec::new();
        while col > 0 {
            let remainder = (col - 1) % 26;
            letters.push((b'A' + remainder as u8) as char);
            col = (col - 1) / 26;
        }
        letters.reverse();
        let col_str: String = letters.into_iter().collect();
        write!(f, "{}{}", col_str, self.row + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_letter_columns() {
        assert_eq!(CellRef::parse("A1"), Some(CellRef::new(0, 0)));
        assert_eq!(CellRef::parse("b2"), Some(CellRef::new(1, 1)));
    }

    #[test]
    fn parses_double_letter_columns() {
        assert_eq!(CellRef::parse("AA1"), Some(CellRef::new(26, 0)));
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(CellRef::parse("1A"), None);
        assert_eq!(CellRef::parse("A0"), None);
        assert_eq!(CellRef::parse("SUM"), None);
    }

    #[test]
    fn round_trips_through_display() {
        for text in ["A1", "Z9", "AA1", "AB12"] {
            let parsed = CellRef::parse(text).unwrap();
            assert_eq!(parsed.to_string(), text);
        }
    }

    #[test]
    fn expands_a_range_in_row_major_order() {
        let cells = expand_range(CellRef::new(0, 0), CellRef::new(1, 1));
        assert_eq!(
            cells,
            vec![
                CellRef::new(0, 0),
                CellRef::new(1, 0),
                CellRef::new(0, 1),
                CellRef::new(1, 1),
            ]
        );
    }

    #[test]
    fn expands_a_range_regardless_of_corner_order() {
        let forward = expand_range(CellRef::new(0, 0), CellRef::new(0, 2));
        let backward = expand_range(CellRef::new(0, 2), CellRef::new(0, 0));
        assert_eq!(forward, backward);
    }

    #[test]
    fn expands_a_single_cell_range() {
        let cells = expand_range(CellRef::new(3, 3), CellRef::new(3, 3));
        assert_eq!(cells, vec![CellRef::new(3, 3)]);
    }
}
