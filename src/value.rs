use crate::eval::EvalError;
use std::cmp::Ordering;
use std::fmt;

/// What a cell evaluates to: either a number, for arithmetic, or text, for
/// concatenation and anything else that isn't meant to be computed on. A
/// plain (non-formula) cell that doesn't parse as a number is just text now
/// - it used to be a `NotANumber` error, but there was never a good reason
/// a spreadsheet couldn't hold a name in one column and a total in another.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Text(String),
}

impl Value {
    /// Coerces to a number for arithmetic. There's no implicit "123" -> 123
    /// conversion for text - a formula that wants that should say so
    /// explicitly, once there's a function to do it, rather than have it
    /// happen silently and hide typos.
    pub fn as_number(&self) -> Result<f64, EvalError> {
        match self {
            Value::Number(n) => Ok(*n),
            Value::Text(s) => Err(EvalError::NotANumber(s.clone())),
        }
    }

    /// Ordering used by the comparison operators (`= <> < > <= >=`).
    /// Numbers compare numerically and text compares lexicographically; a
    /// number and a piece of text never coerce into each other for
    /// arithmetic, but a comparison still needs some answer, so a number is
    /// always treated as less than any text - the same rule spreadsheets use
    /// when sorting a column that mixes the two.
    pub fn compare(&self, other: &Value) -> Ordering {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Value::Text(a), Value::Text(b)) => a.cmp(b),
            (Value::Number(_), Value::Text(_)) => Ordering::Less,
            (Value::Text(_), Value::Number(_)) => Ordering::Greater,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n}"),
            Value::Text(s) => write!(f, "{s}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_coerce_to_themselves() {
        assert_eq!(Value::Number(3.5).as_number(), Ok(3.5));
    }

    #[test]
    fn text_does_not_coerce_to_a_number() {
        assert_eq!(
            Value::Text("abc".to_string()).as_number(),
            Err(EvalError::NotANumber("abc".to_string()))
        );
    }

    #[test]
    fn displays_numbers_and_text_plainly() {
        assert_eq!(Value::Number(2.0).to_string(), "2");
        assert_eq!(Value::Text("hi".to_string()).to_string(), "hi");
    }

    #[test]
    fn compares_numbers_numerically() {
        assert_eq!(Value::Number(1.0).compare(&Value::Number(2.0)), Ordering::Less);
        assert_eq!(Value::Number(2.0).compare(&Value::Number(2.0)), Ordering::Equal);
        assert_eq!(Value::Number(3.0).compare(&Value::Number(2.0)), Ordering::Greater);
    }

    #[test]
    fn compares_text_lexicographically() {
        assert_eq!(
            Value::Text("apple".to_string()).compare(&Value::Text("banana".to_string())),
            Ordering::Less
        );
    }

    #[test]
    fn a_number_always_sorts_before_text() {
        assert_eq!(
            Value::Number(999.0).compare(&Value::Text("a".to_string())),
            Ordering::Less
        );
        assert_eq!(
            Value::Text("a".to_string()).compare(&Value::Number(999.0)),
            Ordering::Greater
        );
    }
}
