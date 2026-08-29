use crate::address::{expand_range, CellRef};
use crate::formula::tokenize;
use crate::grid::Grid;
use crate::parser::{parse, BinOp, Expr};
use std::collections::HashSet;
use std::fmt;

/// Everything that can go wrong turning a cell into a number: the formula
/// text itself failing to parse, a reference cycle, a function nobody
/// defined, or a cell that just isn't numeric.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    CircularReference(CellRef),
    ParseError(String),
    UnknownFunction(String),
    NotANumber(String),
    RangeOutsideFunction,
    EmptyAggregate(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::CircularReference(cell) => write!(f, "circular reference at {cell}"),
            EvalError::ParseError(msg) => write!(f, "{msg}"),
            EvalError::UnknownFunction(name) => write!(f, "unknown function: {name}"),
            EvalError::NotANumber(text) => write!(f, "not a number: {text}"),
            EvalError::RangeOutsideFunction => {
                write!(f, "a cell range can only appear as a function argument")
            }
            EvalError::EmptyAggregate(name) => {
                write!(f, "{name} needs at least one value")
            }
        }
    }
}

/// Evaluates a cell to a number, resolving any cell references its formula
/// depends on (recursively, since a formula cell can point at another
/// formula cell). Non-formula cells are parsed as plain numbers; a missing
/// or blank cell evaluates to 0, matching how spreadsheets treat empty
/// operands in arithmetic.
pub fn eval_cell(grid: &Grid, cell: CellRef) -> Result<f64, EvalError> {
    let mut visiting = HashSet::new();
    eval_cell_inner(grid, cell, &mut visiting)
}

fn eval_cell_inner(
    grid: &Grid,
    cell: CellRef,
    visiting: &mut HashSet<CellRef>,
) -> Result<f64, EvalError> {
    if !visiting.insert(cell) {
        return Err(EvalError::CircularReference(cell));
    }
    let result = eval_cell_body(grid, cell, visiting);
    visiting.remove(&cell);
    result
}

fn eval_cell_body(
    grid: &Grid,
    cell: CellRef,
    visiting: &mut HashSet<CellRef>,
) -> Result<f64, EvalError> {
    let raw = match grid.get(cell) {
        Some(raw) if !raw.trim().is_empty() => raw,
        _ => return Ok(0.0),
    };
    if let Some(body) = raw.strip_prefix('=') {
        let tokens = tokenize(body).map_err(EvalError::ParseError)?;
        let expr = parse(&tokens).map_err(EvalError::ParseError)?;
        eval_expr(grid, &expr, visiting)
    } else {
        raw.trim()
            .parse::<f64>()
            .map_err(|_| EvalError::NotANumber(raw.to_string()))
    }
}

fn eval_expr(grid: &Grid, expr: &Expr, visiting: &mut HashSet<CellRef>) -> Result<f64, EvalError> {
    match expr {
        Expr::Number(n) => Ok(*n),
        Expr::Cell(cell) => eval_cell_inner(grid, *cell, visiting),
        Expr::Range(_, _) => Err(EvalError::RangeOutsideFunction),
        Expr::Neg(inner) => Ok(-eval_expr(grid, inner, visiting)?),
        Expr::BinOp(left, op, right) => {
            let l = eval_expr(grid, left, visiting)?;
            let r = eval_expr(grid, right, visiting)?;
            Ok(match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => l / r,
            })
        }
        Expr::Call(name, args) => eval_call(grid, name, args, visiting),
    }
}

/// Collects the values a single call argument contributes. A range expands
/// to every non-blank cell in it; a single cell reference contributes
/// nothing if blank rather than a 0, so `AVERAGE` and `COUNT` don't treat
/// missing data as a real zero.
fn collect_values(
    grid: &Grid,
    expr: &Expr,
    visiting: &mut HashSet<CellRef>,
) -> Result<Vec<f64>, EvalError> {
    match expr {
        Expr::Range(start, end) => {
            let mut values = Vec::new();
            for cell in expand_range(*start, *end) {
                if cell_is_blank(grid, cell) {
                    continue;
                }
                values.push(eval_cell_inner(grid, cell, visiting)?);
            }
            Ok(values)
        }
        Expr::Cell(cell) => {
            if cell_is_blank(grid, *cell) {
                Ok(Vec::new())
            } else {
                Ok(vec![eval_cell_inner(grid, *cell, visiting)?])
            }
        }
        other => Ok(vec![eval_expr(grid, other, visiting)?]),
    }
}

fn cell_is_blank(grid: &Grid, cell: CellRef) -> bool {
    match grid.get(cell) {
        Some(raw) => raw.trim().is_empty(),
        None => true,
    }
}

fn eval_call(
    grid: &Grid,
    name: &str,
    args: &[Expr],
    visiting: &mut HashSet<CellRef>,
) -> Result<f64, EvalError> {
    let mut values = Vec::new();
    for arg in args {
        values.extend(collect_values(grid, arg, visiting)?);
    }

    match name.to_ascii_uppercase().as_str() {
        "SUM" => Ok(values.iter().sum()),
        "COUNT" => Ok(values.len() as f64),
        "AVERAGE" => {
            if values.is_empty() {
                return Err(EvalError::EmptyAggregate("AVERAGE".to_string()));
            }
            Ok(values.iter().sum::<f64>() / values.len() as f64)
        }
        "MIN" => values
            .into_iter()
            .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.min(v))))
            .ok_or_else(|| EvalError::EmptyAggregate("MIN".to_string())),
        "MAX" => values
            .into_iter()
            .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.max(v))))
            .ok_or_else(|| EvalError::EmptyAggregate("MAX".to_string())),
        other => Err(EvalError::UnknownFunction(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_a_plain_number_cell() {
        let grid = Grid::from_reader("42".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 0)), Ok(42.0));
    }

    #[test]
    fn treats_missing_cells_as_zero() {
        let grid = Grid::from_reader("1".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(5, 5)), Ok(0.0));
    }

    #[test]
    fn evaluates_arithmetic_across_cell_references() {
        let grid = Grid::from_reader("2,3\n=A1*B1".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 1)), Ok(6.0));
    }

    #[test]
    fn evaluates_formulas_that_reference_other_formulas() {
        let grid = Grid::from_reader("10\n=A1+1\n=A2*2".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 2)), Ok(22.0));
    }

    #[test]
    fn sums_a_range() {
        let grid = Grid::from_reader("1\n2\n3\n=SUM(A1:A3)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 3)), Ok(6.0));
    }

    #[test]
    fn averages_a_range_ignoring_blank_cells() {
        let grid = Grid::from_reader("10\n\n20\n=AVERAGE(A1:A3)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 3)), Ok(15.0));
    }

    #[test]
    fn finds_min_and_max_of_a_range() {
        let grid =
            Grid::from_reader("3\n1\n2\n=MIN(A1:A3),=MAX(A1:A3)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 3)), Ok(1.0));
        assert_eq!(eval_cell(&grid, CellRef::new(1, 3)), Ok(3.0));
    }

    #[test]
    fn counts_only_non_blank_cells_in_a_range() {
        let grid = Grid::from_reader("1\n\n3\n=COUNT(A1:A3)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 3)), Ok(2.0));
    }

    #[test]
    fn averaging_an_all_blank_range_is_an_error() {
        let grid = Grid::from_reader("\n\n\n=AVERAGE(A1:A3)".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 3)),
            Err(EvalError::EmptyAggregate("AVERAGE".to_string()))
        );
    }

    #[test]
    fn detects_a_direct_cycle() {
        // A1 = "=A2" and A2 = "=A1": evaluating either one loops back to it.
        let grid = Grid::from_reader("=A2\n=A1".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 0)),
            Err(EvalError::CircularReference(CellRef::new(0, 0)))
        );
    }

    #[test]
    fn detects_a_self_reference() {
        let grid = Grid::from_reader("=A1+1".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 0)),
            Err(EvalError::CircularReference(CellRef::new(0, 0)))
        );
    }

    #[test]
    fn reports_unknown_functions() {
        let grid = Grid::from_reader("5\n=NOPE(A1)".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 1)),
            Err(EvalError::UnknownFunction("NOPE".to_string()))
        );
    }

    #[test]
    fn rejects_non_numeric_text_cells() {
        let grid = Grid::from_reader("hello".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 0)),
            Err(EvalError::NotANumber("hello".to_string()))
        );
    }

    #[test]
    fn rejects_a_bare_range_outside_a_function_call() {
        let grid = Grid::from_reader("1\n2\n=A1:A2".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 2)),
            Err(EvalError::RangeOutsideFunction)
        );
    }
}
