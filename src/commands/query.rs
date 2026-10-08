use crate::{cli::Invocation, compat, error::PhError, io};
use polars::prelude::*;

#[derive(Clone, Copy)]
enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

// Restricted grammar: <column> <operator> <literal>. Quoted string literals are allowed.
// This is deliberately not an implementation of arbitrary pandas DataFrame.query expressions.
fn parse(expr: &str) -> Result<(&str, Op, &str), PhError> {
    for (symbol, op) in [
        ("==", Op::Eq),
        ("!=", Op::Ne),
        (">=", Op::Ge),
        ("<=", Op::Le),
        (">", Op::Gt),
        ("<", Op::Lt),
    ] {
        if let Some((lhs, rhs)) = expr.split_once(symbol) {
            let lhs = lhs.trim();
            let rhs = rhs.trim();
            if !lhs.is_empty() && !rhs.is_empty() {
                return Ok((lhs, op, rhs));
            }
        }
    }
    Err(PhError::new(format!(
        "Unsupported query expression: {expr}."
    )))
}

fn compare<T: PartialOrd + PartialEq>(a: T, b: T, op: Op) -> bool {
    match op {
        Op::Eq => a == b,
        Op::Ne => a != b,
        Op::Gt => a > b,
        Op::Ge => a >= b,
        Op::Lt => a < b,
        Op::Le => a <= b,
    }
}

pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let (name, op, literal) = parse(&inv.args[0])?;
    compat::columns::require(&df, name)?;
    let col = df.column(name)?;
    let is_numeric = matches!(
        col.dtype(),
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
    );
    let mut selected = Vec::with_capacity(df.height());
    if is_numeric {
        let value: f64 = literal
            .parse()
            .map_err(|_| PhError::new(format!("Expected numeric literal: {literal}.")))?;
        let numeric = col.cast(&DataType::Float64)?;
        for v in numeric.f64()?.iter() {
            selected.push(v.map(|v| compare(v, value, op)).unwrap_or(false));
        }
    } else if matches!(col.dtype(), DataType::String) {
        let value = if literal.len() >= 2
            && ((literal.starts_with("'") && literal.ends_with("'"))
                || (literal.starts_with('"') && literal.ends_with('"')))
        {
            &literal[1..literal.len() - 1]
        } else {
            literal
        };
        for v in col.str()?.iter() {
            selected.push(v.map(|v| compare(v, value, op)).unwrap_or(false));
        }
    } else {
        return Err(PhError::new(format!("Unsupported query dtype for {name}.")));
    }
    let mask = BooleanChunked::from_slice("".into(), &selected);
    io::write_csv(df.filter(&mask)?)
}
