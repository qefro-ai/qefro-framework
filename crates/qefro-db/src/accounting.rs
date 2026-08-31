//! Journal post / reverse and fiscal period close. Same OperationDef path.

use crate::operation::{OperationCtx, OperationHandler, OperationRegistry};
use async_trait::async_trait;
use qefro_core::{
    assert_balanced, parse_money, sum_debit_credit, tenant_account_code, LedgerPosting,
    OperationDef, QefroError, QefroResult, TenantBusinessConfig, ACCOUNT_ENTITY, JOURNAL_ENTITY,
    JOURNAL_LINE_ENTITY, PERIOD_CLOSED, PERIOD_ENTITY, PERIOD_OPEN,
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

fn post_def() -> OperationDef {
    OperationDef::new("post", JOURNAL_ENTITY)
        .label("Post")
        .description("Post a balanced draft journal. Posted entries cannot be edited.")
        .permission("journal_entry.post")
        .roles(&["Staff", "Manager"])
        .transition("post")
        .event("journal.posted")
        .idempotent()
        .confirm()
        .confirmation_message("Post this journal to the ledger? Posted entries cannot be edited.")
}

fn reverse_def() -> OperationDef {
    OperationDef::new("reverse", JOURNAL_ENTITY)
        .label("Reverse")
        .description("Create a reversing journal and mark this entry reversed.")
        .permission("journal_entry.reverse")
        .roles(&["Manager"])
        .transition("reverse")
        .event("journal.reversed")
        .idempotent()
        .confirm()
        .confirmation_message("Reverse this posted journal? A reversing entry will be created.")
}

fn close_def() -> OperationDef {
    OperationDef::new("close", PERIOD_ENTITY)
        .label("Close period")
        .description("Close a fiscal period. New postings into this period are rejected.")
        .permission("fiscal_period.close")
        .roles(&["Manager"])
        .transition("close")
        .event("period.closed")
        .confirm()
        .confirmation_message("Close this period? New journals cannot be posted into it.")
}

fn reopen_def() -> OperationDef {
    OperationDef::new("reopen", PERIOD_ENTITY)
        .label("Reopen period")
        .description("Reopen a closed fiscal period. Admin only.")
        .permission("fiscal_period.reopen")
        .roles(&["Admin"])
        .transition("reopen")
        .event("period.reopened")
        .confirm()
        .confirmation_message("Reopen this closed period?")
}

pub fn accounting_operation_defs() -> Vec<OperationDef> {
    vec![post_def(), reverse_def(), close_def(), reopen_def()]
}

pub fn register_accounting_operations(operations: &mut OperationRegistry) {
    operations.register(post_def(), Arc::new(PostJournal));
    operations.register(reverse_def(), Arc::new(ReverseJournal));
    operations.register(close_def(), Arc::new(ClosePeriod));
    operations.register(reopen_def(), Arc::new(ReopenPeriod));
}

pub struct PostJournal;
pub struct ReverseJournal;
pub struct ClosePeriod;
pub struct ReopenPeriod;

#[async_trait]
impl OperationHandler for PostJournal {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        prepare_and_validate_post(ctx).await?;
        ctx.apply_transition("post")?;
        ctx.emit("journal.posted", json!({ "entity_id": ctx.record_id()? }));
        ctx.set_message("Journal posted");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ReverseJournal {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let id = ctx.record_id()?;
        let lines = journal_lines(ctx).await?;
        if lines.is_empty() {
            return Err(OperationCtx::fail(
                "empty_journal",
                "Posted journal has no lines to reverse",
            ));
        }
        let mut inverted = Vec::new();
        for line in &lines {
            inverted.push(json!({
                "account_id": line.get("account_id").cloned().unwrap_or(Value::Null),
                "description": line.get("description").cloned().unwrap_or(Value::Null),
                "debit": line.get("credit").cloned().unwrap_or(json!(0)),
                "credit": line.get("debit").cloned().unwrap_or(json!(0)),
            }));
        }
        let reference = ctx
            .record
            .get("reference")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let doc = ctx
            .record
            .get("doc_no")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mut reversal_data = json!({
            "posting_date": ctx.record.get("posting_date"),
            "description": format!("Reversal of {doc}"),
            "reference": reference,
            "currency": ctx.record.get("currency"),
            "reversed_from_id": id,
            "lines": inverted,
        });
        if let Some(obj) = reversal_data.as_object_mut() {
            if let Some(period_id) = ctx.record.get("period_id").cloned() {
                if !period_id.is_null() {
                    obj.insert("period_id".into(), period_id);
                }
            }
        }
        let reversal = ctx.create(JOURNAL_ENTITY, reversal_data).await?;
        let reversal_id = reversal
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::internal("reversal missing id"))?;
        ctx.execute(JOURNAL_ENTITY, reversal_id, "post", json!({}))
            .await?;
        ctx.apply_transition("reverse")?;
        ctx.emit(
            "journal.reversed",
            json!({
                "entity_id": id,
                "reversal_id": reversal_id,
            }),
        );
        ctx.set_message("Journal reversed");
        ctx.set_navigate(JOURNAL_ENTITY, reversal_id);
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ClosePeriod {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        ctx.apply_transition("close")?;
        ctx.emit("period.closed", json!({ "entity_id": ctx.record_id()? }));
        ctx.set_message("Period closed");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ReopenPeriod {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        ctx.apply_transition("reopen")?;
        ctx.emit("period.reopened", json!({ "entity_id": ctx.record_id()? }));
        ctx.set_message("Period reopened");
        Ok(ctx.record.clone())
    }
}

async fn prepare_and_validate_post(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<()> {
    let lines = journal_lines(ctx).await?;
    if lines.is_empty() {
        return Err(QefroError::validation(vec![qefro_core::FieldError::new(
            "lines",
            "required",
            "Journal must have at least one line",
        )
        .with_rule("double_entry")]));
    }
    let (debit, credit) = sum_debit_credit(&lines)?;
    assert_balanced(debit, credit)?;
    validate_accounts(ctx, &lines).await?;
    validate_period(ctx).await?;
    let posting_date = ctx
        .record
        .get("posting_date")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let journal_no = ctx
        .record
        .get("doc_no")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    for line in &lines {
        let Some(id) = line
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        ctx.update(
            JOURNAL_LINE_ENTITY,
            id,
            json!({
                "posted": true,
                "posting_date": posting_date,
                "journal_no": journal_no,
            }),
        )
        .await?;
    }
    Ok(())
}

async fn journal_lines(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Vec<Value>> {
    if let Some(rows) = ctx.record.get("lines").and_then(|v| v.as_array()) {
        if !rows.is_empty() {
            return Ok(rows.clone());
        }
    }
    let id = ctx.record_id()?;
    ctx.list(JOURNAL_LINE_ENTITY, "journal_id", json!(id.to_string()))
        .await
}

async fn validate_accounts(ctx: &mut OperationCtx<'_, '_>, lines: &[Value]) -> QefroResult<()> {
    let mut errors = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(account_id) = line
            .get("account_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            errors.push(
                qefro_core::FieldError::new(
                    format!("lines.{i}.account_id"),
                    "required",
                    "Account is required",
                )
                .with_rule("account"),
            );
            continue;
        };
        let account = match ctx.get(ACCOUNT_ENTITY, account_id).await {
            Ok(acc) => acc,
            Err(_) => {
                errors.push(
                    qefro_core::FieldError::new(
                        format!("lines.{i}.account_id"),
                        "not_found",
                        "Account does not exist in this tenant",
                    )
                    .with_rule("account"),
                );
                continue;
            }
        };
        if account.get("enabled") == Some(&json!(false)) {
            errors.push(
                qefro_core::FieldError::new(
                    format!("lines.{i}.account_id"),
                    "disabled",
                    "Cannot post to a disabled account",
                )
                .with_rule("account"),
            );
        }
        let _ = parse_money(line.get("debit").unwrap_or(&Value::Null))?;
        let _ = parse_money(line.get("credit").unwrap_or(&Value::Null))?;
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(QefroError::validation(errors))
    }
}

async fn validate_period(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<()> {
    let posting_date = ctx
        .record
        .get("posting_date")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let period_id = ctx
        .record
        .get("period_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    if let Some(period_id) = period_id {
        let period = ctx.get(PERIOD_ENTITY, period_id).await?;
        reject_closed_period(&period, &posting_date)?;
        return Ok(());
    }
    let closed = ctx
        .list(PERIOD_ENTITY, "status", json!(PERIOD_CLOSED))
        .await
        .unwrap_or_default();
    for period in closed {
        if date_in_period(&posting_date, &period) {
            return Err(period_closed_error(&period));
        }
    }
    let open = ctx
        .list(PERIOD_ENTITY, "status", json!(PERIOD_OPEN))
        .await
        .unwrap_or_default();
    if let Some(period) = open.iter().find(|p| date_in_period(&posting_date, p)) {
        if let Some(id) = period.get("id").cloned() {
            ctx.set_field("period_id", id);
        }
    }
    Ok(())
}

fn date_in_period(date: &str, period: &Value) -> bool {
    if date.is_empty() {
        return false;
    }
    let start = period
        .get("start_date")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let end = period
        .get("end_date")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    !start.is_empty() && !end.is_empty() && date >= start && date <= end
}

fn reject_closed_period(period: &Value, posting_date: &str) -> QefroResult<()> {
    if period.get("status").and_then(|v| v.as_str()) == Some(PERIOD_CLOSED) {
        return Err(period_closed_error(period));
    }
    if !posting_date.is_empty()
        && (period.get("start_date").and_then(|v| v.as_str()).is_some()
            && period.get("end_date").and_then(|v| v.as_str()).is_some())
        && !date_in_period(posting_date, period)
    {
        return Err(OperationCtx::fail(
            "period_mismatch",
            "Posting date is outside the selected fiscal period",
        ));
    }
    Ok(())
}

fn period_closed_error(period: &Value) -> QefroError {
    let name = period
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| period.get("code").and_then(|v| v.as_str()))
        .unwrap_or("this period");
    OperationCtx::fail(
        "period_closed",
        format!("This journal cannot be posted because {name} is closed."),
    )
}

/// Extension point for sales, payments, and future inventory consumption.
/// Resolves semantic account keys from tenant config. Returns `Ok(None)` when
/// mappings are missing so applications can skip posting.
pub async fn post_ledger(
    ctx: &mut OperationCtx<'_, '_>,
    posting: LedgerPosting,
) -> QefroResult<Option<Value>> {
    let mut lines = Vec::new();
    let business = TenantBusinessConfig {
        cash_account: ctx.auth.cash_account.clone(),
        receivable_account: ctx.auth.receivable_account.clone(),
        payable_account: ctx.auth.payable_account.clone(),
        sales_account: ctx.auth.sales_account.clone(),
        cogs_account: ctx.auth.cogs_account.clone(),
        inventory_account: ctx.auth.inventory_account.clone(),
        ..Default::default()
    };
    for spec in &posting.lines {
        let Some(code) = tenant_account_code(&business, &spec.account_key) else {
            return Ok(None);
        };
        let accounts = ctx.list(ACCOUNT_ENTITY, "code", json!(code)).await?;
        let Some(account) = accounts.first() else {
            return Err(OperationCtx::fail(
                "account_missing",
                format!("Account code {code} is not in this tenant's chart"),
            ));
        };
        lines.push(json!({
            "account_id": account.get("id"),
            "description": spec.description.clone().unwrap_or_else(|| posting.description.clone()),
            "debit": spec.debit.clone(),
            "credit": spec.credit.clone(),
        }));
    }
    if lines.is_empty() {
        return Ok(None);
    }
    let created = ctx
        .create(
            JOURNAL_ENTITY,
            json!({
                "posting_date": posting.posting_date,
                "description": posting.description,
                "reference": posting.reference,
                "currency": ctx.auth.currency,
                "lines": lines,
            }),
        )
        .await?;
    let id = created
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| QefroError::internal("journal missing id"))?;
    let posted = ctx.execute(JOURNAL_ENTITY, id, "post", json!({})).await?;
    Ok(Some(posted))
}
