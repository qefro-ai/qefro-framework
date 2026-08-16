# UI widgets

Field **data types** and **widgets** are independent. A decimal can render as currency. A string can render as a color picker.

```rust
FieldDef::currency("price")
FieldDef::string("brand_color").ui(UiConfig::color())
FieldDef::date("reservation_date").ui(UiConfig::date())
FieldDef::time("reservation_time").ui(UiConfig::time())
FieldDef::datetime("created_at").ui(UiConfig::datetime().tenant_timezone())
FieldDef::relation("customer", "Customer").required()
.child_table(ChildTableDef::new("items", "OrderItem"))
```

YAML:

```yaml
- name: price
  type: decimal
  ui:
    widget: currency
    widget_options:
      currency: INR
      precision: 2
- name: brand_color
  type: string
  ui:
    widget: color
```

## Registry

The React app resolves `field.ui.widget` through `registerWidget`. Built-in names:

`text`, `textarea`, `number`, `currency`, `percentage`, `date`, `time`, `datetime`, `duration`, `color`, `select`, `multiselect`, `relation`, `checkbox`, `switch`, `radio`, `tags`, `phone`, `url`, `email`, `password`, `rich_text`, `markdown`, `file`, `image`, `json`, `status`, `child_table`

See [Widgets](widgets.md) and [UI 2.0](ui-2.md).

An application can register a custom widget without changing framework core:

```ts
import { registerWidget } from "./metadata/registry";

registerWidget("table-status", TableStatusWidget);
```

Metadata then uses `"widget": "table-status"`. Unknown widgets fall back to `text`.

## Date and time

- `date` stores `YYYY-MM-DD`
- `time` stores `HH:MM` / `HH:MM:SS`
- `datetime` stores UTC RFC3339 (`TIMESTAMPTZ`)
- UI with `timezone: tenant` displays tenant-local time and converts on submit
- Server conversion lives in `qefro_core::timezone` and is tested independently of the browser zone

Currency formatting is display-only. Stored numbers remain authoritative.

## Files and images

`FileUpload` / `ImageUpload` POST to `/api/v1/files`. Objects are stored under `{storage}/{tenant_id}/{key}`. The field stores the storage key, never a local filesystem path. Downloads require the same tenant session.

## Rich text

The editor is intentionally small (bold, italic, underline, headings, lists, links, quotes). HTML is sanitized with `ammonia` before it is stored. Do not render unsanitized client HTML.
