# Documents

Document behavior is metadata on an entity. It uses the existing workflow, operation, transaction, audit, and event pipeline. There is no second execution engine.

```rust
EntityDef::new("Invoice")
    .workflow("invoice")
    .document(
        DocumentConfig::new()
            .submit()
            .cancel()
            .duplicate()
            .lock_states(&["Submitted", "Cancelled"])
            .number_on("submit"),
    )
```

## Lifecycle

Typical states come from the entity workflow (for example Draft → Submitted / Cancelled). They are not hardcoded into every entity.

When the current status is in `lock_states`, PATCH of ordinary fields is rejected. Fields with `allow_on_submit: true` remain writable. See [Allow on submit](allow-on-submit.md). Changes to locked fields otherwise go through operations (`submit`, `cancel`, `duplicate`, `amend`, or the app's own confirm/cancel handlers).

If `submit_enabled` / `cancel_enabled` / `duplicate_enabled` are set and the app did not register those operations, Qefro registers generic handlers that call the workflow engine.

## Audit

Lifecycle actions are written with the existing audit logger: user, tenant, document, operation, timestamp.
