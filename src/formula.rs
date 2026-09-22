use crate::address::CellRef;

/// A lexical token from a formula body (the part after the leading `=`).
/// This is deliberately just the tokenizer for now - turning a token stream
/// into a value requires operator precedence and function dispatch, which
/// come once there's a real evaluator to feed them into.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Text(String),
    Cell(CellRef),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Amp,
    Colon,
    Comma,
    LParen,
    RParen,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
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
            '&' => {
                tokens.push(Token::Amp);
                i += 1;
            }
            '=' => {
                tokens.push(Token::Eq);
                i += 1;
            }
            '<' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Le);
                    i += 2;
                } else if chars.get(i + 1) == Some(&'>') {
                    tokens.push(Token::Ne);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Ge);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '"' => {
                i += 1;
                let mut text = String::new();
                loop {
                    match chars.get(i) {
                        None => return Err("unterminated string literal".to_string()),
                        Some('"') if chars.get(i + 1) == Some(&'"') => {
                            text.push('"');
                            i += 2;
                        }
                        Some('"') => {
                            i += 1;
                            break;
                        }
                        Some(c) => {
                            text.push(*c);
                            i += 1;
                        }
                    }
                }
                tokens.push(Token::Text(text));
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
        assert!(tokenize("=A1?B1").is_err());
    }

    #[test]
    fn tokenizes_a_string_literal() {
        let tokens = tokenize(r#"="hello""#).unwrap();
        assert_eq!(tokens, vec![Token::Text("hello".to_string())]);
    }

    #[test]
    fn a_doubled_quote_is_a_literal_quote_in_a_string_literal() {
        let tokens = tokenize(r#"="she said ""hi""""#).unwrap();
        assert_eq!(tokens, vec![Token::Text(r#"she said "hi""#.to_string())]);
    }

    #[test]
    fn rejects_an_unterminated_string_literal() {
        assert!(tokenize(r#"="hello"#).is_err());
    }

    #[test]
    fn tokenizes_a_comparison_against_a_cell() {
        let tokens = tokenize("=A1<=1").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Cell(CellRef::new(0, 0)),
                Token::Le,
                Token::Number(1.0),
            ]
        );
    }

    #[test]
    fn distinguishes_lt_gt_and_ne() {
        assert_eq!(tokenize("=1<2").unwrap(), vec![Token::Number(1.0), Token::Lt, Token::Number(2.0)]);
        assert_eq!(tokenize("=1>2").unwrap(), vec![Token::Number(1.0), Token::Gt, Token::Number(2.0)]);
        assert_eq!(tokenize("=1<>2").unwrap(), vec![Token::Number(1.0), Token::Ne, Token::Number(2.0)]);
        assert_eq!(tokenize("=1>=2").unwrap(), vec![Token::Number(1.0), Token::Ge, Token::Number(2.0)]);
        assert_eq!(tokenize("=1=2").unwrap(), vec![Token::Number(1.0), Token::Eq, Token::Number(2.0)]);
    }

    #[test]
    fn tokenizes_concatenation() {
        let tokens = tokenize(r#"=A1&"x""#).unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Cell(CellRef::new(0, 0)),
                Token::Amp,
                Token::Text("x".to_string()),
            ]
        );
    }
}
