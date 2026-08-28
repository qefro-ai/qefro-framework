# CRM

Built-in Qefro application. Runtime source: `examples/basic-crm`.

The CRM customer entity is named `CrmCustomer` so it can be installed next to restaurant `Customer`. Optional `person_id` (Identity section) links a known individual. CRM `Contact` is a company contact, not a Person. See [identity](../../docs/identity.md).

```bash
qefro app install crm
qefro dev --app crm
```
