# Public forms

Tenant-scoped forms that do not require an internal login. Example: `/p/{tenant-slug}/book-table`.

```yaml
public_form:
  enabled: true
  slug: book-table
  fields: [customer_name, phone, reservation_date, guests]
  success_message: We'll contact you shortly.
```

Only listed fields are accepted or returned. `tenant_id` and internal fields are stripped. Tenant is resolved from the public route slug, never from the body.

Submission uses `OpContext::public` with role `Public`, then `EntityService::create`. The visitor is not Admin. Workflow operations that are not granted to Public are rejected.

Rate limiting applies (`public:{ip}:{tenant}:{form}`). Spam-protection hooks can wrap the same limiter later.

After success the UI shows the configured message and a reference (id / numbering field), not the full internal record.
