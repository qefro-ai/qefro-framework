//! Declarative document print/preview rendering.
//!
//! HTML is the primary format. PDF is a simple Helvetica text document —
//! no browser runtime and no tenant data leakage. Templates resolve against
//! EntityDef fields and relations; they cannot execute code.

use qefro_core::{
    display_value as tpl_display, render_template, wrap_record, EntityDef, FormatOpts, PrintFormat,
    PrintSection, TenantConfig,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt::Write as _;

const ITEM_COL_ORDER: &[&str] = &[
    "product",
    "description",
    "menu_item_id",
    "account_id",
    "quantity",
    "qty",
    "unit_price",
    "rate",
    "discount",
    "tax",
    "debit",
    "credit",
    "amount",
    "total",
];

const SKIP_ITEM_COLS: &[&str] = &[
    "id",
    "tenant_id",
    "created_at",
    "updated_at",
    "created_by",
    "updated_by",
    "deleted_at",
    "parent_id",
    "sort_order",
    "_expanded",
    "_actions",
    "_permissions",
    "_workflow",
    "_related",
];

pub fn format_opts(config: &TenantConfig) -> FormatOpts {
    FormatOpts {
        currency: config.business.currency.clone(),
        locale: config.business.locale.clone(),
        date_format: config.business.date_format.clone(),
    }
}

pub fn document_filename(format: &PrintFormat, entity: &EntityDef, record: &Value) -> String {
    let field = format
        .filename_field
        .as_deref()
        .or_else(|| entity.naming.as_ref().map(|n| n.field.as_str()))
        .unwrap_or("doc_no");
    let raw = record
        .get(field)
        .or_else(|| record.get("doc_no"))
        .or_else(|| record.get("name"))
        .or_else(|| record.get("code"))
        .or_else(|| record.get("title"))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| entity.slug.clone());
    let stem: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim_matches('-');
    if stem.is_empty() {
        "document.pdf".into()
    } else {
        format!("{stem}.pdf")
    }
}

pub fn print_context(
    entity: &EntityDef,
    format: &PrintFormat,
    record: &Value,
    children: &[Value],
    config: &TenantConfig,
) -> Value {
    let brand = config
        .branding
        .display_name()
        .unwrap_or(entity.label.as_str())
        .to_string();
    let mut extras = HashMap::new();
    extras.insert(
        "branding".into(),
        serde_json::to_value(&config.branding).unwrap_or(json!({})),
    );
    extras.insert("company_name".into(), json!(brand));
    extras.insert("currency".into(), json!(config.business.currency));
    extras.insert("locale".into(), json!(config.business.locale));
    extras.insert("timezone".into(), json!(config.business.timezone));
    let mut rec = record.clone();
    if let Some(obj) = rec.as_object_mut() {
        let table = format
            .item_table
            .clone()
            .or_else(|| {
                entity
                    .fields
                    .iter()
                    .find(|f| f.is_child_table())
                    .map(|f| f.name.clone())
            })
            .unwrap_or_else(|| "items".into());
        if !children.is_empty() {
            obj.entry(table).or_insert_with(|| json!(children));
            obj.entry("items".to_string())
                .or_insert_with(|| json!(children));
        }
        if !obj.contains_key("number") {
            if let Some(v) = obj
                .get("doc_no")
                .cloned()
                .or_else(|| obj.get("name").cloned())
            {
                obj.insert("number".into(), v);
            }
        }
    }
    wrap_record(&entity.name, rec, extras)
}

pub fn render_html(
    entity: &EntityDef,
    format: &PrintFormat,
    record: &Value,
    children: &[Value],
    config: &TenantConfig,
) -> String {
    let opts = format_opts(config);
    let ctx = print_context(entity, format, record, children, config);
    let brand = config
        .branding
        .display_name()
        .unwrap_or(entity.label.as_str());
    let color = config
        .branding
        .primary_color
        .clone()
        .unwrap_or_else(|| "#111827".into());
    let title = format.document_title();
    let compact = format.variant == "compact";
    let mut html = String::new();
    let _ = write!(
        html,
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{}</title>
<style>
@page{{margin:16mm}}
body{{font-family:ui-sans-serif,system-ui,sans-serif;margin:{}px;color:#111;background:#fff;max-width:720px}}
.brand{{display:flex;gap:16px;align-items:flex-start;margin-bottom:16px}}
.brand img{{max-height:48px;max-width:120px}}
h1{{color:{color};margin:0;font-size:{}px;font-weight:600}}
.doc-title{{letter-spacing:.08em;text-transform:uppercase;font-size:12px;color:#6b7280;margin:24px 0 4px}}
.doc-no{{font-size:18px;margin:0 0 16px}}
.meta{{color:#555;margin:4px 0 20px;white-space:pre-line}}
hr{{border:0;border-top:1px solid #e5e7eb;margin:16px 0}}
table{{border-collapse:collapse;width:100%;margin:12px 0}}
th,td{{border-bottom:1px solid #e5e7eb;padding:8px;text-align:left;font-size:13px}}
th{{color:#6b7280;font-weight:600}}
td.num,th.num{{text-align:right}}
.totals{{margin-left:auto;width:280px}}
.section-title{{font-size:11px;letter-spacing:.06em;text-transform:uppercase;color:#6b7280;margin:16px 0 4px}}
.footer{{margin-top:36px;color:#6b7280;font-size:12px;white-space:pre-line}}
.notes{{margin:16px 0;white-space:pre-line}}
</style></head><body>"#,
        escape(&title),
        if compact { 16 } else { 32 },
        if compact { 18 } else { 22 }
    );
    if let Some(body) = &format.body {
        if let Ok(text) = render_template(body, &ctx, &opts) {
            let _ = write!(html, "<pre class=\"notes\">{}</pre>", escape(&text));
        }
        html.push_str("</body></html>");
        return html;
    }
    let sections = effective_sections(format);
    for section in &sections {
        if !section_visible(section, &ctx) {
            continue;
        }
        match section.kind.as_str() {
            "header" => render_header_html(
                &mut html, format, entity, record, config, brand, &title, &ctx, &opts, section,
            ),
            "customer" | "address" => render_fields_html(&mut html, section, &ctx, &opts),
            "items" => {
                let rows = child_rows(format, section, record, children);
                render_items_html(&mut html, entity, &rows);
            }
            "totals" => render_totals_html(&mut html, format, record, &opts),
            "notes" | "terms" | "text" => render_text_html(&mut html, section, &ctx, &opts),
            "footer" => render_footer_html(&mut html, section, brand, config, &ctx, &opts),
            "image" => {}
            _ => render_text_html(&mut html, section, &ctx, &opts),
        }
    }
    html.push_str("</body></html>");
    html
}

fn effective_sections(format: &PrintFormat) -> Vec<PrintSection> {
    if !format.sections.is_empty() {
        return format.sections.clone();
    }
    let mut sections = Vec::new();
    if format.header {
        sections.push(PrintSection::kind("header"));
    }
    sections.push(PrintSection::kind("customer"));
    if format.items {
        sections.push(
            PrintSection::kind("items")
                .loop_over(format.item_table.clone().unwrap_or_else(|| "items".into())),
        );
    }
    if format.totals {
        sections.push(PrintSection::kind("totals"));
    }
    if format.footer {
        sections.push(PrintSection::kind("footer"));
    }
    sections
}

fn section_visible(section: &PrintSection, ctx: &Value) -> bool {
    let Some(when) = &section.show_when else {
        return true;
    };
    let src = format!("{{% if {when} %}}yes{{% endif %}}");
    render_template(&src, ctx, &FormatOpts::default())
        .map(|s| s.contains("yes"))
        .unwrap_or(true)
}

fn render_header_html(
    html: &mut String,
    format: &PrintFormat,
    entity: &EntityDef,
    record: &Value,
    config: &TenantConfig,
    brand: &str,
    title: &str,
    ctx: &Value,
    opts: &FormatOpts,
    section: &PrintSection,
) {
    html.push_str("<div class=\"brand\">");
    if let Some(logo) = config
        .branding
        .logo
        .as_deref()
        .filter(|s| s.starts_with("data:image/"))
    {
        let _ = write!(html, "<img alt=\"\" src=\"{}\">", escape(logo));
    }
    html.push_str("<div>");
    let _ = write!(html, "<h1>{}</h1>", escape(brand));
    let mut addr = Vec::new();
    for key in ["address", "phone", "email", "website"] {
        if let Some(v) = branding_field(&config.branding, key) {
            addr.push(v);
        }
    }
    if !addr.is_empty() {
        let _ = write!(
            html,
            "<div class=\"meta\">{}</div>",
            escape(&addr.join("\n"))
        );
    }
    html.push_str("</div></div>");
    let _ = write!(html, "<div class=\"doc-title\">{}</div>", escape(title));
    let number = record
        .get("doc_no")
        .or_else(|| record.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !number.is_empty() {
        let _ = write!(
            html,
            "<p class=\"doc-no\">{} #{}</p>",
            escape(title),
            escape(number)
        );
    }
    let status = record.get("status").and_then(|v| v.as_str()).unwrap_or("");
    if !status.is_empty() {
        let _ = write!(html, "<div class=\"meta\">Status: {}</div>", escape(status));
    }
    if let Some(text) = &section.text {
        if let Ok(rendered) = render_template(text, ctx, opts) {
            if !rendered.trim().is_empty() {
                let _ = write!(html, "<div class=\"meta\">{}</div>", escape(&rendered));
            }
        }
    }
    let _ = entity;
    let _ = format;
    html.push_str("<hr>");
}

fn branding_field(branding: &qefro_core::TenantBranding, key: &str) -> Option<String> {
    let v = match key {
        "address" => branding.address.as_deref(),
        "phone" => branding.phone.as_deref(),
        "email" => branding.email.as_deref(),
        "website" => branding.website.as_deref(),
        _ => None,
    };
    v.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn render_fields_html(html: &mut String, section: &PrintSection, ctx: &Value, opts: &FormatOpts) {
    if let Some(title) = &section.title {
        let _ = write!(html, "<div class=\"section-title\">{}</div>", escape(title));
    } else if section.kind == "customer" {
        html.push_str("<div class=\"section-title\">Bill To</div>");
    }
    if let Some(text) = &section.text {
        if let Ok(rendered) = render_template(text, ctx, opts) {
            let _ = write!(html, "<div class=\"notes\">{}</div>", escape(&rendered));
        }
        return;
    }
    let fields = if section.fields.is_empty() {
        vec![
            "customer.name".into(),
            "customer_name".into(),
            "customer.person.name".into(),
        ]
    } else {
        section.fields.clone()
    };
    let mut lines = Vec::new();
    for field in fields {
        let expr = format!("{{{{ {field} }}}}");
        if let Ok(v) = render_template(&expr, ctx, opts) {
            let t = v.trim();
            if !t.is_empty() {
                lines.push(t.to_string());
            }
        }
    }
    if !lines.is_empty() {
        let _ = write!(
            html,
            "<div class=\"meta\">{}</div>",
            escape(&lines.join("\n"))
        );
    }
}

fn child_rows<'a>(
    format: &PrintFormat,
    section: &PrintSection,
    record: &'a Value,
    children: &'a [Value],
) -> Vec<Value> {
    let name = section
        .loop_over
        .as_deref()
        .or(format.item_table.as_deref())
        .unwrap_or("items");
    if let Some(rows) = record.get(name).and_then(|v| v.as_array()) {
        if !rows.is_empty() {
            return rows
                .iter()
                .take(qefro_core::template::MAX_LOOP)
                .cloned()
                .collect();
        }
    }
    children
        .iter()
        .take(qefro_core::template::MAX_LOOP)
        .cloned()
        .collect()
}

fn render_items_html(html: &mut String, entity: &EntityDef, children: &[Value]) {
    if children.is_empty() {
        return;
    }
    html.push_str("<table><thead><tr>");
    let cols = item_columns(entity, children);
    for col in &cols {
        let num = is_numeric_col(col);
        let _ = write!(
            html,
            "<th{}>{}</th>",
            if num { " class=\"num\"" } else { "" },
            escape(&humanize(col))
        );
    }
    html.push_str("</tr></thead><tbody>");
    for row in children {
        html.push_str("<tr>");
        for col in &cols {
            let val = cell_value(row, col);
            let num = is_numeric_col(col);
            let _ = write!(
                html,
                "<td{}>{}</td>",
                if num { " class=\"num\"" } else { "" },
                escape(&val)
            );
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table>");
}

fn render_totals_html(html: &mut String, format: &PrintFormat, record: &Value, opts: &FormatOpts) {
    html.push_str("<table class=\"totals\">");
    let totals = if format.total_fields.is_empty() {
        vec![
            "subtotal".into(),
            "tax".into(),
            "discount".into(),
            "grand_total".into(),
            "total".into(),
            "total_debit".into(),
            "total_credit".into(),
            "amount".into(),
        ]
    } else {
        format.total_fields.clone()
    };
    for name in totals {
        if let Some(v) = record.get(&name) {
            if v.is_null() {
                continue;
            }
            let shown = if looks_like_money(&name) {
                let expr = format!("{{{{ {name} | currency }}}}");
                render_template(&expr, record, opts).unwrap_or_else(|_| tpl_display(v))
            } else {
                tpl_display(v)
            };
            if shown.trim().is_empty() {
                continue;
            }
            let _ = write!(
                html,
                "<tr><th>{}</th><td class=\"num\">{}</td></tr>",
                escape(&humanize(&name)),
                escape(&shown)
            );
        }
    }
    html.push_str("</table>");
}

fn render_text_html(html: &mut String, section: &PrintSection, ctx: &Value, opts: &FormatOpts) {
    if let Some(title) = &section.title {
        let _ = write!(html, "<div class=\"section-title\">{}</div>", escape(title));
    }
    if let Some(text) = &section.text {
        if let Ok(rendered) = render_template(text, ctx, opts) {
            if !rendered.trim().is_empty() {
                let _ = write!(html, "<div class=\"notes\">{}</div>", escape(&rendered));
            }
        }
        return;
    }
    for field in &section.fields {
        let expr = format!("{{{{ {field} }}}}");
        if let Ok(v) = render_template(&expr, ctx, opts) {
            if !v.trim().is_empty() {
                let _ = write!(html, "<div class=\"notes\">{}</div>", escape(&v));
            }
        }
    }
}

fn render_footer_html(
    html: &mut String,
    section: &PrintSection,
    brand: &str,
    config: &TenantConfig,
    ctx: &Value,
    opts: &FormatOpts,
) {
    html.push_str("<hr>");
    if let Some(text) = &section.text {
        if let Ok(rendered) = render_template(text, ctx, opts) {
            let _ = write!(html, "<div class=\"footer\">{}</div>", escape(&rendered));
            return;
        }
    }
    let _ = write!(
        html,
        "<div class=\"footer\">{} · {} · {}</div>",
        escape(brand),
        escape(&config.business.timezone),
        escape(&config.business.currency)
    );
}

pub fn render_pdf(title: &str, lines: &[String]) -> Vec<u8> {
    const LINES_PER_PAGE: usize = 42;
    let mut pages: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = vec![title];
    for line in lines {
        if current.len() >= LINES_PER_PAGE {
            pages.push(std::mem::take(&mut current));
        }
        current.push(line.as_str());
    }
    if !current.is_empty() {
        pages.push(current);
    }
    if pages.is_empty() {
        pages.push(vec![title]);
    }
    let n = pages.len();
    let font_id = 3 + n * 2;
    let mut objects: Vec<String> = Vec::new();
    objects.push("1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n".into());
    let kids: String = (0..n)
        .map(|i| format!("{} 0 R", 3 + i * 2))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!(
        "2 0 obj << /Type /Pages /Kids [{kids}] /Count {n} >> endobj\n"
    ));
    for (i, page_lines) in pages.iter().enumerate() {
        let page_id = 3 + i * 2;
        let content_id = page_id + 1;
        objects.push(format!(
            "{page_id} 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content_id} 0 R /Resources << /Font << /F1 {font_id} 0 R >> >> >> endobj\n"
        ));
        let mut content = String::from("BT /F1 11 Tf 50 750 Td ");
        for (li, line) in page_lines.iter().enumerate() {
            if li > 0 {
                content.push_str("0 -16 Td ");
            }
            content.push_str(&pdf_escape(line));
            content.push_str(" Tj ");
        }
        content.push_str("ET");
        let stream = format!(
            "<< /Length {} >>\nstream\n{}\nendstream\n",
            content.len(),
            content
        );
        objects.push(format!("{content_id} 0 obj {stream} endobj\n"));
    }
    objects.push(format!(
        "{font_id} 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj\n"
    ));
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for obj in &objects {
        offsets.push(pdf.len());
        pdf.push_str(obj);
    }
    let xref = pdf.len();
    pdf.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for off in offsets {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
        objects.len() + 1
    ));
    pdf.into_bytes()
}

pub fn pdf_lines(
    entity: &EntityDef,
    format: &PrintFormat,
    record: &Value,
    children: &[Value],
    config: &TenantConfig,
) -> Vec<String> {
    let opts = format_opts(config);
    let ctx = print_context(entity, format, record, children, config);
    if let Some(body) = &format.body {
        if let Ok(text) = render_template(body, &ctx, &opts) {
            return text.lines().map(|s| s.to_string()).collect();
        }
    }
    let brand = config
        .branding
        .display_name()
        .unwrap_or(entity.label.as_str());
    let title = format.document_title();
    let mut lines = Vec::new();
    let sections = effective_sections(format);
    for section in &sections {
        if !section_visible(section, &ctx) {
            continue;
        }
        match section.kind.as_str() {
            "header" => {
                lines.push(brand.to_string());
                for key in ["address", "phone", "email", "website"] {
                    if let Some(v) = branding_field(&config.branding, key) {
                        lines.push(v);
                    }
                }
                lines.push(String::new());
                let number = record
                    .get("doc_no")
                    .or_else(|| record.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if number.is_empty() {
                    lines.push(title.clone());
                } else {
                    lines.push(format!("{title} #{number}"));
                }
                if let Some(status) = record.get("status").and_then(|v| v.as_str()) {
                    if !status.is_empty() {
                        lines.push(format!("Status: {status}"));
                    }
                }
                lines.push("--------------------------------".into());
            }
            "customer" | "address" => {
                if section.kind == "customer" {
                    lines.push("Bill To:".into());
                }
                let fields = if section.fields.is_empty() {
                    vec![
                        "customer.name".into(),
                        "customer_name".into(),
                        "customer.person.name".into(),
                    ]
                } else {
                    section.fields.clone()
                };
                for field in fields {
                    if let Ok(v) = render_template(&format!("{{{{ {field} }}}}"), &ctx, &opts) {
                        if !v.trim().is_empty() {
                            lines.push(v.trim().to_string());
                        }
                    }
                }
                if let Some(text) = &section.text {
                    if let Ok(v) = render_template(text, &ctx, &opts) {
                        for line in v.lines().filter(|s| !s.trim().is_empty()) {
                            lines.push(line.to_string());
                        }
                    }
                }
                lines.push(String::new());
            }
            "items" => {
                let rows = child_rows(format, section, record, children);
                let cols = item_columns(entity, &rows);
                if !cols.is_empty() {
                    lines.push(
                        cols.iter()
                            .map(|c| humanize(c))
                            .collect::<Vec<_>>()
                            .join("  "),
                    );
                }
                for row in &rows {
                    let cells: Vec<String> = cols.iter().map(|c| cell_value(row, c)).collect();
                    if cells.iter().any(|c| !c.is_empty()) {
                        lines.push(cells.join("  "));
                    }
                }
                lines.push("--------------------------------".into());
            }
            "totals" => {
                let totals = if format.total_fields.is_empty() {
                    vec![
                        "subtotal".into(),
                        "tax".into(),
                        "discount".into(),
                        "grand_total".into(),
                        "total".into(),
                        "total_debit".into(),
                        "total_credit".into(),
                        "amount".into(),
                    ]
                } else {
                    format.total_fields.clone()
                };
                for name in totals {
                    if let Some(v) = record.get(&name) {
                        if v.is_null() {
                            continue;
                        }
                        let shown = if looks_like_money(&name) {
                            render_template(&format!("{{{{ {name} | currency }}}}"), record, &opts)
                                .unwrap_or_else(|_| tpl_display(v))
                        } else {
                            tpl_display(v)
                        };
                        if !shown.trim().is_empty() {
                            lines.push(format!("{}  {}", humanize(&name), shown));
                        }
                    }
                }
            }
            "notes" | "terms" | "text" | "footer" => {
                if let Some(text) = &section.text {
                    if let Ok(v) = render_template(text, &ctx, &opts) {
                        for line in v.lines() {
                            lines.push(line.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if lines.is_empty() {
        lines.push(entity.label.clone());
    }
    lines
}

fn item_columns(_entity: &EntityDef, children: &[Value]) -> Vec<String> {
    let Some(row) = children.first().and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut cols = Vec::new();
    for name in ITEM_COL_ORDER {
        if row.contains_key(*name) && !SKIP_ITEM_COLS.contains(name) {
            cols.push((*name).to_string());
        }
    }
    for key in row.keys() {
        if SKIP_ITEM_COLS.contains(&key.as_str())
            || key.ends_with("_id")
                && key != "account_id"
                && key != "menu_item_id"
                && key != "product_id"
        {
            continue;
        }
        if key == "product_id" && !cols.iter().any(|c| c == "product") {
            cols.insert(0, key.clone());
            continue;
        }
        if !cols.iter().any(|c| c == key) {
            cols.push(key.clone());
        }
    }
    cols
}

fn cell_value(row: &Value, col: &str) -> String {
    if let Some(expanded) = row
        .get("_expanded")
        .and_then(|v| v.as_object())
        .and_then(|m| m.get(col).or_else(|| m.get(&format!("{col}"))))
    {
        if let Some(label) = expanded.get("label").and_then(|v| v.as_str()) {
            if !label.is_empty() {
                return label.to_string();
            }
        }
    }
    row.get(col).map(tpl_display).unwrap_or_default()
}

fn is_numeric_col(name: &str) -> bool {
    matches!(
        name,
        "quantity"
            | "qty"
            | "unit_price"
            | "rate"
            | "discount"
            | "tax"
            | "amount"
            | "total"
            | "debit"
            | "credit"
    )
}

fn looks_like_money(name: &str) -> bool {
    matches!(
        name,
        "subtotal"
            | "tax"
            | "discount"
            | "grand_total"
            | "total"
            | "amount"
            | "total_debit"
            | "total_credit"
            | "unit_price"
            | "rate"
    )
}

fn humanize(name: &str) -> String {
    name.replace('_', " ")
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pdf_escape(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    format!(
        "({})",
        cleaned
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use qefro_core::{EntityDef, FieldDef, PrintSection, TenantBranding};

    fn entity() -> EntityDef {
        EntityDef::new("Invoice")
            .field(FieldDef::string("doc_no"))
            .field(FieldDef::currency("total"))
            .build()
    }

    fn config() -> TenantConfig {
        TenantConfig {
            branding: TenantBranding {
                company_name: Some("Qefro Bistro".into()),
                address: Some("123 Main Street".into()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn html_uses_entity_data_and_branding() {
        let format = PrintFormat::new("Invoice", "Invoice")
            .title("Invoice")
            .total_fields(&["total"])
            .section(PrintSection::kind("header"))
            .section(PrintSection::kind("customer").fields(&["customer.name"]))
            .section(PrintSection::kind("items"))
            .section(PrintSection::kind("totals"))
            .section(PrintSection::kind("footer"));
        let record = json!({
            "doc_no": "INV-10042",
            "total": "26.00",
            "status": "PAID",
            "customer": { "name": "Ahmed Khan" }
        });
        let items = vec![
            json!({"product": "Burger", "quantity": 2, "unit_price": "10", "amount": "20"}),
            json!({"product": "Coffee", "quantity": 1, "unit_price": "4", "amount": "4"}),
        ];
        let html = render_html(&entity(), &format, &record, &items, &config());
        assert!(html.contains("INV-10042"));
        assert!(html.contains("Qefro Bistro"));
        assert!(html.contains("123 Main Street"));
        assert!(html.contains("Ahmed Khan"));
        assert!(html.contains("Burger"));
        assert!(html.contains("Coffee"));
        assert!(!html.contains("undefined"));
        assert!(!html.contains("null"));
        let filename = document_filename(&format, &entity(), &record);
        assert_eq!(filename, "INV-10042.pdf");
    }

    #[test]
    fn pdf_paginates_long_item_lists() {
        let items: Vec<String> = (0..80).map(|i| format!("Item {i}")).collect();
        let bytes = render_pdf("Invoice", &items);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("%PDF-1.4"));
        assert!(text.contains("/Count 2") || text.contains("/Count 3"));
    }
}
