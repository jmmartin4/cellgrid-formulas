use crate::address::CellRef;
use crate::formula::Token;

/// A parsed formula expression. This is what the future evaluator will walk;
/// the parser's only job is turning a flat token stream into this tree with
/// the right operator precedence and grouping.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Cell(CellRef),
    Range(CellRef, CellRef),
    Neg(Box<Expr>),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Parses a full token stream (as produced by `formula::tokenize`) into an
/// `Expr`. Errors out on trailing tokens rather than silently ignoring them,
/// since a leftover `)` or operator almost always means the input was
/// malformed in a way worth surfacing.
pub fn parse(tokens: &[Token]) -> Result<Expr, String> {
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.parse_expr()?;
    if let Some(tok) = parser.peek() {
        return Err(format!("unexpected trailing token: {tok:?}"));
    }
    Ok(expr)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.advance() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => Err(format!("expected {expected:?}, found {tok:?}")),
            None => Err(format!("expected {expected:?}, found end of input")),
        }
    }

    // expr := term (('+' | '-') term)*
    fn parse_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    // term := unary (('*' | '/') unary)*
    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    // unary := ('-' | '+') unary | primary
    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Token::Minus) => {
                self.advance();
                Ok(Expr::Neg(Box::new(self.parse_unary()?)))
            }
            Some(Token::Plus) => {
                self.advance();
                self.parse_unary()
            }
            _ => self.parse_primary(),
        }
    }

    // primary := Number
    //          | Cell (':' Cell)?
    //          | Ident '(' (expr (',' expr)*)? ')'
    //          | '(' expr ')'
    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.advance().cloned() {
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
            Some(Token::Cell(cell)) => {
                if let Some(Token::Colon) = self.peek() {
                    self.advance();
                    match self.advance().cloned() {
                        Some(Token::Cell(end)) => Ok(Expr::Range(cell, end)),
                        Some(other) => {
                            Err(format!("expected cell reference after ':', found {other:?}"))
                        }
                        None => Err("expected cell reference after ':'".to_string()),
                    }
                } else {
                    Ok(Expr::Cell(cell))
                }
            }
            Some(Token::Ident(name)) => {
                self.expect(&Token::LParen)?;
                let mut args = Vec::new();
                if self.peek() != Some(&Token::RParen) {
                    args.push(self.parse_expr()?);
                    while self.peek() == Some(&Token::Comma) {
                        self.advance();
                        args.push(self.parse_expr()?);
                    }
                }
                self.expect(&Token::RParen)?;
                Ok(Expr::Call(name, args))
            }
            Some(Token::LParen) => {
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(other) => Err(format!("unexpected token: {other:?}")),
            None => Err("unexpected end of input".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::tokenize;

    fn parse_str(formula: &str) -> Expr {
        parse(&tokenize(formula).unwrap()).unwrap()
    }

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        // 1+2*3 should be 1+(2*3), not (1+2)*3.
        let expr = parse_str("=1+2*3");
        assert_eq!(
            expr,
            Expr::BinOp(
                Box::new(Expr::Number(1.0)),
                BinOp::Add,
                Box::new(Expr::BinOp(
                    Box::new(Expr::Number(2.0)),
                    BinOp::Mul,
                    Box::new(Expr::Number(3.0)),
                )),
            )
        );
    }

    #[test]
    fn parentheses_override_precedence() {
        // (1+2)*3 should keep the addition together.
        let expr = parse_str("=(1+2)*3");
        assert_eq!(
            expr,
            Expr::BinOp(
                Box::new(Expr::BinOp(
                    Box::new(Expr::Number(1.0)),
                    BinOp::Add,
                    Box::new(Expr::Number(2.0)),
                )),
                BinOp::Mul,
                Box::new(Expr::Number(3.0)),
            )
        );
    }

    #[test]
    fn parses_unary_minus() {
        let expr = parse_str("=-5+3");
        assert_eq!(
            expr,
            Expr::BinOp(
                Box::new(Expr::Neg(Box::new(Expr::Number(5.0)))),
                BinOp::Add,
                Box::new(Expr::Number(3.0)),
            )
        );
    }

    #[test]
    fn parses_function_calls_with_a_range_argument() {
        let expr = parse_str("=SUM(A1:A3)");
        assert_eq!(
            expr,
            Expr::Call(
                "SUM".to_string(),
                vec![Expr::Range(CellRef::new(0, 0), CellRef::new(0, 2))],
            )
        );
    }

    #[test]
    fn parses_function_calls_with_multiple_arguments() {
        let expr = parse_str("=MAX(A1,B1,10)");
        assert_eq!(
            expr,
            Expr::Call(
                "MAX".to_string(),
                vec![
                    Expr::Cell(CellRef::new(0, 0)),
                    Expr::Cell(CellRef::new(1, 0)),
                    Expr::Number(10.0),
                ],
            )
        );
    }

    #[test]
    fn rejects_unbalanced_parentheses() {
        let tokens = tokenize("=(1+2").unwrap();
        assert!(parse(&tokens).is_err());
    }

    #[test]
    fn rejects_trailing_tokens() {
        let tokens = tokenize("=1+2)").unwrap();
        assert!(parse(&tokens).is_err());
    }

    #[test]
    fn rejects_function_call_missing_parens() {
        let tokens = tokenize("=SUM").unwrap();
        assert!(parse(&tokens).is_err());
    }
}
