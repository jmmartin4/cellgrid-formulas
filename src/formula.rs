use crate::address::CellRef;

/// A lexical token from a formula body (the part after the leading `=`).
/// This is deliberately just the tokenizer for now - turning a token stream
/// into a value requires operator precedence and function dispatch, which
/// come once there's a real evaluator to feed them into.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Cell(CellRef),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Colon,
    Comma,
    LParen,
    RParen,
}

pub fn tokenize(formula: &str) -> Result<Vec<Token>, String> {
    let body = formula.strip_prefix('=').unwrap_or(formula);
    let chars: Vec<char> = body.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            ':' => {
                tokens.push(Token::Colon);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                let n: f64 = text
                    .parse()
                    .map_err(|_| format!("invalid number: {text}"))?;
                tokens.push(Token::Number(n));
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                match CellRef::parse(&text) {
                    Some(cell) => tokens.push(Token::Cell(cell)),
                    None => tokens.push(Token::Ident(text)),
                }
            }
            other => return Err(format!("unexpected character: {other}")),
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_a_sum_range() {
        let tokens = tokenize("=SUM(A1:A3)").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Ident("SUM".to_string()),
                Token::LParen,
                Token::Cell(CellRef::new(0, 0)),
                Token::Colon,
                Token::Cell(CellRef::new(0, 2)),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn tokenizes_arithmetic() {
        let tokens = tokenize("=A1+10*2").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Cell(CellRef::new(0, 0)),
                Token::Plus,
                Token::Number(10.0),
                Token::Star,
                Token::Number(2.0),
            ]
        );
    }

    #[test]
    fn rejects_unknown_characters() {
        assert!(tokenize("=A1&B1").is_err());
    }
}
