# Widgets

Field **types** and **widgets** stay independent. The renderer looks up `field.widget` in `registerWidget`.

Built-in names:

`text`, `textarea`, `number`, `integer`, `decimal`, `currency`, `percentage`, `date`, `time`, `datetime`, `duration`, `select`, `enum`, `multiselect`, `checkbox`, `boolean`, `switch`, `toggle`, `radio`, `relation`, `color`, `image`, `file`, `rich_text`, `markdown`, `email`, `phone`, `url`, `password`, `status`, `json`, `tags`, `child_table`

Where applicable, widgets honor `label`, `description` / `help`, `placeholder`, `required`, `readonly`, `disabled`, and server field errors. React does not decide validity.

```ts
import { registerWidget } from "./metadata/registry";

registerWidget("signature", SignaturePad);
```

Relation pickers call the existing list/get/create APIs (tenant session, RBAC). Quick create uses `POST /api/v1/{slug}` — the same EntityService path as the full form.

See [UI widgets](ui-widgets.md) for type/widget pairing.
