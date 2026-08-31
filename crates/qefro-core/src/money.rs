//! Precise decimal money for accounting. Do not use `f64` for ledger math.

use crate::error::{FieldError, QefroError, QefroResult};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

/// Ledger scale matches PostgreSQL `NUMERIC(18,6)`.
pub const MONEY_SCALE: u32 = 6;

pub fn parse_money(value: &Value) -> QefroResult<Decimal> {
    let raw = match value {
        Value::Null => return Ok(Decimal::ZERO),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                return Ok(Decimal::ZERO);
            }
            t.to_string()
        }
        _ => {
            return Err(QefroError::validation(vec![FieldError::new(
                "amount",
                "invalid_type",
                "Amount must be a number",
            )]))
        }
    };
    let parsed = Decimal::from_str(&raw).map_err(|_| {
        QefroError::validation(vec![FieldError::new(
            "amount",
            "invalid_type",
            "Amount must be a decimal number",
        )])
    })?;
    Ok(round_money(parsed))
}

pub fn round_money(value: Decimal) -> Decimal {
    value.round_dp(MONEY_SCALE)
}

pub fn money_to_json(value: Decimal) -> Value {
    let rounded = round_money(value);
    if let Some(f) = rounded.to_f64() {
        return serde_json::json!(f);
    }
    Value::String(rounded.to_string())
}

pub fn money_zero() -> Decimal {
    Decimal::ZERO
}

/// `quantity × unit_price` using decimal arithmetic.
pub fn money_mul_qty(unit: Decimal, qty: i64) -> Decimal {
    round_money(unit * Decimal::from(qty))
}

/// Sum debit and credit columns on journal lines. Does not use floating point.
pub fn sum_debit_credit(lines: &[Value]) -> QefroResult<(Decimal, Decimal)> {
    let mut debit = Decimal::ZERO;
    let mut credit = Decimal::ZERO;
    for (i, line) in lines.iter().enumerate() {
        let d = parse_money(line.get("debit").unwrap_or(&Value::Null))?;
        let c = parse_money(line.get("credit").unwrap_or(&Value::Null))?;
        if d.is_sign_negative() || c.is_sign_negative() {
            return Err(QefroError::validation(vec![FieldError::new(
                format!("lines.{i}.debit"),
                "min_value",
                "Amounts cannot be negative",
            )
            .with_rule("accounting")]));
        }
        if d > Decimal::ZERO && c > Decimal::ZERO {
            return Err(QefroError::validation(vec![FieldError::new(
                format!("lines.{i}.debit"),
                "invalid",
                "A journal line cannot have both debit and credit",
            )
            .with_rule("accounting")]));
        }
        if d == Decimal::ZERO && c == Decimal::ZERO {
            return Err(QefroError::validation(vec![FieldError::new(
                format!("lines.{i}.debit"),
                "required",
                "Each journal line needs a debit or a credit",
            )
            .with_rule("accounting")]));
        }
        debit += d;
        credit += c;
    }
    Ok((round_money(debit), round_money(credit)))
}

pub fn assert_balanced(debit: Decimal, credit: Decimal) -> QefroResult<()> {
    if debit != credit {
        return Err(QefroError::validation(vec![FieldError::new(
            "lines",
            "unbalanced",
            format!("Journal is not balanced (debit {debit} credit {credit})"),
        )
        .with_rule("double_entry")]));
    }
    if debit == Decimal::ZERO {
        return Err(QefroError::validation(vec![FieldError::new(
            "lines",
            "required",
            "Journal must have at least one line",
        )
        .with_rule("double_entry")]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn balanced_lines_use_decimal_not_float() {
        let lines = vec![
            json!({ "debit": "0.10", "credit": 0 }),
            json!({ "debit": "0.20", "credit": 0 }),
            json!({ "debit": 0, "credit": "0.30" }),
        ];
        let (d, c) = sum_debit_credit(&lines).unwrap();
        assert_eq!(d, c);
        assert_balanced(d, c).unwrap();
    }

    #[test]
    fn unbalanced_is_rejected() {
        let lines = vec![
            json!({ "debit": "100.00", "credit": 0 }),
            json!({ "debit": 0, "credit": "90.00" }),
        ];
        let (d, c) = sum_debit_credit(&lines).unwrap();
        let err = assert_balanced(d, c).unwrap_err();
        match err {
            QefroError::Validation { fields, .. } => {
                assert_eq!(fields[0].code, "unbalanced");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn both_debit_and_credit_rejected() {
        let err = sum_debit_credit(&[json!({ "debit": "10", "credit": "10" })]).unwrap_err();
        match err {
            QefroError::Validation { fields, .. } => assert_eq!(fields[0].code, "invalid"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn summing_many_lines_is_cheap() {
        let line = json!({ "debit": "1.25", "credit": 0 });
        let credit = json!({ "debit": 0, "credit": "1.25" });
        let mut lines = Vec::new();
        for _ in 0..500 {
            lines.push(line.clone());
            lines.push(credit.clone());
        }
        let (d, c) = sum_debit_credit(&lines).unwrap();
        assert_eq!(d, c);
    }

    #[test]
    fn quantity_times_unit_price_is_decimal() {
        let unit = parse_money(&json!("20.00")).unwrap();
        assert_eq!(
            money_mul_qty(unit, 2),
            parse_money(&json!("40.00")).unwrap()
        );
        let discount = parse_money(&json!("5")).unwrap();
        let tax = parse_money(&json!("3.50")).unwrap();
        let total = round_money(money_mul_qty(unit, 2) - discount + tax);
        assert_eq!(total, parse_money(&json!("38.50")).unwrap());
    }

    #[test]
    fn summing_ten_thousand_lines_stays_exact() {
        let line = json!({ "debit": "0.10", "credit": 0 });
        let credit = json!({ "debit": 0, "credit": "0.10" });
        let mut lines = Vec::new();
        for _ in 0..10_000 {
            lines.push(line.clone());
            lines.push(credit.clone());
        }
        let (d, c) = sum_debit_credit(&lines).unwrap();
        assert_eq!(d, c);
        assert_eq!(d, parse_money(&json!(1000)).unwrap());
    }
}
