//! Restricted formula language for computed fields.
//!
//! Expressions are parsed into an AST and evaluated in-process. There is no
//! `eval`, no dynamic SQL, and no arbitrary code execution.

use crate::error::{QefroError, QefroResult};
use crate::field::{FieldDef, FieldType};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Func {
    Sum,
    Min,
    Max,
    Count,
    Round,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Field(String),
    ChildField { table: String, field: String },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call { func: Func, args: Vec<Expr> },
}

#[derive(Debug, Clone)]
pub struct FormulaContext<'a> {
    pub record: &'a Value,
    pub children: &'a HashMap<String, Vec<Value>>,
}

pub fn parse_formula(input: &str) -> QefroResult<Expr> {
    let mut p = Parser {
        src: input.trim(),
        pos: 0,
    };
    if p.src.is_empty() {
        return Err(QefroError::bad_request("formula is empty"));
    }
    let expr = p.parse_expr()?;
    p.skip_ws();
    if p.pos != p.src.len() {
        return Err(QefroError::bad_request(format!(
            "unexpected input in formula at '{}'",
            &p.src[p.pos..]
        )));
    }
    Ok(expr)
}

pub fn eval_formula(expr: &Expr, ctx: &FormulaContext<'_>) -> QefroResult<f64> {
    match expr {
        Expr::Number(n) => Ok(*n),
        Expr::Field(name) => {
            if let Some(rows) = ctx.children.get(name) {
                return Ok(rows.len() as f64);
            }
            number_at(ctx.record, name)
        }
        Expr::ChildField { table, field } => {
            let rows = ctx.children.get(table).map(|v| v.as_slice()).unwrap_or(&[]);
            if rows.len() == 1 {
                return number_at(&rows[0], field);
            }
            Err(QefroError::bad_request(format!(
                "child field '{table}.{field}' must be used inside SUM/MIN/MAX/COUNT"
            )))
        }
        Expr::Binary { op, left, right } => {
            let l = eval_formula(left, ctx)?;
            let r = eval_formula(right, ctx)?;
            match op {
                BinOp::Add => Ok(l + r),
                BinOp::Sub => Ok(l - r),
                BinOp::Mul => Ok(l * r),
                BinOp::Div => {
                    if r == 0.0 {
                        return Err(QefroError::bad_request("division by zero in formula"));
                    }
                    Ok(l / r)
                }
                BinOp::Mod => {
                    if r == 0.0 {
                        return Err(QefroError::bad_request("modulo by zero in formula"));
                    }
                    Ok(l % r)
                }
            }
        }
        Expr::Call { func, args } => eval_call(func, args, ctx),
    }
}

fn eval_call(func: &Func, args: &[Expr], ctx: &FormulaContext<'_>) -> QefroResult<f64> {
    match func {
        Func::Round => {
            if args.is_empty() || args.len() > 2 {
                return Err(QefroError::bad_request("ROUND takes 1 or 2 arguments"));
            }
            let value = eval_formula(&args[0], ctx)?;
            let digits = if args.len() == 2 {
                eval_formula(&args[1], ctx)? as i32
            } else {
                0
            };
            let factor = 10f64.powi(digits.max(0));
            Ok((value * factor).round() / factor)
        }
        Func::Count => {
            if args.len() != 1 {
                return Err(QefroError::bad_request("COUNT takes 1 argument"));
            }
            match &args[0] {
                Expr::Field(name) => Ok(ctx.children.get(name).map(|r| r.len()).unwrap_or(0) as f64),
                Expr::ChildField { table, field } => {
                    let rows = ctx.children.get(table).map(|v| v.as_slice()).unwrap_or(&[]);
                    Ok(rows
                        .iter()
                        .filter(|row| {
                            row.get(field.as_str())
                                .map(|v| !v.is_null())
                                .unwrap_or(false)
                        })
                        .count() as f64)
                }
                _ => Err(QefroError::bad_request(
                    "COUNT expects a child table or child field",
                )),
            }
        }
        Func::Sum | Func::Min | Func::Max => {
            if args.len() != 1 {
                return Err(QefroError::bad_request(format!(
                    "{func:?} takes 1 argument"
                )));
            }
            let values = collect_agg_values(&args[0], ctx)?;
            if values.is_empty() {
                return Ok(0.0);
            }
            match func {
                Func::Sum => Ok(values.iter().sum()),
                Func::Min => Ok(values.iter().cloned().fold(f64::INFINITY, f64::min)),
                Func::Max => Ok(values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
                _ => unreachable!(),
            }
        }
    }
}

fn collect_agg_values(expr: &Expr, ctx: &FormulaContext<'_>) -> QefroResult<Vec<f64>> {
    match expr {
        Expr::ChildField { table, field } => {
            let rows = ctx.children.get(table).map(|v| v.as_slice()).unwrap_or(&[]);
            let mut out = Vec::new();
            for row in rows {
                if let Ok(n) = number_at(row, field) {
                    out.push(n);
                }
            }
            Ok(out)
        }
        Expr::Field(name) => {
            if let Some(rows) = ctx.children.get(name) {
                return Ok(vec![rows.len() as f64]);
            }
            Ok(vec![number_at(ctx.record, name)?])
        }
        other => Ok(vec![eval_formula(other, ctx)?]),
    }
}

fn number_at(record: &Value, name: &str) -> QefroResult<f64> {
    let Some(value) = record.get(name) else {
        return Ok(0.0);
    };
    match value {
        Value::Null => Ok(0.0),
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| QefroError::bad_request(format!("field '{name}' is not numeric"))),
        Value::String(s) => s
            .parse::<f64>()
            .map_err(|_| QefroError::bad_request(format!("field '{name}' is not numeric"))),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        _ => Err(QefroError::bad_request(format!(
            "field '{name}' cannot be used in a formula"
        ))),
    }
}

pub fn formula_dependencies(expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    walk_deps(expr, &mut out);
    out.sort();
    out.dedup();
    out
}

fn walk_deps(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Number(_) => {}
        Expr::Field(name) => out.push(name.clone()),
        Expr::ChildField { table, field } => out.push(format!("{table}.{field}")),
        Expr::Binary { left, right, .. } => {
            walk_deps(left, out);
            walk_deps(right, out);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                walk_deps(arg, out);
            }
        }
    }
}

/// Detect circular computed-field dependencies within a single entity.
pub fn detect_cycles(fields: &[FieldDef]) -> QefroResult<()> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let computed: HashSet<String> = fields
        .iter()
        .filter(|f| f.computed)
        .map(|f| f.name.clone())
        .collect();
    for field in fields {
        if !field.computed {
            continue;
        }
        let Some(formula) = &field.formula else {
            return Err(QefroError::bad_request(format!(
                "computed field '{}' is missing a formula",
                field.name
            )));
        };
        let expr = parse_formula(formula)?;
        let deps = formula_dependencies(&expr)
            .into_iter()
            .filter(|d| computed.contains(d) || d.contains('.'))
            .collect();
        graph.insert(field.name.clone(), deps);
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for name in graph.keys() {
        visit_cycle(name, &graph, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit_cycle(
    node: &str,
    graph: &HashMap<String, Vec<String>>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> QefroResult<()> {
    if visited.contains(node) {
        return Ok(());
    }
    if !visiting.insert(node.to_string()) {
        return Err(QefroError::bad_request(format!(
            "circular formula dependency involving '{node}'"
        )));
    }
    if let Some(deps) = graph.get(node) {
        for dep in deps {
            if graph.contains_key(dep) {
                visit_cycle(dep, graph, visiting, visited)?;
            }
        }
    }
    visiting.remove(node);
    visited.insert(node.to_string());
    Ok(())
}

/// Stable evaluation order: dependencies first.
pub fn evaluation_order(fields: &[FieldDef]) -> QefroResult<Vec<String>> {
    detect_cycles(fields)?;
    let mut remaining: Vec<&FieldDef> = fields.iter().filter(|f| f.computed).collect();
    let mut done: HashSet<String> = HashSet::new();
    let mut order = Vec::new();
    while !remaining.is_empty() {
        let mut progressed = false;
        remaining.retain(|field| {
            let expr = match field.formula.as_deref().and_then(|f| parse_formula(f).ok()) {
                Some(e) => e,
                None => return true,
            };
            let ready = formula_dependencies(&expr).iter().all(|d| {
                !fields.iter().any(|f| f.computed && f.name == *d) || done.contains(d)
            });
            if ready {
                done.insert(field.name.clone());
                order.push(field.name.clone());
                progressed = true;
                false
            } else {
                true
            }
        });
        if !progressed {
            return Err(QefroError::bad_request(
                "could not order computed fields (circular dependency)",
            ));
        }
    }
    Ok(order)
}

pub fn apply_computed_fields(
    fields: &[FieldDef],
    record: &mut Value,
    children: &HashMap<String, Vec<Value>>,
) -> QefroResult<()> {
    let order = evaluation_order(fields)?;
    for name in &order {
        let field = fields.iter().find(|f| f.name == *name).expect("field");
        let formula = field.formula.as_deref().unwrap_or("");
        let expr = parse_formula(formula)?;
        let value = {
            let ctx = FormulaContext { record, children };
            eval_formula(&expr, &ctx)?
        };
        if let Some(obj) = record.as_object_mut() {
            obj.insert(name.clone(), numeric_value(field, value));
        }
    }
    Ok(())
}

fn numeric_value(field: &FieldDef, value: f64) -> Value {
    match field.field_type {
        FieldType::Integer => json_int(value),
        _ => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::from(value as i64)),
    }
}

fn json_int(value: f64) -> Value {
    Value::from(value.round() as i64)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn parse_expr(&mut self) -> QefroResult<Expr> {
        let mut left = self.parse_term()?;
        loop {
            self.skip_ws();
            let op = match self.peek() {
                Some('+') => BinOp::Add,
                Some('-') => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_term()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> QefroResult<Expr> {
        let mut left = self.parse_factor()?;
        loop {
            self.skip_ws();
            let op = match self.peek() {
                Some('*') => BinOp::Mul,
                Some('/') => BinOp::Div,
                Some('%') => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.parse_factor()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> QefroResult<Expr> {
        self.skip_ws();
        match self.peek() {
            Some('(') => {
                self.bump();
                let inner = self.parse_expr()?;
                self.skip_ws();
                if self.bump() != Some(')') {
                    return Err(QefroError::bad_request("missing ')' in formula"));
                }
                Ok(inner)
            }
            Some('-') => {
                self.bump();
                let inner = self.parse_factor()?;
                Ok(Expr::Binary {
                    op: BinOp::Sub,
                    left: Box::new(Expr::Number(0.0)),
                    right: Box::new(inner),
                })
            }
            Some(c) if c.is_ascii_digit() || c == '.' => self.parse_number(),
            Some(c) if c.is_ascii_alphabetic() || c == '_' => self.parse_ident_or_call(),
            Some(c) => Err(QefroError::bad_request(format!(
                "unexpected '{c}' in formula"
            ))),
            None => Err(QefroError::bad_request("unexpected end of formula")),
        }
    }

    fn parse_number(&mut self) -> QefroResult<Expr> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == '.') {
            self.bump();
        }
        let raw = &self.src[start..self.pos];
        let n = raw
            .parse::<f64>()
            .map_err(|_| QefroError::bad_request(format!("invalid number '{raw}'")))?;
        Ok(Expr::Number(n))
    }

    fn parse_ident_or_call(&mut self) -> QefroResult<Expr> {
        let ident = self.parse_ident();
        self.skip_ws();
        if self.peek() == Some('(') {
            self.bump();
            let mut args = Vec::new();
            self.skip_ws();
            if self.peek() != Some(')') {
                loop {
                    args.push(self.parse_expr()?);
                    self.skip_ws();
                    match self.peek() {
                        Some(',') => {
                            self.bump();
                        }
                        Some(')') => break,
                        _ => {
                            return Err(QefroError::bad_request(
                                "expected ',' or ')' in function call",
                            ))
                        }
                    }
                }
            }
            if self.bump() != Some(')') {
                return Err(QefroError::bad_request("missing ')' in function call"));
            }
            let func = match ident.to_ascii_uppercase().as_str() {
                "SUM" => Func::Sum,
                "MIN" => Func::Min,
                "MAX" => Func::Max,
                "COUNT" => Func::Count,
                "ROUND" => Func::Round,
                other => {
                    return Err(QefroError::bad_request(format!(
                        "unknown formula function '{other}'"
                    )))
                }
            };
            return Ok(Expr::Call { func, args });
        }
        if self.peek() == Some('.') {
            self.bump();
            let field = self.parse_ident();
            if field.is_empty() {
                return Err(QefroError::bad_request("expected field after '.'"));
            }
            return Ok(Expr::ChildField {
                table: ident,
                field,
            });
        }
        Ok(Expr::Field(ident))
    }

    fn parse_ident(&mut self) -> String {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            self.bump();
        }
        self.src[start..self.pos].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldDef;
    use serde_json::json;

    fn ctx<'a>(record: &'a Value, children: &'a HashMap<String, Vec<Value>>) -> FormulaContext<'a> {
        FormulaContext { record, children }
    }

    #[test]
    fn arithmetic_and_precedence() {
        let expr = parse_formula("1 + 2 * 3").unwrap();
        let record = json!({});
        let children = HashMap::new();
        assert_eq!(eval_formula(&expr, &ctx(&record, &children)).unwrap(), 7.0);
        let expr = parse_formula("(1 + 2) * 3").unwrap();
        assert_eq!(eval_formula(&expr, &ctx(&record, &children)).unwrap(), 9.0);
    }

    #[test]
    fn field_refs_and_child_sum() {
        let record = json!({ "discount": 10, "tax_rate": 0.1 });
        let children = HashMap::from([(
            "items".into(),
            vec![
                json!({ "quantity": 2, "rate": 300, "amount": 600 }),
                json!({ "quantity": 1, "rate": 100, "amount": 100 }),
            ],
        )]);
        let amount = parse_formula("quantity * rate").unwrap();
        assert_eq!(
            eval_formula(&amount, &ctx(&children["items"][0], &HashMap::new())).unwrap(),
            600.0
        );
        let subtotal = parse_formula("SUM(items.amount)").unwrap();
        assert_eq!(
            eval_formula(&subtotal, &ctx(&record, &children)).unwrap(),
            700.0
        );
        let grand = parse_formula("SUM(items.amount) - discount").unwrap();
        assert_eq!(eval_formula(&grand, &ctx(&record, &children)).unwrap(), 690.0);
        let count = parse_formula("COUNT(items)").unwrap();
        assert_eq!(eval_formula(&count, &ctx(&record, &children)).unwrap(), 2.0);
    }

    #[test]
    fn rejects_unknown_function_and_sql() {
        assert!(parse_formula("EVAL(1)").is_err());
        assert!(parse_formula("1; DROP TABLE orders").is_err());
        assert!(parse_formula("SUM(items.amount) + SELECT 1").is_err());
    }

    #[test]
    fn detects_cycles() {
        let fields = vec![
            FieldDef::decimal("a").computed("b + 1"),
            FieldDef::decimal("b").computed("a + 1"),
        ];
        let err = detect_cycles(&fields).unwrap_err();
        assert!(err.to_string().contains("circular"));
    }

    #[test]
    fn client_override_is_ignored_by_apply() {
        let fields = vec![
            FieldDef::decimal("quantity"),
            FieldDef::decimal("rate"),
            FieldDef::decimal("amount").computed("quantity * rate"),
        ];
        let mut record = json!({ "quantity": 2, "rate": 300, "amount": 999999 });
        let children = HashMap::new();
        apply_computed_fields(&fields, &mut record, &children).unwrap();
        assert_eq!(record["amount"].as_f64().unwrap(), 600.0);
    }

    #[test]
    fn round_and_modulo() {
        let record = json!({});
        let children = HashMap::new();
        let expr = parse_formula("ROUND(10 / 3, 2)").unwrap();
        assert!((eval_formula(&expr, &ctx(&record, &children)).unwrap() - 3.33).abs() < 0.001);
        let expr = parse_formula("10 % 3").unwrap();
        assert_eq!(eval_formula(&expr, &ctx(&record, &children)).unwrap(), 1.0);
    }
}
