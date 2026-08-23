//! Building blocks for reading spreadsheet-like grids (A1-style cell
//! addresses, formula text) from a file or from stdin, and for breaking
//! formula strings into tokens.

pub mod address;
pub mod formula;
pub mod grid;

pub use address::CellRef;
pub use grid::Grid;
