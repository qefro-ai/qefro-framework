# Restaurant

Built-in Qefro application. Runtime source: `examples/restaurant`.

Entities, workflows, permissions, and the operations dashboard are registered from that crate. Framework core contains no restaurant business rules.

Ops nav: Reservations, Tables, Orders, Customers. Setup (locations, menu, People, Users) is under **Settings**.

Walk-in Customer: name / email / phone, leave Person empty — no User. Linked guest: Settings → People, then Customer → Person. Customer still stores name/email/phone for unlinked rows. See [identity](../../docs/identity.md).

```bash
qefro app install restaurant
qefro migrate --app restaurant
qefro dev --app restaurant
```
