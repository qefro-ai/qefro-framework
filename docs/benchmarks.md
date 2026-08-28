# Benchmarks

Reproducible HTTP measurements for Qefro 1.x. This is a **baseline protocol**, not a marketing comparison. Numbers in this file are from one machine and one dataset. They are not SLAs.

Architecture under test is unchanged: `EntityDef` → `EntityService`; UI → `QefroClient` → REST; agents → `EntityOps` → `EntityService`.

## Harness

```bash
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
qefro migrate --app restaurant
qefro dev --app restaurant   # another terminal
python3 scripts/bench.py --url http://127.0.0.1:8080 --out benches/results
```

`scripts/bench.py` registers a throwaway tenant, then measures:

| Op | HTTP |
| --- | --- |
| create | `POST /api/v1/{slug}` |
| get | `GET /api/v1/{slug}/{id}` |
| list | `GET /api/v1/{slug}?page=1&page_size=25` |
| search | `GET /api/v1/{slug}?search=…` |
| update | `PATCH /api/v1/{slug}/{id}` |
| relation expand | GET after create with a many-to-one field |
| child table | POST parent with nested `items` when the entity has a child table |
| workflow | `POST /api/v1/{slug}/{id}/transition` when `_workflow` is present |
| concurrency | same GET/list at N in-flight requests (1, 10, 50, 100, … up to `--max-concurrency`) |

It records p50 / p95 / p99, RPS, process RSS/CPU (best-effort from `ps`), and environment (OS, Rustc, Postgres, Qefro version, pool settings if advertised by `/ready` or `/metrics`).

Startup is measured separately:

```bash
/usr/bin/time -l qefro migrate --app restaurant   # macOS
# Linux: /usr/bin/time -v
```

Time until `GET /health` returns 200 after `qefro serve` is also in the harness (`--startup` if the server is started by the script; default is attach-to-running).

## Record these fields every run

- CPU, RAM, OS, `rustc -V`, `psql --version`, `qefro --version`
- Dataset size (rows created by the harness, plus any preloaded seed)
- Postgres `max_connections`, Qefro `sqlx` pool (default unless overridden)
- Git commit
- Concurrency levels actually reached (stop early if error rate > 1%)

## Frappe comparison protocol

Do **not** invent Frappe numbers. Equivalent protocol when a Frappe site can run on the same host:

1. Same CPU/RAM/OS; same Postgres major version if possible.
2. Equivalent model: Customer / Company / Contact (or Restaurant reservation + related table) as DocTypes with matching required fields.
3. Same dataset cardinality (N customers, M child rows).
4. Same client: `scripts/bench.py` against Qefro REST; for Frappe use `POST /api/resource/{doctype}` and `GET /api/resource/{doctype}` with a session cookie or token.
5. Report Qefro and Frappe rows **side by side** only when both runs completed. If Frappe cannot run, write `frappe: not executed` and stop.

This repository does not vendor or start Frappe.

## Developer productivity (small CRM-like model)

Measure from Qefro YAML (and estimate Frappe from its usual DocType JSON + Python controller + permissions + workspace):

| Metric | How to count (Qefro) | Frappe (protocol) |
| --- | --- | --- |
| Files touched | entity YAML + permissions + `app.toml` + optional workflow | DocType JSON, `{doctype}.py`, `hooks.py`, workspace, permissions |
| LOC | `wc -l` on those files | same on Frappe artifacts |
| Manual UI | none — generic List/Form/Detail from `GET /meta/ui` | Desk form is generated; custom Client Scripts extra |
| API endpoints | generated `GET/POST/PATCH/DELETE /api/v1/{slug}` | `/api/resource/{doctype}` |
| Time-to-CRUD | clock `qefro app new` → `migrate` → `dev` → first POST | clock `bench new-site` → DocType → first POST |

A worked Qefro count (this repo, no Frappe run):

| App | Files | LOC (`wc -l`) | Manual UI | API |
| --- | --- | --- | --- | --- |
| `qefro app new` skeleton (Customer) | 3 (`app.toml`, `entities/customer.yaml`, `permissions/staff.yaml`) plus generated README | ~40 in those three | none | generated `/api/v1/customers` |
| YAML helpdesk (`apps/helpdesk`) | 10 | 169 | none | generated per entity slug |
| YAML inventory (`apps/inventory`) | 11 | 204 | none | generated per entity slug |

Company + Contact from [creating-an-app.md](creating-an-app.md) is two extra entity YAML files, extra permission rows, and an optional `workflows/company.yaml`. Time-to-CRUD is `validate` → `install` → `migrate` → `dev` plus one `POST /auth/register`.

Frappe: not executed here. Typical equivalent is one DocType JSON per entity, optional `{doctype}.py`, permissions, and a workspace shortcut. Desk forms are generated; custom Client Scripts are extra. Do not treat the Qefro counts as a Frappe-beating claim.

## Results

Machine-specific JSON from `scripts/bench.py` belongs under `benches/results/` (gitignored). Do not copy these numbers into README as product claims.

**Baseline (2026-08-28), not an SLA.** Apple M1, 8 GiB RAM, macOS 27.0 arm64, `rustc 1.95.0`, PostgreSQL 16.14 (Homebrew), Qefro runtime **1.0.2** (`GET /health`), debug `qefro dev --app restaurant`, dataset 40 Customer rows created by the harness. Frappe: **not executed**. Concurrency above 100 was not run (8 GiB host). sqlx pool: runtime default.

| Op | n | errors | p50 ms | p95 ms | p99 ms | notes |
| --- | --- | --- | --- | --- | --- | --- |
| create | 40 | 0 | 10.1 | 12.6 | 18.7 | sequential |
| get | 40 | 0 | 5.7 | 7.3 | 7.4 | sequential |
| list | 40 | 0 | 4.5 | 7.3 | 10.4 | sequential |
| search | 40 | 0 | 3.1 | 5.6 | 6.2 | sequential |
| update | 20 | 0 | 11.7 | 14.1 | 14.7 | sequential |
| list c=1 | 100 | 0 | 4.0 | 5.9 | 6.4 | ~224 wall RPS |
| list c=10 | 100 | 0 | 14.9 | 17.6 | 20.1 | ~641 wall RPS |
| list c=50 | 100 | 0 | 73.7 | 79.3 | 79.6 | ~621 wall RPS |
| list c=100 | 100 | 0 | 70.7 | 128.4 | 133.4 | ~597 wall RPS |

Server RSS about 32 MiB after the run. Sequential create is the slowest single-op path (~10 ms p50); list/search stay in the single-digit milliseconds until concurrency raises tail latency. No extra index or pool change was made — this is a baseline, not a tuned peak.

Relation expand was absent on Customer (one-to-many shows as `_related`). Workflow transition was absent on Customer. Child-table POST was not exercised on this entity.
