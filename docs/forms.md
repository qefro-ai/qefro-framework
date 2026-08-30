# Forms

Generic create/edit pages are driven by `GET /api/v1/meta/ui`. There are no per-entity React form components.

```rust
EntityDef::new("Reservation")
    .field(FieldDef::relation("customer", "Customer").required().section("Booking Details"))
    .field(FieldDef::date("reservation_date").required().ui(UiConfig::date()).section("Booking Details"))
    .field(FieldDef::text("notes").section("Additional Information"))
    .build();
```

The renderer:

1. Hides fields with `hidden` or failing `visible_when` (presentation only).
2. Marks fields `readonly` / `readonly_when` (the API still enforces mutations).
3. Groups by `views.form.sections` (tab / section / columns) when present, otherwise `tab` then `section` then `order` / `width`.
4. Resolves each field's `widget` in the widget registry.
5. Shows server `FieldError`s next to the matching input, plus a form-level error count that focuses the field.
6. Warns on unsaved navigation (`Stay` / `Discard`) and marks required fields.
7. Relation fields can Open, Create (full form, then return), search, and clear.

See [UI 2.0](ui-2.md) and [Forms in UI 2.0](ui-2.md).

Dynamic defaults (`default_from`: `current_user`, `current_date`, `current_datetime`, `tenant_timezone`, `tenant_currency`) are applied in `EntityService::create`, not in React.

Client-side checks improve UX. Unique constraints, permissions, workflow, and validation remain server-side.
