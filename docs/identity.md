# Identity

**Qefro 1.1 Person foundation, 1.2 Organization / party**

```
                    Identity
                       │
             ┌─────────┴─────────┐
             ▼                   ▼
           Person            Organization
             │                   │
          optional            optional
             │                   ▼
             ▼                Contacts (app)
            User
             │
             ▼
        Authentication
```

```
Person ≠ User ≠ Organization ≠ Business Entity (Customer / Patient / Employee / Supplier)
```

| Concept | What it is | Typical table |
| --- | --- | --- |
| **Person** | A real-world individual (name, email, phone). Canonical identity **once linked**. | `people` |
| **Organization** | A company / legal entity (name, legal name, email, phone, website, address, logo, enabled). | `organizations` |
| **User** | An optional login: password, roles, tenant membership, enabled | `users` + `user_tenants` |
| **Customer / Patient / Employee / Supplier** | A **business** record. It may point at a Person and/or Organization. It is not a User. | app tables |

Do not make Customer/User or Patient/User equivalent. Do not copy Frappe’s User / Contact / party model. Authentication stays in `qefro-auth`. Person, Organization, and User are `EntityDef`s so the generic UI, REST, and agents all go through `EntityService`.

There is no Identity API, extra auth stack, or invitation product.

```
UI  →  QefroClient  →  REST  →  EntityService
Agents  →  EntityOps  →  EntityService
```

`POST /api/v1/users` is the existing Admin helper and creates a User through EntityService.

## Convention: `person_id` / `organization_id` / `party_type`

Business entities link with nullable many-to-ones:

- **`person_id`** → Person (individual)
- **`organization_id`** → Organization (company)
- optional **`party_type`** = `Person` \| `Organization`

Use `EntityDef::with_party()` to add those fields when missing. This is a metadata convention, not an ERP party table.

When `person_id` is set, **Person is the source of truth for name, email, and phone**. The business row still stores its own name/email/phone for **unlinked and legacy** records. Unlinked rows keep working exactly as before.

The runtime wires inverse one-to-many fields on Person and Organization for every entity that uses the convention. Detail Related panels consume that metadata. There is no `if entity === Customer` branch.

```
Customer  --person_id-->  Person  --user_id-->  User (optional)
          --organization_id-->  Organization
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
