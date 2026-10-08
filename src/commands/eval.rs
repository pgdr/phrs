//! Small arithmetic-expression frontend for Polars lazy expressions.
//! Grammar: optional `identifier =`, numbers, column identifiers, parentheses,
//! unary +/- and binary +, -, *, /, ** (right-associative).
use crate::{cli::Invocation, error::PhError, io};
use polars::prelude::*;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Number(String),
    Plus,
    Minus,
    Star,
    Slash,
    Power,
    LParen,
    RParen,
    Equal,
}

fn tokenize(input: &str) -> Result<Vec<Token>, PhError> {
    let mut chars = input.char_indices().peekable();
    let mut out = Vec::new();
    while let Some((start, c)) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        let token = match c {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '/' => Token::Slash,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '=' => Token::Equal,
            '*' => {
                if chars.peek().is_some_and(|(_, next)| *next == '*') {
                    chars.next();
                    Token::Power
                } else {
                    Token::Star
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut end = start + c.len_utf8();
                while let Some(&(i, ch)) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        end = i + ch.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                Token::Name(input[start..end].to_owned())
            }
            c if c.is_ascii_digit() || c == '.' => {
                let mut end = start + c.len_utf8();
                let mut seen_dot = c == '.';
                let mut seen_exponent = false;
                while let Some(&(i, ch)) = chars.peek() {
                    if ch.is_ascii_digit() {
                        end = i + 1;
                        chars.next();
                    } else if ch == '.' && !seen_dot && !seen_exponent {
                        seen_dot = true;
                        end = i + 1;
                        chars.next();
                    } else if (ch == 'e' || ch == 'E') && !seen_exponent {
                        seen_exponent = true;
                        end = i + 1;
                        chars.next();
                        if let Some(&(j, sign)) = chars.peek()
                            && (sign == '+' || sign == '-')
                        {
                            end = j + 1;
                            chars.next();
                        }
                    } else {
                        break;
                    }
                }
                let number = &input[start..end];
                if number.parse::<f64>().is_err() {
                    return Err(PhError::new(format!("Invalid number: {number}.")));
                }
                Token::Number(number.to_owned())
            }
            _ => {
                return Err(PhError::new(format!(
                    "Unexpected character in eval expression: {c}."
                )));
            }
        };
        out.push(token);
    }
    Ok(out)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    first_column: Option<String>,
}
impl Parser<'_> {
    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, PhError> {
        let mut lhs = match self.next().cloned() {
            Some(Token::Name(name)) => {
                if self.first_column.is_none() {
                    self.first_column = Some(name.clone());
                }
                col(name.as_str())
            }
            Some(Token::Number(s)) => {
                if !s.chars().any(|c| matches!(c, '.' | 'e' | 'E')) {
                    if let Ok(n) = s.parse::<i64>() {
                        lit(n)
                    } else {
                        lit(s
                            .parse::<f64>()
                            .map_err(|_| PhError::new("Invalid number."))?)
                    }
                } else {
                    lit(s
                        .parse::<f64>()
                        .map_err(|_| PhError::new("Invalid number."))?)
                }
            }
            Some(Token::Plus) => self.parse_expr(5)?,
            Some(Token::Minus) => lit(0) - self.parse_expr(5)?,
            Some(Token::LParen) => {
                let expr = self.parse_expr(0)?;
                if !matches!(self.next(), Some(Token::RParen)) {
                    return Err(PhError::new(
                        "Missing closing parenthesis in eval expression.",
                    ));
                }
                expr
            }
            _ => {
                return Err(PhError::new(
                    "Expected a value or column name in eval expression.",
                ));
            }
        };
        loop {
            let (left_bp, right_bp) = match self.peek() {
                Some(Token::Plus | Token::Minus) => (1, 2),
                Some(Token::Star | Token::Slash) => (3, 4),
                Some(Token::Power) => (7, 7),
                _ => break,
            };
            if left_bp < min_bp {
                break;
            }
            let op = self.next().expect("peeked above").clone();
            let rhs = self.parse_expr(right_bp)?;
            lhs = match op {
                Token::Plus => lhs + rhs,
                Token::Minus => lhs - rhs,
                Token::Star => lhs * rhs,
                Token::Slash => lhs.cast(DataType::Float64) / rhs.cast(DataType::Float64),
                Token::Power => lhs.pow(rhs),
                _ => unreachable!(),
            };
        }
        Ok(lhs)
    }
    fn next(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        self.pos += usize::from(t.is_some());
        t
    }
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }
}

pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let tokens = tokenize(&inv.args[0])?;
    let (assignment, rhs) = match tokens.as_slice() {
        [Token::Name(name), Token::Equal, rest @ ..] => (Some(name.clone()), rest),
        _ => (None, tokens.as_slice()),
    };
    if rhs.is_empty() {
        return Err(PhError::new("Missing eval expression."));
    }
    let mut parser = Parser {
        tokens: rhs,
        pos: 0,
        first_column: None,
    };
    let expr = parser.parse_expr(0)?;
    if parser.peek().is_some() {
        return Err(PhError::new("Unexpected token in eval expression."));
    }
    let result = if let Some(name) = assignment {
        df.lazy().with_column(expr.alias(name.as_str())).collect()?
    } else {
        // Python ph uses the first referenced column as the output name for
        // an unassigned scalar expression, e.g. `eval "x**2"` -> header `x`.
        let name = parser.first_column.unwrap_or_else(|| "result".to_owned());
        df.lazy().select([expr.alias(name.as_str())]).collect()?
    };
    io::write_csv(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lex_numbers_and_names() {
        assert_eq!(
            tokenize("z = x**2 + 1.5e-2").unwrap(),
            vec![
                Token::Name("z".into()),
                Token::Equal,
                Token::Name("x".into()),
                Token::Power,
                Token::Number("2".into()),
                Token::Plus,
                Token::Number("1.5e-2".into())
            ]
        );
    }
    #[test]
    fn bad_lex() {
        assert!(tokenize("x @ 2").is_err());
    }
}
