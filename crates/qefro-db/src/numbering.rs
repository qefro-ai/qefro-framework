//! Tenant-scoped, concurrency-safe document numbering.

use chrono::{DateTime, Utc};
use qefro_core::{NamingConfig, QefroError, QefroResult};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub fn period_key(pattern: &str, now: DateTime<Utc>) -> String {
    let has_year = pattern.contains("{YYYY}") || pattern.contains("{YY}");
    let has_month = pattern.contains("{MM}");
    if has_year && has_month {
        now.format("%Y-%m").to_string()
    } else if has_year {
        now.format("%Y").to_string()
    } else {
        "_".into()
    }
}

pub fn render_number(pattern: &str, seq: i64, now: DateTime<Utc>) -> String {
    let mut out = pattern.to_string();
    out = out.replace("{YYYY}", &now.format("%Y").to_string());
    out = out.replace("{YY}", &now.format("%y").to_string());
    out = out.replace("{MM}", &now.format("%m").to_string());
    if let Some(start) = out.find("{#") {
        if let Some(rel_end) = out[start..].find('}') {
            let token = &out[start..start + rel_end + 1];
            let width = token.chars().filter(|c| *c == '#').count().max(1);
            let padded = format!("{seq:0width$}");
            out = out.replacen(token, &padded, 1);
        }
    }
    out
}

pub async fn allocate(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    entity: &str,
    naming: &NamingConfig,
    now: DateTime<Utc>,
) -> QefroResult<String> {
    let period = period_key(&naming.pattern, now);
    let seq: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO document_sequences (tenant_id, entity, period, last_value)
        VALUES ($1, $2, $3, 1)
        ON CONFLICT (tenant_id, entity, period)
        DO UPDATE SET last_value = document_sequences.last_value + 1
        RETURNING last_value
        "#,
    )
    .bind(tenant_id)
    .bind(entity)
    .bind(&period)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(render_number(&naming.pattern, seq, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_tokens() {
        let now = DateTime::parse_from_rfc3339("2026-08-15T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            render_number("INV-{YYYY}-{#####}", 1, now),
            "INV-2026-00001"
        );
        assert_eq!(period_key("INV-{YYYY}-{#####}", now), "2026");
        assert_eq!(period_key("ORD-{YYYY}-{MM}-{##}", now), "2026-08");
    }
}
