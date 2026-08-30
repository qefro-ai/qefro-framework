# Accounting

Accounting is a Qefro business capability on `EntityDef` / `EntityService`. There is no second ERP, ledger engine, or accounting API.

```
EntityDef → EntityService → Business Operation → Journal Entry → Journal Lines → Ledger
```

REST, the generic UI, SDK, workflow, permissions, activity, audit, events, automation, and reports all use that path.

`UI_SCHEMA_VERSION` stays `"1"`.

## Chart of accounts

`Account` is a tenant-owned entity:

| Field | Notes |
|---|---|
| `code` | Unique per tenant (for example `1100`) |
| `name` | Searchable |
| `account_type` | `Asset` `Liability` `Equity` `Revenue` `Expense` |
| `parent_id` | Optional self-relation. Not a custom tree store. |
| `enabled` | Disabled accounts cannot be posted to |
| `currency` | Defaults from the tenant |

Never hardcode account IDs in Rust. Tenant business config stores semantic codes:

```text
cash_account
receivable_account
payable_account
sales_account
cogs_account
inventory_account
```

Settings and Studio System edit those codes. `post_ledger` resolves them inside the tenant.

## Journal entries

`JournalEntry` has child table `lines` (`JournalLine`):

```text
account  debit  credit  description
```

Workflow:

```text
Draft → Post → Posted → Reverse → Reversed
```

Status is never PATCHed. Posted and reversed journals are document-locked. Corrections are reversal + a new entry. The reversing entry inverts debit/credit and keeps the original reference.

## Double-entry

Posting is an idempotent business operation (`journal_entry.post`). Before the transition it:

1. Loads lines
2. Sums debit and credit with `rust_decimal` (not `f64`)
3. Rejects unbalanced journals (`code: unbalanced`)
4. Checks accounts exist, belong to this tenant, and are enabled
5. Rejects closed fiscal periods (`code: period_closed`)
6. Stamps `posted` / `posting_date` / `journal_no` on lines
7. Applies the `post` transition in the same SQLx transaction
8. Writes activity, audit, and `journal.posted` through the outbox

A malicious REST client cannot post an unbalanced journal or mutate a posted entry.

## Fiscal periods

`FiscalPeriod` is `Open` or `Closed`. Close is a Manager operation. Reopen is Admin-only. If a closed period covers the posting date, post fails.

## Permissions

Existing `PermissionRegistry`. Staff can draft journals and post. Manager can reverse and close periods. Admin can reopen. There is no separate accounting ACL.

## Reports

Existing `ReportDef` on `JournalLine` (posted lines only):

- Trial Balance — group by account, sum debit/credit
- General Ledger — group by account and date
- Account Balance — group by account

Account detail uses the generic related `journal_lines` list.

## Integration

```rust
post_ledger(ctx, LedgerPosting::new("Order JE-1004", "JE-1004")
    .debit("cash", amount)
    .credit("sales", amount))
    .await?;
```

If tenant mappings are empty, posting is skipped (`Ok(None)`). Inventory consumption can use the same helper (`cogs` / `inventory`) later — this runtime does not implement inventory accounting.

Restaurant Complete Order posts cash/sales when those codes are configured. Restaurant already has Payment; this does not invent Invoice.

## Invariants

- Every posted journal balances
- Posted journals are immutable
- Every journal belongs to one tenant
- Lines reference a valid tenant account
- Closed periods cannot receive postings
- Reversals preserve auditability
- No floating-point ledger math
- No SQL / JS / Rust expressions from metadata
