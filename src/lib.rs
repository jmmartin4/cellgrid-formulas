//! Building blocks for reading spreadsheet-like grids (A1-style cell
//! addresses, formula text) from a file or from stdin, parsing formula
//! strings into an expression tree, and evaluating that tree against a
//! grid to a number.

pub mod address;
pub mod eval;
pub mod formula;
pub mod grid;
pub mod parser;

pub use address::CellRef;
pub use eval::{eval_cell, EvalError};
pub use grid::Grid;
pub use parser::{parse, BinOp, Expr};
