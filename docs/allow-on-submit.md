# Allow on submit

When a document status is in `lock_states`, most fields cannot be PATCHed. Fields marked `allow_on_submit` remain writable.

```yaml
document:
  lock_states:
    - Submitted
    - Approved
fields:
  delivery_note:
    allow_on_submit: true
```

Lock states come from document metadata, not a hardcoded `"Submitted"` string. The workflow engine still owns transitions.

The server rejects locked fields with a structured validation error (`locked`). The generic form marks those widgets readonly so the UI matches the API.

Lifecycle operations (`submit`, `cancel`, named actions) are not PATCH and still run through `EntityService::execute`.
