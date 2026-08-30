# Decision records

## 001. Runtime metadata instead of proc macros (V0.1)

Procedural macros can wait. A builder + serde YAML/JSON keeps entity definitions serializable and loadable from files. A future `entity!` macro can emit the same `EntityDef`.

## 002. Application-layer tenant isolation first

RLS is valuable but easy to misconfigure during schema generation. V0.1 always injects `tenant_id` in SQL and ignores client-supplied tenant fields. RLS can be added as generated policies later.

## 003. Shared EntityService for HTTP and agents

Two transports, one mutation path. This is the only way agent tools cannot bypass RBAC, validation, or workflow.

## 004. In-process events

A trait (`EventBus`) hides the implementation. Redis/NATS can be added without changing `customer.created` publishers.

## 005. Person ≠ User ≠ Customer (1.1 identity foundation)

Identity, authentication, and business records are three types:

```
Person (canonical identity once linked) ≠ User (optional login) ≠ Customer/Patient/Employee (business)
```

Person is a tenant-scoped individual. User wraps the existing `users` / `user_tenants` / JWT session tables. Customer, Patient, and Employee remain app entities that may reference Person via nullable `person_id`. When linked, Person is the source of truth for name/email/phone; business rows keep their own columns for unlinked and legacy data. Qefro does not clone Frappe’s User/Contact/party model, and does not add an Identity API or invitation product. See [identity.md](identity.md).

## 006. Activity ≠ Audit (1.2 business object runtime)

Activity is the business timeline on a record. Audit is the Admin-only security log. Organization is a tenant-scoped identity entity, not a User and not a Customer. Party fields (`party_type`, `person_id`, `organization_id`) are metadata conventions on existing `EntityDef`s. Workflow UI always goes through the transition endpoint. See [business-object-runtime.md](business-object-runtime.md).
