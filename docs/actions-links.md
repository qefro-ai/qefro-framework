# Actions and links

## Actions

Actions are metadata over existing operations. Discovery is filtered by role; invocation is re-checked in `EntityService::execute`.

```yaml
actions:
  - name: submit
    label: Submit
    operation: submit
  - name: create_invoice
    label: Create Invoice
    operation: create_invoice
    confirmation:
      required: true
      message: Create an invoice from this order?
```

`GET` includes `_actions` for the current user. The generic detail page never hardcodes business buttons. Confirmation is UI-only; the backend still enforces the operation.

## Links

Related lists come from one-to-many relationships when possible. Explicit links fill gaps:

```yaml
links:
  - label: Invoices
    entity: Invoice
    relation: customer
```

`GET` includes `_links` with counts. Each list query uses `EntityService` / repository filters: tenant, app entitlement, RBAC, and field permissions. Links are not unrestricted SQL joins.
