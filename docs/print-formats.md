# Print formats

Declarative print layouts — not a drag-and-drop designer. See [Documents](documents.md) for the full document runtime.

```rust
.print_format(
    PrintFormat::new("Order Standard", "Order")
        .title("Receipt")
        .item_table("items")
        .total_fields(&["subtotal", "tax", "discount", "grand_total"])
        .section(PrintSection::kind("header"))
        .section(PrintSection::kind("customer").fields(&["customer.name"]))
        .section(PrintSection::kind("items").loop_over("items"))
        .section(PrintSection::kind("totals"))
        .section(PrintSection::kind("footer")),
)
```

```json
{
  "name": "Invoice",
  "entity": "Invoice",
  "variant": "default",
  "version": 1,
  "header": true,
  "items": true,
  "totals": true,
  "footer": true,
  "sections": [{ "kind": "header" }, { "kind": "items" }, { "kind": "totals" }]
}
```

## Endpoints

- `GET /api/v1/{slug}/{id}/print` — HTML preview (tenant branding, locale, timezone, currency)
- `GET /api/v1/{slug}/{id}/print.pdf` — server-side PDF of the same document
- `GET /api/v1/{slug}/{id}/preview` — alias of print
- `POST /api/v1/{slug}/{id}/actions/generate_document` — PDF + attachment

Rendering uses the already-loaded parent record, permitted relations, and child table. Another tenant's branding or rows cannot appear: GET is tenant-scoped before render.

Templates cannot execute SQL, JavaScript, or reach the filesystem. Missing fields render as empty.
