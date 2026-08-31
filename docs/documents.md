# Documents

Document behavior is metadata on an entity. Print and PDF rendering sit on the same EntityDef / EntityService path as CRUD, workflow, permissions, activity, and audit. There is no second document engine.

```text
EntityDef
   ↓
EntityService
   ↓
Document definition (PrintFormat)
   ↓
Template renderer
   ↓
PDF / Print
   ↓
Attachment
```

## Lifecycle

Typical states come from the entity workflow (for example Draft → Submitted / Cancelled). They are not hardcoded into every entity.

When the current status is in `lock_states`, PATCH of ordinary fields is rejected. Fields with `allow_on_submit: true` remain writable. See [Allow on submit](allow-on-submit.md). Changes to locked fields otherwise go through operations (`submit`, `cancel`, `duplicate`, `amend`, or the app's own confirm/cancel handlers).

If `submit_enabled` / `cancel_enabled` / `duplicate_enabled` are set and the app did not register those operations, Qefro registers generic handlers that call the workflow engine.

## Print formats

A `PrintFormat` is presentation of the existing business model. It does not duplicate fields.

```rust
.print_format(
    PrintFormat::new("Invoice", "Invoice")
        .title("Invoice")
        .item_table("items")
        .total_fields(&["subtotal", "tax", "discount", "total"])
        .filename_field("doc_no")
        .section(PrintSection::kind("header"))
        .section(PrintSection::kind("customer").fields(&["customer.name"]))
        .section(PrintSection::kind("items").loop_over("items"))
        .section(PrintSection::kind("totals"))
        .section(PrintSection::kind("footer")),
)
```

YAML under `print_formats/` is equivalent and is suitable for Git review.

Reusable section kinds: `header`, `customer`, `address`, `items`, `totals`, `notes`, `terms`, `footer`, `text`, `image`.

Variants (`default`, `compact`, `professional`) change presentation only. The entity stays the same. `version` is a simple integer (default `1`) so older records remain renderable after a template edit.

## Templates

Safe interpolation only. No JavaScript, Rust, Python, SQL, shell, filesystem, or network.

```text
{{ invoice.number }}
{{ customer.name }}
{{ invoice.total | currency }}
{% for row in items %}{{ row.product }} {{ row.quantity }}x{% endfor %}
{% if discount > 0 %}Discount{% endif %}
```

Filters: `currency`, `date`, `time`, `number`, `percent`. Locale, date format, and currency come from tenant configuration. Missing values render as empty strings — never `null`, `undefined`, or a panic.

Paths resolve against EntityDef fields and relations (`customer` → `customer_id` / `_expanded` / `customer_name`). Nested relations such as `invoice.customer.person.name` only work when those relations exist. Child tables iterate with `{% for %}` / `loop_over` (max 200 rows, nesting depth 8).

Monetary values displayed in documents are the authoritative server-side decimals. Templates do not recalculate ledgers.

## REST

- `GET /api/v1/{slug}/{id}/print` — HTML preview (tenant branding, locale, timezone, currency)
- `GET /api/v1/{slug}/{id}/print.pdf` — server-side Helvetica PDF
- `POST /api/v1/{slug}/{id}/actions/generate_document` — generate PDF and attach it when the entity has attachments

`generate_document` is the existing action pipeline, not a second API. It requires **Read** on the source entity (not Update). Query `?format=` selects a named template variant.

Filenames come from `filename_field` or the entity naming field (`INV-10042.pdf`).

## Permissions and tenant isolation

Generating a document uses the same Read check, field permissions, row policies, and tenant predicate as `GET`. Hidden or unauthorized fields are stripped before the template runs. Tenant A cannot render Tenant B data.

Studio rejects JavaScript, SQL, URLs, filesystem paths, and unknown entity/field/relation paths. `qefro validate` runs the same checks.

## Attachments, activity, audit

`generate_document` stores the PDF through the existing attachment runtime and records one activity event (`Invoice PDF generated`). Audit uses the existing logger. Print/preview GETs do not flood the timeline.

The PDF is an artifact a future email/notification system can consume. This runtime does not implement SMTP.

## Generic UI

When `capabilities.print` is true (the entity has a print format or document definition), Entity Detail shows **Print**, **Download PDF**, and an optional **Attach PDF**. The generic UI discovers this from metadata — it does not hardcode entity names.

## Studio and CLI

Studio → Print Formats edits sections, fields, child tables, and text. Arbitrary HTML/JavaScript is rejected.

```bash
qefro inspect Invoice
# Documents
#   Invoice  default  Invoice

qefro validate --app restaurant
```

## SDK

Use the normal action client:

```ts
await api.action("invoices", id, "generate_document", { format: "Invoice" });
await api.downloadPdf("invoices", id);
```

## Examples

The same runtime renders restaurant Order receipts, CRM Opportunity quotes, commerce Quote / Sales Order / Invoice / Payment receipts, and accounting Journal Entry prints.

## What this is not

Not a Word processor, Google Docs clone, rich-text CMS, e-sign product, SMTP server, OCR pipeline, or tax-compliance engine.
