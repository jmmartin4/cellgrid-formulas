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
    Bool(bool),
}

impl Value {
    /// Coerces to a number for arithmetic. There's no implicit "123" -> 123
    /// conversion for text - a formula that wants that should say so
    /// explicitly, once there's a function to do it, rather than have it
    /// happen silently and hide typos. A boolean is 1 or 0, so `=(A1>3)*10`
    /// and `SUM` over a column of comparisons work.
    pub fn as_number(&self) -> Result<f64, EvalError> {
        match self {
            Value::Number(n) => Ok(*n),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Text(s) => Err(EvalError::NotANumber(s.clone())),
        }
    }

    /// Truthiness for `IF`: a boolean is itself, a number is true unless it
    /// is zero, and text is an error rather than quietly true or false.
    pub fn as_bool(&self) -> Result<bool, EvalError> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Ok(other.as_number()? != 0.0),
        }
    }

    /// Ordering used by the comparison operators (`= <> < > <= >=`).
    /// Numbers compare numerically and text compares lexicographically; a
    /// number and a piece of text never coerce into each other for
    /// arithmetic, but a comparison still needs some answer, so values of
    /// different kinds order as number < text < boolean - the same rule
    /// spreadsheets use when sorting a column that mixes them. That also
    /// means `1=TRUE`-style comparisons are false rather than coerced.
    pub fn compare(&self, other: &Value) -> Ordering {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Value::Text(a), Value::Text(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            _ => self.kind_rank().cmp(&other.kind_rank()),
        }
    }

    fn kind_rank(&self) -> u8 {
        match self {
            Value::Number(_) => 0,
            Value::Text(_) => 1,
            Value::Bool(_) => 2,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n}"),
            Value::Text(s) => write!(f, "{s}"),
            Value::Bool(true) => write!(f, "TRUE"),
            Value::Bool(false) => write!(f, "FALSE"),
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
    fn booleans_coerce_to_one_and_zero() {
        assert_eq!(Value::Bool(true).as_number(), Ok(1.0));
        assert_eq!(Value::Bool(false).as_number(), Ok(0.0));
    }

    #[test]
    fn truthiness_follows_the_kind_of_value() {
        assert_eq!(Value::Bool(false).as_bool(), Ok(false));
        assert_eq!(Value::Number(2.0).as_bool(), Ok(true));
        assert_eq!(Value::Number(0.0).as_bool(), Ok(false));
        assert_eq!(
            Value::Text("x".to_string()).as_bool(),
            Err(EvalError::NotANumber("x".to_string()))
        );
    }

    #[test]
    fn displays_booleans_in_capitals() {
        assert_eq!(Value::Bool(true).to_string(), "TRUE");
        assert_eq!(Value::Bool(false).to_string(), "FALSE");
    }

    #[test]
    fn a_boolean_sorts_after_numbers_and_text() {
        assert_eq!(Value::Bool(false).compare(&Value::Number(5.0)), Ordering::Greater);
        assert_eq!(
            Value::Bool(false).compare(&Value::Text("z".to_string())),
            Ordering::Greater
        );
        assert_eq!(Value::Bool(false).compare(&Value::Bool(true)), Ordering::Less);
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
