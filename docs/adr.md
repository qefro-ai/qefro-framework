# Decision records

## 001. Runtime metadata instead of proc macros (V0.1)

Procedural macros can wait. A builder + serde YAML/JSON keeps entity definitions serializable and loadable from files. A future `entity!` macro can emit the same `EntityDef`.

## 002. Application-layer tenant isolation first

RLS is valuable but easy to misconfigure during schema generation. V0.1 always injects `tenant_id` in SQL and ignores client-supplied tenant fields. RLS can be added as generated policies later.

## 003. Shared EntityService for HTTP and agents

Two transports, one mutation path. This is the only way agent tools cannot bypass RBAC, validation, or workflow.

## 004. In-process events

A trait (`EventBus`) hides the implementation. Redis/NATS can be added without changing `customer.created` publishers.
