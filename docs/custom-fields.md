# Custom fields

Custom fields extend Qefro's business model. They do not replace `EntityDef` or create a second runtime.

```text
EntityDef
   +
Tenant / application custom metadata
   ↓
Effective entity metadata
   ↓
EntityService
   ↓
REST / UI / SDK / Studio / Import / Export
```

## What they are

A custom field is a normal [`FieldDef`](entities.md) with `custom: true`. Validation, permissions, defaults, read-only, hidden, select options, and `visible_when` are the same as core fields.

Application fields are declared in Rust or YAML and committed to Git:

```rust
EntityDef::new("Customer")
    .field(FieldDef::string("name").required())
    .custom_field(
        FieldDef::enum_values("loyalty_tier", vec!["Bronze", "Silver", "Gold"])
            .filterable(),
    )
```

Tenant fields are created in Studio (`entity.custom_field`) and stored in `qefro_custom_fields`. They are merged at request time. They are never written into the process-wide entity overlay (that would leak Tenant A metadata to Tenant B).

## Storage

Every business table has one JSONB bag:

```text
qefro_custom
```

There is no `ADD COLUMN` when a custom field is published. Studio cannot emit SQL, DDL, or credentials.

Writes pack custom values into the bag. Reads unpack them to flat keys so REST looks like any other field:

```json
{
  "name": "Ahmed Khan",
  "loyalty_tier": "Gold"
}
```

The nested alias `{ "custom": { "loyalty_tier": "Gold" } }` is also accepted.

A GIN index on `qefro_custom` supports containment. Equality filters use `qefro_custom->>'name'`. Custom fields are not sortable (no per-key btree indexes). Administrators cannot create unlimited database indexes.

## Namespaces

| Source | Scope | How |
| --- | --- | --- |
| Framework | All apps | `EntityDef::field` in qefro-core |
| Application | That app | `.custom_field()` or `AppModule::extend_entity` |
| Tenant | One tenant | Studio publish → `qefro_custom_fields` |

Reserved names: `id`, `tenant_id`, timestamps, `qefro_custom`, `custom`, identity secrets (`password`, `token`, …), `debit`, `credit`. Collisions with existing `EntityDef` fields are rejected.

Allowed types: string, text, integer, decimal, boolean, date, datetime, time, enum/select. Email, phone, and currency reuse existing `FieldDef` helpers. Relations and child tables are not custom-field types.

## Validation and permissions

`EntityService` applies defaults, `required`, min/max, pattern, and enum options through `validate_record`. Read-only is enforced on the server. Hidden is presentation only — field permissions remain authoritative.

Secret / ephemeral / write-only semantics apply. Custom fields cannot expose passwords or bypass row policy.

## Consumers

Once a field is on the effective entity, existing machinery sees it:

- `GET /api/v1/meta/ui` (schema version stays `"1"`)
- REST create / update / list / filter
- SDK entity operations (no separate custom-field client)
- Generic form, detail, and optional list columns
- Import / export
- Documents `{{ customer.loyalty_tier }}`
- Communication templates
- Automation conditions
- Reports that already bind entity fields (filters yes; GROUP BY/SUM stay on stored columns)

OpenAPI static schemas still describe core columns. Dynamic fields appear in UI metadata.

## Studio

Entities → **custom fields** → **+ Add custom field**. Validate, then publish. Impact is **Safe** (no migration). Disable hides the field; JSONB values are kept. Type changes that cannot represent existing data are rejected. Two admins editing the same field use the `version` column (concurrent publish returns conflict).

Preview uses the real generic form renderer.

## CLI

`qefro inspect Customer` lists **core** and **custom** fields.

`qefro validate` rejects reserved names, collisions, invalid types, invalid defaults, and invalid options.

## Lifecycle

```text
Active → Deprecated → Disabled
```

Disabled fields leave effective metadata. Data is not physically deleted. Rename is a metadata change plus a controlled JSONB key copy — never an arbitrary user script.

## Safety

Custom fields do not become workflow state, authentication fields, ledger accounts, stock quantities, or authoritative prices. Identity, accounting, inventory, and commerce stay on their existing runtimes.

## Restaurant example

Restaurant `Customer` declares `loyalty_tier`, `preferred_table`, and `dietary_notes` as application custom fields. CRM adds `lead_source`, `account_size`, and `customer_segment` the same way. Restaurant also extends framework `Product` with manufacturer / warranty / color via `extend_entity` — still not columns on the core Product definition.
