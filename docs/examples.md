# Example applications

Both examples are ordinary Qefro apps. Neither requires changes to framework core. To build your own from scratch, see [Create an application](creating-an-app.md), [App Developer Guide](developer-guide.md), and [Build a fullstack application](fullstack.md).

## Restaurant

Entities: Customer, Restaurant, Branch, DiningTable, MenuCategory, MenuItem, Reservation, Order, OrderItem, Payment. Framework Person and User are under **Settings**, not ops nav (Reservations / Tables / Orders / Customers).

Relationships: Customer → optional Person (`person_id`); Customer → Reservations/Orders; Reservation → Customer + Table; Order → Customer + items + payments.

**Walk-in guest (no login):** create a Customer with name, email, and phone. Leave Person empty. Reservations reference Customer. No User.

**Linked individual:** Settings → People, then set Customer → Person. Customer still stores name/email/phone for unlinked and legacy rows. Create a User only if they should sign in.

Reservation workflow: Pending → Confirmed → Seated → Completed, or Pending/Confirmed → Cancelled.

Order workflow: Draft → Confirmed → Preparing → Ready → Completed, with cancellation from Draft/Confirmed/Preparing.

Dashboard cards (generic UI): today's reservations, available/occupied tables, draft/preparing/ready orders, today's sales.

```bash
qefro app install restaurant
qefro migrate --app restaurant
qefro dev --app restaurant
```

## CRM

Entities: CrmCustomer (labeled Customer), Lead, Contact, Opportunity, Activity. `CrmCustomer.person_id` is optional (Identity section). CRM `Contact` is a company contact, not a Person.

Lead: New → Contacted → Qualified, plus **qualify** and **convert** operations (convert creates a `CrmCustomer`). Opportunity: Open → Qualified → Won/Lost. Activity **complete** sets `done` without a workflow. CRM registers these operations without changing framework core.

```bash
qefro app install crm
qefro dev --app crm
qefro dev --app all
```

## Walkthrough: reservation

1. Register a tenant in the UI or via `POST /api/v1/auth/register`.
2. Create Restaurant → Branch → Table (Settings) → Customer (ops). Walk-in customers skip Person; known guests set Customer → Person.
3. Create Reservation (relation pickers, not UUIDs).
4. Transition Confirm → Seat Customer → Complete (or cancel). Confirm reserves the table; complete frees it. These are server-side operations, not UI-only status changes.
5. `GET /api/v1/tools` then invoke `create_reservation` or `transition_reservation`.
