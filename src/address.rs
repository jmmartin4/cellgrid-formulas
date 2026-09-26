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

    /// Parses an "A1:C3"-style range into its two corners. This is the part
    /// `SUM`/`AVERAGE` never had to expose on its own: something that wants
    /// to walk a range without going through the formula tokenizer can split
    /// on the same `:` a formula body uses and get both `CellRef`s directly.
    pub fn parse_range(s: &str) -> Option<(CellRef, CellRef)> {
        let (start, end) = s.trim().split_once(':')?;
        Some((CellRef::parse(start)?, CellRef::parse(end)?))
    }
}

/// A lazy, row-major iterator over every cell in a rectangular range.
/// Accepts corners in either order, so `A1:C3` and `C3:A1` produce the same
/// cells. Being an iterator rather than a `Vec` matters once a range gets
/// used somewhere other than an aggregate that was going to visit every cell
/// anyway - something that only wants the first few cells, or that wants to
/// stop early, doesn't pay to build cells it never looks at.
#[derive(Debug, Clone)]
pub struct RangeIter {
    col_start: u32,
    col_end: u32,
    row_end: u32,
    next: Option<(u32, u32)>,
}

impl RangeIter {
    pub fn new(a: CellRef, b: CellRef) -> Self {
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
        RangeIter {
            col_start,
            col_end,
            row_end,
            next: Some((col_start, row_start)),
        }
    }
}

impl Iterator for RangeIter {
    type Item = CellRef;

    fn next(&mut self) -> Option<CellRef> {
        let (col, row) = self.next?;
        self.next = if col < self.col_end {
            Some((col + 1, row))
        } else if row < self.row_end {
            Some((self.col_start, row + 1))
        } else {
            None
        };
        Some(CellRef::new(col, row))
    }
}

/// Expands a rectangular range into every cell it covers, in row-major
/// order. A thin `Vec`-collecting wrapper around [`RangeIter`] for callers
/// that want the whole set at once, such as the aggregate functions that
/// were always going to visit every cell in the range regardless.
pub fn expand_range(a: CellRef, b: CellRef) -> Vec<CellRef> {
    RangeIter::new(a, b).collect()
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

    #[test]
    fn range_iter_matches_expand_range() {
        let a = CellRef::new(0, 0);
        let b = CellRef::new(2, 2);
        let via_iter: Vec<CellRef> = RangeIter::new(a, b).collect();
        assert_eq!(via_iter, expand_range(a, b));
    }

    #[test]
    fn range_iter_can_be_stopped_early_without_visiting_the_rest() {
        let mut iter = RangeIter::new(CellRef::new(0, 0), CellRef::new(1, 1));
        assert_eq!(iter.next(), Some(CellRef::new(0, 0)));
        assert_eq!(iter.next(), Some(CellRef::new(1, 0)));
        drop(iter);
    }

    #[test]
    fn parses_a_range_string_into_its_corners() {
        assert_eq!(
            CellRef::parse_range("A1:C3"),
            Some((CellRef::new(0, 0), CellRef::new(2, 2)))
        );
    }

    #[test]
    fn parse_range_rejects_input_without_a_colon() {
        assert_eq!(CellRef::parse_range("A1"), None);
    }

    #[test]
    fn parse_range_rejects_a_malformed_corner() {
        assert_eq!(CellRef::parse_range("A1:???"), None);
    }
}
