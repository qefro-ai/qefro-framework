# Identity

**Qefro 1.1 identity foundation**

```
Qefro Identity: Person (canonical identity once linked) ≠ User (optional login) ≠ Customer/Patient/Employee (business)
```

| Concept | What it is | Typical table |
| --- | --- | --- |
| **Person** | A real-world individual (name, email, phone). Canonical identity **once linked**. | `people` |
| **User** | An optional login: password, roles, tenant membership, enabled | `users` + `user_tenants` |
| **Customer / Patient / Employee** | A **business** record. It may point at a Person. It is not a User. | app tables |

Do not model Customer as User. Do not copy Frappe’s User / Contact / party model. Authentication stays in `qefro-auth` (email/password, JWT sessions, `user_tenants`). Person and User are `EntityDef`s so the generic UI, REST, and agents all go through `EntityService`.

There is no Identity API, extra auth stack, Organization/Contact product, or invitation product in this increment.

```
UI  →  QefroClient  →  REST  →  EntityService
Agents  →  EntityOps  →  EntityService
```

`POST /api/v1/users` is the existing Admin helper and creates a User through EntityService.

## Convention: `person_id`

Business entities link with a nullable many-to-one named **`person_id`** targeting Person.

When `person_id` is set, **Person is the source of truth for name, email, and phone**. The Customer (or Patient, Employee, CrmCustomer, …) row still stores its own name/email/phone for **unlinked and legacy** records. This increment does not auto-overwrite Customer from Person. The generic UI displays the linked Person (and, if present, the User) via `_expanded` and existing relation widgets.

Unlinked rows (`person_id` null) keep working exactly as before.

The runtime wires an inverse one-to-many on Person for every entity that has `person_id`. Person detail then lists those business relationships in `_related` / `_links`. The generic Related panel consumes that metadata. There is no `if entity === Customer` branch.

```
Customer  --person_id-->  Person  --user_id-->  User (optional)
                ↑
         inverse one-to-many
         (customers, crm_customers, …)
```

Customer (and similar) detail:

- **Person** — many-to-one Open link from `_expanded.person_id`
- **User / account status** — nested `_expanded.person_id._expanded.user_id` with `enabled`, when the Person has a login

## Examples

**Walk-in restaurant guest (no login)**

1. In the restaurant UI, open **Customers** (ops nav). Create a Customer with name, email, and phone. Leave **Person** empty.
2. Or via API: `POST /api/v1/customers` without `person_id`.
3. Reservations reference Customer. No Person and no User row. Customer still has its own name/email/phone.

**Linked restaurant guest (still no login unless they should sign in)**

1. Settings → People (`POST /api/v1/people`) with name / phone.
2. Create or edit a Customer and set `person_id`. The Person field is first on the Customer form (Identity section) and appears as a list column.
3. Reservations still reference Customer. Create a User only if they should sign in.

**CRM company contact (no login)**

CRM `Contact` is a **company** contact on `CrmCustomer`, not a Person. Optionally set `CrmCustomer.person_id` (Identity section / list column) when the account is a known individual. Still no User. CrmCustomer keeps its own name/email/phone columns.

**Patient portal (optional login)**

1. Person for the patient.
2. Patient (clinical record) → `person_id`.
3. Only if they should sign in: create a User (or check **Create login** on the Person form) and set `person.user_id`. Roles are tenant membership roles, not “Patient = User”.

**Employee with HR record and login**

Employee is an HR document. Person holds the individual’s name. User is created only when they need Qefro access. Disable the User to revoke login without deleting the Employee or Person.

## Bootstrap

There is no default admin user and no seeded password.

1. `POST /api/v1/auth/register` with `{ name, email, password, tenant_name, tenant_slug }` creates the tenant and the first **Admin**.
2. That Admin creates further Users (`POST /api/v1/users` or the Users list in Settings) with roles such as Staff or Manager.
3. Set `JWT_SECRET` in production. Never commit it.

Password hashes, session `token_hash`, and write-only `password` never appear on EntityService reads, `/meta/ui`, search, `_related`, `_expanded`, or agent tool payloads.

## Roles and disable

- Role assignment requires **User update**, which is Admin-only (Manager may list/read Users).
- `enabled: false` on a User disables **this tenant’s** membership, revokes that tenant’s sessions, and blocks login to that tenant. Other tenant memberships are unchanged.
- You cannot disable or remove your own membership.

## Optional “create login”

On Person create, the generic form includes a **Create login** checkbox and a password field (`visible_when: create_account`). That path calls the same User create as `POST /api/v1/users`. It requires User **create** permission (Admin). Staff can still create a Person without a login.

## Invitations

V1 does not send invitation email or persist invite rows. Apps can implement `qefro_auth::InvitationSender` and call it from an operation. Until then, create the User directly (and share a password out of band) or register a new tenant. Invitation product is later.

## Later: Organization and Contact

Not in 1.1:

- **Organization** — a tenant-scoped party for companies, practices, or legal entities. Person and Organization would both be “parties”; Customer would reference one or the other.
- **Contact** as a framework entity — CRM already has a company-contact entity. A shared Contact would be an optional link table (Person ↔ Organization), not a replacement for User.

Keep those as documented extension points. Do not grow a full party model until an app needs it.

## Tenant isolation

Person is tenant-owned (`WHERE tenant_id = $1`). User list/get is membership-scoped via `user_tenants`. Cross-tenant reads return **404**. Client `tenant_id` is rejected.
