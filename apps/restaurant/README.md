# Restaurant

Built-in Qefro application. Runtime source: `examples/restaurant`.

Entities, workflows, permissions, and the operations dashboard are registered from that crate. Framework core contains no restaurant business rules.

Ops nav: Reservations, Tables, Orders, Customers. Setup (locations, menu, People, Users) is under **Settings**.

Walk-in Customer: name / email / phone, leave Person empty — no User. Linked guest: Settings → People, then Customer → Person. Customer still stores name/email/phone for unlinked rows. See [identity](../../docs/identity.md).

## Takeaway orders

Orders are one entity. **Type** is Dine-in (default) or Takeaway. Table and reservation show only for dine-in; pickup time shows only for takeaway. No extra nav item.

### Walk-in takeaway

1. Orders → New.
2. Type **Takeaway**. Leave **Pickup at** empty.
3. Add items, save.
4. **Confirm** (kitchen sees Confirmed → Preparing → Ready).
5. When the ticket is ready, **Mark Ready**. Guest collects at the counter; **Complete**.

### Prebooked pickup

1. Orders → New.
2. Type **Takeaway**. Set **Pickup at**.
3. Add items, save.
4. **Schedule Pickup** — status becomes Scheduled (Kanban column, dashboard **Upcoming pickups**).
5. On the day, **Confirm**, then the same kitchen flow. Ready takeaway tickets also count on **Ready for pickup**.

Filter the Orders list by Type. Switch to Kanban to work the kitchen by status.

Dine-in is unchanged: Type Dine-in, pick a table, add items, **Confirm**. Table is required at confirm; reservation is optional.

```bash
qefro app install restaurant
qefro migrate --app restaurant
qefro dev --app restaurant
```
