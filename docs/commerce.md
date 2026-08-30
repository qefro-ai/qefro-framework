# Commerce

Commerce is a Qefro business capability on `EntityDef` / `EntityService`. There is no second ERP, storefront, cart, or commerce REST API.

```
EntityDef → EntityService → Business Operation → Quote / Order / Invoice / Payment / Return
```

REST, the generic UI, SDK (`client.execute`), workflow, permissions, activity, audit, attachments, events, automation, reports, Studio, and the CLI all use that path.

`UI_SCHEMA_VERSION` stays `"1"`.

## Lifecycle

```
Quote
  ↓ Send → Sent
  ↓ Accept → Accepted
  ↓ Convert → Sales Order (Converted)
       ↓ Confirm → Confirmed   (inventory reserve hook)
       ↓ Fulfill → shipment(s), possibly Partial
       ↓ Fulfill → Fulfilled   (inventory consume hook)
       ↓ Complete → Completed
       ↓ Cancel (from Confirmed) → Cancelled  (inventory release hook)
            ↓ Issue invoice
                 ↓ Record payment → Paid
```

Returns:

```
Sales Order → Return (Requested → Approve → Receive → Refund)
```

Status is never `PATCH`ed. Named operations own transitions.

## Entities

Platform records (not restaurant `Order` / `Payment`):

| Entity | Slug | Notes |
|---|---|---|
| `Product` | `products` | Sellable item. Restaurant `MenuItem` stays hospitality-specific. |
| `Quote` / `QuoteItem` | `quotes` | Offer. Convert copies customer, lines, and stamped prices. |
| `SalesOrder` / `SalesOrderItem` | `sales-orders` | Generic sales order. Coexists with restaurant `Order`. |
| `Shipment` / `ShipmentItem` | `shipments` | Fulfillment. No carrier APIs. Partial quantities are allowed. |
| `Invoice` / `InvoiceItem` | `invoices` | Issue posts the ledger when account mappings exist. Overdue is derived (Issued + due date), not a workflow state. |
| `SalesPayment` / `PaymentAllocation` | `sales-payments` | Customer payment allocated to invoices. Restaurant `Payment` remains order tender. Label in the UI is **Payment**. |
| `SalesReturn` / `SalesReturnItem` | `sales-returns` | Return against a sales order. Refund emits `payment.refunded`; no payment gateway. |

There is no platform `Customer`. Commerce uses the same polymorphic party pair as Task:

```
customer_type   Customer | CrmCustomer | Person | …
customer_id
customer_name
```

A customer does not require a User login. `Person ≠ User ≠ Customer`. Applications opt in with `EntityDef::with_commerce()`, which adds related lists (Quotes, Sales Orders, Invoices, Payments, Returns) filtered by `customer_type`.

Restaurant `Customer` and CRM `CrmCustomer` both call `with_commerce()`. Restaurant `Order` is dine-in/takeaway and is not replaced. Confirming a restaurant order calls the inventory **reserve** hook; starting preparation calls **consume**; cancel calls **release**. Completing a restaurant order already posts Cash / Sales through Accounting `post_ledger`.

## Operations

All writes for a named operation share **one PostgreSQL transaction**. Idempotency keys are honored. The server is authoritative for price, tenant, warehouse, and account.

| Operation | Does |
|---|---|
| `Quote.send` / `accept` | Stamp line prices from `Product.unit_price`, then transition. |
| `Quote.convert` | Validate lines, create `SalesOrder`, copy items and pricing, link `quote_id`, emit `order.created`. |
| `SalesOrder.confirm` | Stamp prices, inventory reserve hook, `order.confirmed`. |
| `SalesOrder.fulfill` | Create a `Shipment` for remaining or requested quantities. Partial fulfillment leaves the order Confirmed with `fulfillment_status=Partial`. Full remaining qty transitions to Fulfilled. |
| `SalesOrder.complete` / `cancel` | Complete from Fulfilled. Cancel from Confirmed releases the reservation hook. |
| `SalesOrder.issue_invoice` | Create an invoice from order lines and issue it. |
| `Invoice.issue` | Stamp prices, `invoice.issued`, `post_ledger` Dr AR / Cr Sales. |
| `Invoice.record_payment` | Create a `SalesPayment` + allocation, receive it, update `paid_amount`. Issued → Paid only when `paid_amount >= total`. |
| `SalesPayment.receive` | `post_ledger` Dr Cash / Cr AR, apply allocations. |
| `SalesReturn.approve` / `receive` / `refund` | Receive restores inventory (hook). Refund posts Sales / AR reversal when mapped and emits `payment.refunded`. |

Inventory Runtime is not implemented. `inventory_reserve` / `consume` / `release` / `restore` are no-op extension points. Do not increment stock counters from Commerce.

Accounting is not duplicated. Commerce calls `post_ledger`. If tenant account codes are unmapped, posting is skipped and the commerce document still transitions.

## Pricing

Line amount is `quantity × unit_price` with `rust_decimal` (`money_mul_qty`). Header formulas:

```
subtotal = SUM(items.amount)
tax      = ROUND(subtotal * tax_rate / 100, 2)
total    = subtotal + tax - discount
```

Clients may send a unit price. Send / accept / convert / confirm / issue **replace** it from `Product.unit_price`. Disabled products cannot be sold. Never use `f64` for money.

## Events, activity, audit, notifications

Domain events go through the existing outbox after COMMIT:

`quote.created`, `quote.accepted`, `order.created`, `order.confirmed`, `order.fulfilled`, `order.cancelled`, `invoice.issued`, `payment.received`, `return.created`, `return.completed`, plus `payment.refunded` for gateways.

Activity and audit use the existing stores (`who` / `what` / `when` / before / after, correlated with operation and request ids). Notifications are `NotificationDef` + `AutomationDef` (order confirmed, invoice issued, payment received). Delivery is not hardcoded.

Attachments are the existing file runtime (quote PDF, PO, invoice PDF, return photo).

## Search, reports, dashboards

Global search indexes document numbers and customer names (`QT-` / `SO-` / `INV-` / `PAY-` / `RET-`, `Ahmed Khan`) with tenant isolation and RBAC.

Reports (existing `ReportDef`): Sales by Customer, Sales by Product, Orders by Status, Invoices Outstanding (Issued), Payments, Returns.

Dashboard widgets: today's completed sales, open (Confirmed) orders, pending fulfillment, outstanding invoices, received payments, requested returns.

## UI, Studio, CLI, SDK

The generic List / Cards / Kanban / Calendar / Form / Detail / Related / Activity / Attachments / Reports surfaces render these entities. Detail actions come from operation metadata (`Send`, `Accept`, `Convert`, `Confirm`, `Fulfill`, `Cancel`, `Issue`, `Record payment`, `Approve`, `Receive`, `Refund`). Do not add a commerce page.

Studio edits the same metadata (entities, workflow, relations, operations). `qefro inspect SalesOrder` prints fields, relations, workflow, operations, permissions, rules, and a commerce capability line.

```
client.execute("SalesOrder", id, "confirm", {})
```

There is no `client.createOrder()` helper.

## What this is not

Payment gateways, shipping carriers, a tax jurisdiction engine, subscriptions, marketplaces, a cart, an e-commerce storefront, pricing AI, and recommendations are out of scope.
