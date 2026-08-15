# Print formats

Declarative print layouts — not a drag-and-drop designer.

```rust
.print_format(
    PrintFormat::new("Order Standard", "Order")
        .title("Order")
        .item_table("items")
        .total_fields(&["subtotal", "tax", "discount", "grand_total"]),
)
```

```json
{
  "name": "Invoice Standard",
  "entity": "Invoice",
  "header": true,
  "items": true,
  "totals": true,
  "footer": true
}
```

## Endpoints

- `GET /api/v1/{slug}/{id}/print` — HTML preview (tenant branding, locale, timezone, currency)
- `GET /api/v1/{slug}/{id}/print.pdf` — simple PDF of the same document
- `GET /api/v1/{slug}/{id}/preview` — alias of print

Rendering uses the already-loaded parent record and child table. Another tenant's branding or rows cannot appear: GET is tenant-scoped before render.
