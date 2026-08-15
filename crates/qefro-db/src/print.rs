//! Declarative document print/preview rendering.
//!
//! HTML is the primary format. PDF is a simple text document using built-in
//! Helvetica — no external renderer and no tenant data leakage.

use qefro_core::{EntityDef, PrintFormat, TenantConfig};
use serde_json::Value;
use std::fmt::Write as _;

pub fn render_html(
    entity: &EntityDef,
    format: &PrintFormat,
    record: &Value,
    children: &[Value],
    config: &TenantConfig,
) -> String {
    let brand = config
        .branding
        .company_name
        .clone()
        .or(config.branding.app_name.clone())
        .unwrap_or_else(|| entity.label.clone());
    let color = config
        .branding
        .primary_color
        .clone()
        .unwrap_or_else(|| "#111827".into());
    let title = format
        .title
        .clone()
        .unwrap_or_else(|| entity.label.clone());
    let doc_no = record
        .get("doc_no")
        .or_else(|| record.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let status = record
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let mut html = String::new();
    let _ = write!(
        html,
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{}</title>
<style>
body{{font-family:ui-sans-serif,system-ui,sans-serif;margin:32px;color:#111}}
h1{{color:{color};margin:0}}
.meta{{color:#555;margin:8px 0 24px}}
table{{border-collapse:collapse;width:100%;margin:16px 0}}
th,td{{border-bottom:1px solid #e5e7eb;padding:8px;text-align:left}}
.totals{{margin-left:auto;width:280px}}
.footer{{margin-top:48px;color:#6b7280;font-size:12px}}
</style></head><body>"#,
        escape(&title)
    );
    if format.header {
        let _ = write!(
            html,
            "<h1>{}</h1><div class=\"meta\"><strong>{}</strong> · {} {} · {}</div>",
            escape(&brand),
            escape(&title),
            escape(doc_no),
            escape(status),
            escape(&config.business.locale)
        );
    }
    if format.items && !children.is_empty() {
        html.push_str("<table><thead><tr>");
        let cols = item_columns(entity, children);
        for col in &cols {
            let _ = write!(html, "<th>{}</th>", escape(col));
        }
        html.push_str("</tr></thead><tbody>");
        for row in children {
            html.push_str("<tr>");
            for col in &cols {
                let val = row.get(col).map(display_value).unwrap_or_default();
                let _ = write!(html, "<td>{}</td>", escape(&val));
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody></table>");
    }
    if format.totals {
        html.push_str("<table class=\"totals\">");
        let totals = if format.total_fields.is_empty() {
            vec![
                "subtotal".into(),
                "tax".into(),
                "discount".into(),
                "grand_total".into(),
                "total".into(),
            ]
        } else {
            format.total_fields.clone()
        };
        for name in totals {
            if let Some(v) = record.get(&name) {
                if v.is_null() {
                    continue;
                }
                let _ = write!(
                    html,
                    "<tr><th>{}</th><td>{}</td></tr>",
                    escape(&humanize(&name)),
                    escape(&display_value(v))
                );
            }
        }
        html.push_str("</table>");
    }
    if format.footer {
        let _ = write!(
            html,
            "<div class=\"footer\">{} · {} · {}</div>",
            escape(&brand),
            escape(&config.business.timezone),
            escape(&config.business.currency)
        );
    }
    html.push_str("</body></html>");
    html
}

pub fn render_pdf(title: &str, lines: &[String]) -> Vec<u8> {
    let mut content = String::from("BT /F1 12 Tf 50 750 Td ");
    content.push_str(&pdf_escape(title));
    content.push_str(" Tj 0 -20 Td ");
    for line in lines.iter().take(40) {
        content.push_str(&pdf_escape(line));
        content.push_str(" Tj 0 -16 Td ");
    }
    content.push_str("ET");
    let stream = format!("<< /Length {} >>\nstream\n{}\nendstream\n", content.len(), content);
    let objects = [
        "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n".to_string(),
        "2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n".to_string(),
        "3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >> endobj\n".to_string(),
        format!("4 0 obj {stream} endobj\n"),
        "5 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj\n".to_string(),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for obj in &objects {
        offsets.push(pdf.len());
        pdf.push_str(obj);
    }
    let xref = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for off in offsets {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
        objects.len() + 1
    ));
    pdf.into_bytes()
}

pub fn pdf_lines(entity: &EntityDef, record: &Value, children: &[Value]) -> Vec<String> {
    let mut lines = vec![entity.label.clone()];
    if let Some(no) = record.get("doc_no").and_then(|v| v.as_str()) {
        lines.push(format!("Number: {no}"));
    }
    for field in entity.business_fields() {
        if field.is_child_table() || field.system {
            continue;
        }
        if let Some(v) = record.get(&field.name) {
            if !v.is_null() && !v.is_object() && !v.is_array() {
                lines.push(format!("{}: {}", field.label, display_value(v)));
            }
        }
    }
    for (i, row) in children.iter().enumerate() {
        lines.push(format!("Item {}: {}", i + 1, display_value(row.get("amount").unwrap_or(&Value::Null))));
    }
    lines
}

fn item_columns(_entity: &EntityDef, children: &[Value]) -> Vec<String> {
    let Some(row) = children.first().and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    row.keys()
        .filter(|k| {
            !matches!(
                k.as_str(),
                "id" | "tenant_id"
                    | "created_at"
                    | "updated_at"
                    | "created_by"
                    | "updated_by"
                    | "deleted_at"
                    | "parent_id"
                    | "order_id"
                    | "opportunity_id"
                    | "showcase_id"
            )
        })
        .cloned()
        .collect()
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn humanize(name: &str) -> String {
    name.replace('_', " ")
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn pdf_escape(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    format!("({})", cleaned.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)"))
}
