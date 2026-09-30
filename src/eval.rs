use crate::address::{CellRef, RangeIter};
use crate::formula::tokenize;
use crate::grid::Grid;
use crate::parser::{parse, BinOp, Expr};
use crate::value::Value;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;

/// Everything that can go wrong evaluating a cell: the formula text itself
/// failing to parse, a reference cycle, a function nobody defined, or text
/// showing up somewhere that requires a number.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    CircularReference(CellRef),
    ParseError(String),
    UnknownFunction(String),
    NotANumber(String),
    RangeOutsideFunction,
    EmptyAggregate(String),
    WrongArgumentCount(String, usize, usize),
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
            EvalError::WrongArgumentCount(name, expected, found) => {
                write!(f, "{name} takes {expected} arguments, found {found}")
            }
        }
    }
}

/// Evaluates a cell, resolving any cell references its formula depends on
/// (recursively, since a formula cell can point at another formula cell).
/// A non-formula cell is a number if it parses as one, and text otherwise;
/// a missing or blank cell evaluates to 0, matching how spreadsheets treat
/// empty operands in arithmetic.
pub fn eval_cell(grid: &Grid, cell: CellRef) -> Result<Value, EvalError> {
    let mut visiting = HashSet::new();
    eval_cell_inner(grid, cell, &mut visiting)
}

fn eval_cell_inner(
    grid: &Grid,
    cell: CellRef,
    visiting: &mut HashSet<CellRef>,
) -> Result<Value, EvalError> {
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
) -> Result<Value, EvalError> {
    let raw = match grid.get(cell) {
        Some(raw) if !raw.trim().is_empty() => raw,
        _ => return Ok(Value::Number(0.0)),
    };
    if let Some(body) = raw.strip_prefix('=') {
        let tokens = tokenize(body).map_err(EvalError::ParseError)?;
        let expr = parse(&tokens).map_err(EvalError::ParseError)?;
        eval_expr(grid, &expr, visiting)
    } else {
        let trimmed = raw.trim();
        match trimmed.parse::<f64>() {
            Ok(n) => Ok(Value::Number(n)),
            Err(_) => Ok(Value::Text(trimmed.to_string())),
        }
    }
}

fn eval_expr(
    grid: &Grid,
    expr: &Expr,
    visiting: &mut HashSet<CellRef>,
) -> Result<Value, EvalError> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::Text(s) => Ok(Value::Text(s.clone())),
        Expr::Cell(cell) => eval_cell_inner(grid, *cell, visiting),
        Expr::Range(_, _) => Err(EvalError::RangeOutsideFunction),
        Expr::Neg(inner) => {
            let n = eval_expr(grid, inner, visiting)?.as_number()?;
            Ok(Value::Number(-n))
        }
        Expr::BinOp(left, op, right) => {
            let l = eval_expr(grid, left, visiting)?;
            let r = eval_expr(grid, right, visiting)?;
            match op {
                BinOp::Concat => Ok(Value::Text(format!("{l}{r}"))),
                BinOp::Add => Ok(Value::Number(l.as_number()? + r.as_number()?)),
                BinOp::Sub => Ok(Value::Number(l.as_number()? - r.as_number()?)),
                BinOp::Mul => Ok(Value::Number(l.as_number()? * r.as_number()?)),
                BinOp::Div => Ok(Value::Number(l.as_number()? / r.as_number()?)),
                BinOp::Eq => Ok(Value::Bool(l.compare(&r) == Ordering::Equal)),
                BinOp::Ne => Ok(Value::Bool(l.compare(&r) != Ordering::Equal)),
                BinOp::Lt => Ok(Value::Bool(l.compare(&r) == Ordering::Less)),
                BinOp::Gt => Ok(Value::Bool(l.compare(&r) == Ordering::Greater)),
                BinOp::Le => Ok(Value::Bool(l.compare(&r) != Ordering::Greater)),
                BinOp::Ge => Ok(Value::Bool(l.compare(&r) != Ordering::Less)),
            }
        }
        Expr::Call(name, args) => eval_call(grid, name, args, visiting),
    }
}

/// Collects the numbers a single call argument contributes. A range expands
/// to every non-blank cell in it; a single cell reference contributes
/// nothing if blank rather than a 0, so `AVERAGE` and `COUNT` don't treat
/// missing data as a real zero. A cell or expression that evaluates to text
/// is a `NotANumber` error - the aggregate functions have no concept of a
/// non-numeric input.
fn collect_values(
    grid: &Grid,
    expr: &Expr,
    visiting: &mut HashSet<CellRef>,
) -> Result<Vec<f64>, EvalError> {
    match expr {
        Expr::Range(start, end) => {
            let mut values = Vec::new();
            for cell in RangeIter::new(*start, *end) {
                if cell_is_blank(grid, cell) {
                    continue;
                }
                values.push(eval_cell_inner(grid, cell, visiting)?.as_number()?);
            }
            Ok(values)
        }
        Expr::Cell(cell) => {
            if cell_is_blank(grid, *cell) {
                Ok(Vec::new())
            } else {
                Ok(vec![eval_cell_inner(grid, *cell, visiting)?.as_number()?])
            }
        }
        other => Ok(vec![eval_expr(grid, other, visiting)?.as_number()?]),
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
) -> Result<Value, EvalError> {
    let upper = name.to_ascii_uppercase();
    // IF only evaluates the branch it takes, unlike the aggregate functions
    // below - a bad reference or division by zero in the untaken branch
    // shouldn't fail the whole formula.
    if upper == "IF" {
        return eval_if(grid, args, visiting);
    }

    let mut values = Vec::new();
    for arg in args {
        values.extend(collect_values(grid, arg, visiting)?);
    }

    let result = match upper.as_str() {
        "SUM" => values.iter().sum(),
        "COUNT" => values.len() as f64,
        "AVERAGE" => {
            if values.is_empty() {
                return Err(EvalError::EmptyAggregate("AVERAGE".to_string()));
            }
            values.iter().sum::<f64>() / values.len() as f64
        }
        "MIN" => values
            .into_iter()
            .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.min(v))))
            .ok_or_else(|| EvalError::EmptyAggregate("MIN".to_string()))?,
        "MAX" => values
            .into_iter()
            .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.max(v))))
            .ok_or_else(|| EvalError::EmptyAggregate("MAX".to_string()))?,
        other => return Err(EvalError::UnknownFunction(other.to_string())),
    };
    Ok(Value::Number(result))
}

/// `IF(condition, then, else)`. The condition is a boolean such as `A1>10`,
/// or a number, which is true when nonzero, so a cell holding a plain 0/1
/// flag still works. The taken branch is returned as whatever
/// `Value` it evaluates to, so `IF` can pick between two numbers, two pieces
/// of text, or one of each.
fn eval_if(
    grid: &Grid,
    args: &[Expr],
    visiting: &mut HashSet<CellRef>,
) -> Result<Value, EvalError> {
    if args.len() != 3 {
        return Err(EvalError::WrongArgumentCount(
            "IF".to_string(),
            3,
            args.len(),
        ));
    }
    let condition = eval_expr(grid, &args[0], visiting)?.as_bool()?;
    if condition {
        eval_expr(grid, &args[1], visiting)
    } else {
        eval_expr(grid, &args[2], visiting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_a_plain_number_cell() {
        let grid = Grid::from_reader("42".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 0)), Ok(Value::Number(42.0)));
    }

    #[test]
    fn treats_missing_cells_as_zero() {
        let grid = Grid::from_reader("1".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(5, 5)),
            Ok(Value::Number(0.0))
        );
    }

    #[test]
    fn evaluates_arithmetic_across_cell_references() {
        let grid = Grid::from_reader("2,3\n=A1*B1".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 1)), Ok(Value::Number(6.0)));
    }

    #[test]
    fn evaluates_formulas_that_reference_other_formulas() {
        let grid = Grid::from_reader("10\n=A1+1\n=A2*2".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 2)),
            Ok(Value::Number(22.0))
        );
    }

    #[test]
    fn sums_a_range() {
        let grid = Grid::from_reader("1\n2\n3\n=SUM(A1:A3)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 3)), Ok(Value::Number(6.0)));
    }

    #[test]
    fn averages_a_range_ignoring_blank_cells() {
        let grid = Grid::from_reader("10\n\n20\n=AVERAGE(A1:A3)".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 3)),
            Ok(Value::Number(15.0))
        );
    }

    #[test]
    fn finds_min_and_max_of_a_range() {
        let grid =
            Grid::from_reader("3\n1\n2\n=MIN(A1:A3),=MAX(A1:A3)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 3)), Ok(Value::Number(1.0)));
        assert_eq!(eval_cell(&grid, CellRef::new(1, 3)), Ok(Value::Number(3.0)));
    }

    #[test]
    fn counts_only_non_blank_cells_in_a_range() {
        let grid = Grid::from_reader("1\n\n3\n=COUNT(A1:A3)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 3)), Ok(Value::Number(2.0)));
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
    fn evaluates_a_non_numeric_cell_as_text() {
        let grid = Grid::from_reader("hello".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 0)),
            Ok(Value::Text("hello".to_string()))
        );
    }

    #[test]
    fn arithmetic_on_a_text_cell_is_an_error() {
        let grid = Grid::from_reader("hello\n=A1+1".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 1)),
            Err(EvalError::NotANumber("hello".to_string()))
        );
    }

    #[test]
    fn a_range_containing_text_is_an_error_in_sum() {
        let grid = Grid::from_reader("1\nhello\n=SUM(A1:A2)".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 2)),
            Err(EvalError::NotANumber("hello".to_string()))
        );
    }

    #[test]
    fn concatenates_text_and_numbers() {
        let grid = Grid::from_reader(r#"Total,=" "&1&2"#.as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(1, 0)),
            Ok(Value::Text(" 12".to_string()))
        );
    }

    #[test]
    fn concatenates_cell_references() {
        let grid = Grid::from_reader("Jane,Doe\n=A1&\" \"&B1".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 1)),
            Ok(Value::Text("Jane Doe".to_string()))
        );
    }

    #[test]
    fn concatenation_binds_looser_than_addition() {
        let grid = Grid::from_reader(r#"="x"&1+2"#.as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 0)),
            Ok(Value::Text("x3".to_string()))
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

    #[test]
    fn if_picks_the_true_branch_on_a_nonzero_condition() {
        let grid = Grid::from_reader("=IF(1,10,20)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 0)), Ok(Value::Number(10.0)));
    }

    #[test]
    fn if_picks_the_false_branch_on_a_zero_condition() {
        let grid = Grid::from_reader("=IF(0,10,20)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 0)), Ok(Value::Number(20.0)));
    }

    #[test]
    fn if_condition_can_be_a_cell_reference() {
        let grid = Grid::from_reader("5\n=IF(A1,1,-1)".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 1)), Ok(Value::Number(1.0)));
    }

    #[test]
    fn if_does_not_evaluate_the_untaken_branch() {
        // The false branch calls a function that doesn't exist, but since
        // the condition is true it should never be evaluated.
        let grid = Grid::from_reader("=IF(1,42,NOPE())".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 0)), Ok(Value::Number(42.0)));
    }

    #[test]
    fn if_can_return_text_branches() {
        let grid = Grid::from_reader(r#"=IF(1,"yes","no")"#.as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 0)),
            Ok(Value::Text("yes".to_string()))
        );
    }

    #[test]
    fn if_condition_can_be_a_comparison() {
        let grid = Grid::from_reader("15\n=IF(A1>10,\"big\",\"small\")".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 1)),
            Ok(Value::Text("big".to_string()))
        );
    }

    #[test]
    fn evaluates_each_comparison_operator() {
        let grid = Grid::from_reader("=1<2,=1>2,=1<=1,=2>=3,=1=1,=1<>2".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 0)), Ok(Value::Bool(true)));
        assert_eq!(eval_cell(&grid, CellRef::new(1, 0)), Ok(Value::Bool(false)));
        assert_eq!(eval_cell(&grid, CellRef::new(2, 0)), Ok(Value::Bool(true)));
        assert_eq!(eval_cell(&grid, CellRef::new(3, 0)), Ok(Value::Bool(false)));
        assert_eq!(eval_cell(&grid, CellRef::new(4, 0)), Ok(Value::Bool(true)));
        assert_eq!(eval_cell(&grid, CellRef::new(5, 0)), Ok(Value::Bool(true)));
    }

    #[test]
    fn compares_text_cells_lexicographically() {
        let grid = Grid::from_reader("apple,banana\n=A1<B1".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 1)), Ok(Value::Bool(true)));
    }

    #[test]
    fn a_comparison_can_be_used_as_a_number() {
        let grid = Grid::from_reader("5\n=(A1>3)*10".as_bytes()).unwrap();
        assert_eq!(eval_cell(&grid, CellRef::new(0, 1)), Ok(Value::Number(10.0)));
    }

    #[test]
    fn a_comparison_concatenates_as_true_or_false() {
        let grid = Grid::from_reader(r#"="ok: "&(1<2)"#.as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 0)),
            Ok(Value::Text("ok: TRUE".to_string()))
        );
    }

    #[test]
    fn if_rejects_a_text_condition() {
        let grid = Grid::from_reader("hi\n=IF(A1,1,2)".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 1)),
            Err(EvalError::NotANumber("hi".to_string()))
        );
    }

    #[test]
    fn if_rejects_the_wrong_number_of_arguments() {
        let grid = Grid::from_reader("=IF(1,2)".as_bytes()).unwrap();
        assert_eq!(
            eval_cell(&grid, CellRef::new(0, 0)),
            Err(EvalError::WrongArgumentCount("IF".to_string(), 3, 2))
        );
    }
}
