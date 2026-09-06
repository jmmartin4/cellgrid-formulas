//! Exercises the public API the way a consumer would: load a grid, look up
//! and evaluate cells, without reaching into any of the crate's internals.
//! Unit tests in each module cover the pieces in isolation; this file is
//! here to catch breakage in how those pieces fit together.

use cellgrid_formulas::{eval_cell, CellRef, EvalError, Grid};

#[test]
fn loads_and_evaluates_a_small_budget_sheet() {
    let csv = "\
Rent,1200
Groceries,340.50
Utilities,95
Total,=SUM(B1:B3)
Average,=AVERAGE(B1:B3)
";
    let grid = Grid::from_reader(csv.as_bytes()).unwrap();

    let label = CellRef::parse("A1").unwrap();
    assert_eq!(grid.get(label), Some("Rent"));

    let total = CellRef::parse("B4").unwrap();
    assert_eq!(eval_cell(&grid, total), Ok(1635.5));

    let average = CellRef::parse("B5").unwrap();
    assert_eq!(eval_cell(&grid, average), Ok((1200.0 + 340.50 + 95.0) / 3.0));
}

#[test]
fn a_formula_can_build_on_another_formula_several_cells_away() {
    let csv = "10,20,30\n=SUM(A1:C1),,\n=B2*2,,\n";
    let grid = Grid::from_reader(csv.as_bytes()).unwrap();

    let doubled_total = CellRef::parse("A3").unwrap();
    assert_eq!(eval_cell(&grid, doubled_total), Ok(120.0));
}

#[test]
fn surfaces_a_circular_reference_through_the_public_error_type() {
    let grid = Grid::from_reader("=B1\n=A1".as_bytes()).unwrap();
    let a1 = CellRef::parse("A1").unwrap();
    assert_eq!(
        eval_cell(&grid, a1),
        Err(EvalError::CircularReference(a1))
    );
}

#[test]
fn round_trips_a_quoted_csv_export_through_evaluation() {
    let csv = "\"Smith, John\",50\n\"Doe, Jane\",75\nTotal,=SUM(B1:B2)\n";
    let grid = Grid::from_reader(csv.as_bytes()).unwrap();

    let name = CellRef::parse("A1").unwrap();
    assert_eq!(grid.get(name), Some("Smith, John"));

    let total = CellRef::parse("B3").unwrap();
    assert_eq!(eval_cell(&grid, total), Ok(125.0));
}
