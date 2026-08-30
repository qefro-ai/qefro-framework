//! Quote convert, order fulfill, invoice issue/pay, returns. Same OperationDef path.
//!
//! Inventory Runtime is not implemented; reserve/consume/release/restore are
//! no-op extension points. Accounting uses `post_ledger`.

use crate::operation::{OperationCtx, OperationHandler, OperationRegistry};
use async_trait::async_trait;
use qefro_core::{
    money_mul_qty, parse_money, round_money, LedgerPosting, OperationDef, QefroError, QefroResult,
    ACCOUNT_KEY_CASH, ACCOUNT_KEY_RECEIVABLE, ACCOUNT_KEY_SALES, CUSTOMER_ID_FIELD,
    CUSTOMER_TYPE_FIELD, FULFILL_FULFILLED, FULFILL_PARTIAL, FULFILL_UNFULFILLED, INVOICE_ENTITY,
    INVOICE_ITEM_ENTITY, INVOICE_PAID, ORDER_COMPLETED, ORDER_CONFIRMED, ORDER_FULFILLED,
    PAYMENT_ALLOCATION_ENTITY, PRODUCT_ENTITY, QUOTE_ENTITY, QUOTE_ITEM_ENTITY, SALES_ORDER_ENTITY,
    SALES_ORDER_ITEM_ENTITY, SALES_PAYMENT_ENTITY, SALES_RETURN_ENTITY, SALES_RETURN_ITEM_ENTITY,
    SHIPMENT_ENTITY,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

fn send_quote() -> OperationDef {
    OperationDef::new("send", QUOTE_ENTITY)
        .label("Send")
        .permission("quote.send")
        .roles(&["Staff", "Manager"])
        .transition("send")
        .event("quote.created")
        .idempotent()
}

fn accept_quote() -> OperationDef {
    OperationDef::new("accept", QUOTE_ENTITY)
        .label("Accept")
        .permission("quote.accept")
        .roles(&["Staff", "Manager"])
        .transition("accept")
        .event("quote.accepted")
        .idempotent()
}

fn convert_quote() -> OperationDef {
    OperationDef::new("convert", QUOTE_ENTITY)
        .label("Convert")
        .description("Create a sales order from this accepted quote.")
        .permission("quote.convert")
        .roles(&["Staff", "Manager"])
        .transition("convert")
        .idempotent()
        .confirm()
        .confirmation_message("Convert this quote to a sales order?")
}

fn confirm_order() -> OperationDef {
    OperationDef::new("confirm", SALES_ORDER_ENTITY)
        .label("Confirm")
        .permission("sales_order.confirm")
        .roles(&["Staff", "Manager"])
        .transition("confirm")
        .event("order.confirmed")
        .idempotent()
        .confirm()
        .confirmation_message("Confirm this sales order?")
}

fn fulfill_order() -> OperationDef {
    OperationDef::new("fulfill", SALES_ORDER_ENTITY)
        .label("Fulfill")
        .description("Create a shipment for remaining (or requested) quantities. Partial fulfillment is allowed.")
        .permission("sales_order.fulfill")
        .roles(&["Staff", "Manager"])
        .idempotent()
}

fn complete_order() -> OperationDef {
    OperationDef::new("complete", SALES_ORDER_ENTITY)
        .label("Complete")
        .permission("sales_order.complete")
        .roles(&["Staff", "Manager"])
        .transition("complete")
        .idempotent()
}

fn cancel_order() -> OperationDef {
    OperationDef::new("cancel", SALES_ORDER_ENTITY)
        .label("Cancel")
        .permission("sales_order.cancel")
        .roles(&["Manager"])
        .transition("cancel")
        .event("order.cancelled")
        .idempotent()
        .confirm()
        .confirmation_message("Cancel this sales order?")
}

fn issue_invoice() -> OperationDef {
    OperationDef::new("issue", INVOICE_ENTITY)
        .label("Issue")
        .permission("invoice.issue")
        .roles(&["Staff", "Manager"])
        .transition("issue")
        .event("invoice.issued")
        .idempotent()
        .confirm()
        .confirmation_message("Issue this invoice?")
}

fn issue_invoice_from_order() -> OperationDef {
    OperationDef::new("issue_invoice", SALES_ORDER_ENTITY)
        .label("Issue invoice")
        .permission("sales_order.issue_invoice")
        .roles(&["Staff", "Manager"])
        .idempotent()
}

fn record_payment() -> OperationDef {
    OperationDef::new("record_payment", INVOICE_ENTITY)
        .label("Record payment")
        .permission("invoice.record_payment")
        .roles(&["Staff", "Manager"])
        .idempotent()
}

fn receive_payment() -> OperationDef {
    OperationDef::new("receive", SALES_PAYMENT_ENTITY)
        .label("Receive")
        .permission("sales_payment.receive")
        .roles(&["Staff", "Manager"])
        .transition("receive")
        .event("payment.received")
        .idempotent()
}

fn approve_return() -> OperationDef {
    OperationDef::new("approve", SALES_RETURN_ENTITY)
        .label("Approve")
        .permission("sales_return.approve")
        .roles(&["Manager"])
        .transition("approve")
        .event("return.created")
        .idempotent()
}

fn receive_return() -> OperationDef {
    OperationDef::new("receive", SALES_RETURN_ENTITY)
        .label("Receive")
        .permission("sales_return.receive")
        .roles(&["Staff", "Manager"])
        .transition("receive")
        .idempotent()
}

fn refund_return() -> OperationDef {
    OperationDef::new("refund", SALES_RETURN_ENTITY)
        .label("Refund")
        .permission("sales_return.refund")
        .roles(&["Manager"])
        .transition("refund")
        .event("return.completed")
        .idempotent()
        .confirm()
        .confirmation_message("Refund this return?")
}

fn ship_prepare() -> OperationDef {
    OperationDef::new("prepare", SHIPMENT_ENTITY)
        .label("Prepare")
        .roles(&["Staff", "Manager"])
        .transition("prepare")
        .idempotent()
}

fn ship_ship() -> OperationDef {
    OperationDef::new("ship", SHIPMENT_ENTITY)
        .label("Ship")
        .roles(&["Staff", "Manager"])
        .transition("ship")
        .idempotent()
}

fn ship_deliver() -> OperationDef {
    OperationDef::new("deliver", SHIPMENT_ENTITY)
        .label("Deliver")
        .roles(&["Staff", "Manager"])
        .transition("deliver")
        .idempotent()
}

pub fn commerce_operation_defs() -> Vec<OperationDef> {
    vec![
        send_quote(),
        accept_quote(),
        convert_quote(),
        confirm_order(),
        fulfill_order(),
        complete_order(),
        cancel_order(),
        issue_invoice(),
        issue_invoice_from_order(),
        record_payment(),
        receive_payment(),
        approve_return(),
        receive_return(),
        refund_return(),
        ship_prepare(),
        ship_ship(),
        ship_deliver(),
    ]
}

pub fn register_commerce_operations(operations: &mut OperationRegistry) {
    operations.register(send_quote(), Arc::new(StampAndTransition));
    operations.register(accept_quote(), Arc::new(StampAndTransition));
    operations.register(convert_quote(), Arc::new(ConvertQuote));
    operations.register(confirm_order(), Arc::new(ConfirmOrder));
    operations.register(fulfill_order(), Arc::new(FulfillOrder));
    operations.register(complete_order(), Arc::new(SimpleTransition));
    operations.register(cancel_order(), Arc::new(CancelOrder));
    operations.register(issue_invoice(), Arc::new(IssueInvoice));
    operations.register(issue_invoice_from_order(), Arc::new(IssueInvoiceFromOrder));
    operations.register(record_payment(), Arc::new(RecordPayment));
    operations.register(receive_payment(), Arc::new(ReceivePayment));
    operations.register(approve_return(), Arc::new(SimpleTransition));
    operations.register(receive_return(), Arc::new(ReceiveReturn));
    operations.register(refund_return(), Arc::new(RefundReturn));
    operations.register(ship_prepare(), Arc::new(SimpleTransition));
    operations.register(ship_ship(), Arc::new(ShipShipment));
    operations.register(ship_deliver(), Arc::new(SimpleTransition));
}

pub struct SimpleTransition;
pub struct StampAndTransition;
pub struct ConvertQuote;
pub struct ConfirmOrder;
pub struct FulfillOrder;
pub struct CancelOrder;
pub struct IssueInvoice;
pub struct IssueInvoiceFromOrder;
pub struct RecordPayment;
pub struct ReceivePayment;
pub struct ReceiveReturn;
pub struct RefundReturn;
pub struct ShipShipment;

#[async_trait]
impl OperationHandler for SimpleTransition {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        if let Some(name) = ctx.def.workflow_transition.clone() {
            ctx.apply_transition(&name)?;
        }
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for StampAndTransition {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        stamp_quote_prices(ctx).await?;
        if let Some(name) = ctx.def.workflow_transition.clone() {
            ctx.apply_transition(&name)?;
        }
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ConvertQuote {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        stamp_quote_prices(ctx).await?;
        let lines = child_lines(ctx, QUOTE_ITEM_ENTITY, "quote_id").await?;
        if lines.is_empty() {
            return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                "items",
                "required",
                "Quote must have at least one line",
            )]));
        }
        let mut order_lines = Vec::new();
        for line in &lines {
            order_lines.push(copy_line(
                line,
                &["product_id", "description", "quantity", "unit_price"],
            ));
        }
        let quote_id = ctx.record_id()?;
        let order = ctx
            .create(
                SALES_ORDER_ENTITY,
                json!({
                    "customer_type": ctx.record.get(CUSTOMER_TYPE_FIELD),
                    "customer_id": ctx.record.get(CUSTOMER_ID_FIELD),
                    "customer_name": ctx.record.get("customer_name"),
                    "quote_id": quote_id,
                    "order_date": ctx.record.get("quote_date"),
                    "currency": ctx.record.get("currency"),
                    "tax_rate": ctx.record.get("tax_rate"),
                    "discount": ctx.record.get("discount"),
                    "notes": ctx.record.get("notes"),
                    "items": order_lines,
                }),
            )
            .await?;
        ctx.apply_transition("convert")?;
        ctx.emit(
            "order.created",
            json!({
                "entity_id": order.get("id"),
                "quote_id": quote_id,
            }),
        );
        ctx.set_message("Sales order created from quote");
        if let Some(id) = order
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            ctx.set_navigate(SALES_ORDER_ENTITY, id);
        }
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ConfirmOrder {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        stamp_order_prices(ctx).await?;
        inventory_reserve(ctx, &ctx.record.clone()).await?;
        ctx.apply_transition("confirm")?;
        ctx.emit("order.confirmed", json!({ "entity_id": ctx.record_id()? }));
        ctx.set_message("Order confirmed");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for FulfillOrder {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        if ctx.status() != ORDER_CONFIRMED {
            return Err(OperationCtx::fail(
                "invalid_state",
                "Only confirmed orders can be fulfilled",
            ));
        }
        let lines = child_lines(ctx, SALES_ORDER_ITEM_ENTITY, "order_id").await?;
        if lines.is_empty() {
            return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                "items",
                "required",
                "Order has no lines to fulfill",
            )]));
        }
        let requested = ctx.input.get("items").and_then(|v| v.as_array()).cloned();
        let mut ship_items = Vec::new();
        for line in &lines {
            let id = line.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let qty = line.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
            let done = line
                .get("qty_fulfilled")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let remaining = (qty - done).max(0);
            let take = if let Some(req) = &requested {
                req.iter()
                    .find(|r| r.get("order_item_id").and_then(|v| v.as_str()) == Some(id))
                    .and_then(|r| r.get("quantity").and_then(|v| v.as_i64()))
                    .unwrap_or(0)
            } else {
                remaining
            };
            if take < 0 {
                return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                    "items",
                    "min_value",
                    "Fulfillment quantity cannot be negative",
                )]));
            }
            if take > remaining {
                return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                    "items",
                    "max_value",
                    "Cannot fulfill more than the remaining quantity",
                )]));
            }
            if take == 0 {
                continue;
            }
            ship_items.push(json!({
                "order_item_id": id,
                "product_id": line.get("product_id"),
                "quantity": take,
            }));
        }
        if ship_items.is_empty() {
            return Err(OperationCtx::fail(
                "nothing_to_fulfill",
                "No remaining quantity to fulfill",
            ));
        }
        let warehouse = ctx.input.get("warehouse").cloned().unwrap_or(Value::Null);
        let shipment = ctx
            .create(
                SHIPMENT_ENTITY,
                json!({
                    "order_id": ctx.record_id()?,
                    "warehouse": warehouse,
                    "items": ship_items,
                }),
            )
            .await?;
        let ship_id = shipment
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::internal("shipment missing id"))?;
        ctx.execute(SHIPMENT_ENTITY, ship_id, "prepare", json!({}))
            .await?;
        ctx.execute(SHIPMENT_ENTITY, ship_id, "ship", json!({}))
            .await?;
        inventory_consume(ctx, &ctx.record.clone()).await?;
        refresh_fulfillment(ctx).await?;
        ctx.set_message("Shipment created");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for CancelOrder {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        inventory_release(ctx, &ctx.record.clone()).await?;
        ctx.apply_transition("cancel")?;
        ctx.emit("order.cancelled", json!({ "entity_id": ctx.record_id()? }));
        ctx.set_message("Order cancelled");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for IssueInvoice {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        stamp_child_prices(ctx, INVOICE_ITEM_ENTITY, "invoice_id").await?;
        let total = document_total(ctx, INVOICE_ITEM_ENTITY, "invoice_id").await?;
        if total <= Decimal::ZERO {
            return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                "total",
                "required",
                "Invoice total must be greater than zero",
            )]));
        }
        ctx.apply_transition("issue")?;
        let doc = ctx
            .record
            .get("doc_no")
            .and_then(|v| v.as_str())
            .unwrap_or("invoice")
            .to_string();
        if let Some(journal) = crate::post_ledger(
            ctx,
            LedgerPosting::new(format!("Invoice {doc}"), doc)
                .debit(ACCOUNT_KEY_RECEIVABLE, money_json(total))
                .credit(ACCOUNT_KEY_SALES, money_json(total)),
        )
        .await?
        {
            if let Some(id) = journal.get("id").cloned() {
                ctx.set_field("journal_id", id);
            }
        }
        ctx.emit("invoice.issued", json!({ "entity_id": ctx.record_id()? }));
        ctx.set_message("Invoice issued");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for IssueInvoiceFromOrder {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        if !matches!(
            ctx.status(),
            ORDER_CONFIRMED | ORDER_FULFILLED | ORDER_COMPLETED
        ) {
            return Err(OperationCtx::fail(
                "invalid_state",
                "Invoice can be issued from a confirmed, fulfilled, or completed order",
            ));
        }
        let existing = ctx
            .list(
                INVOICE_ENTITY,
                "order_id",
                json!(ctx.record_id()?.to_string()),
            )
            .await?;
        if let Some(found) = existing.into_iter().next() {
            ctx.set_message("Invoice already exists for this order");
            if let Some(id) = found
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                ctx.set_navigate(INVOICE_ENTITY, id);
            }
            return Ok(ctx.record.clone());
        }
        let lines = child_lines(ctx, SALES_ORDER_ITEM_ENTITY, "order_id").await?;
        let mut items = Vec::new();
        for line in &lines {
            items.push(copy_line(
                line,
                &["product_id", "description", "quantity", "unit_price"],
            ));
        }
        let created = ctx
            .create(
                INVOICE_ENTITY,
                json!({
                    "customer_type": ctx.record.get(CUSTOMER_TYPE_FIELD),
                    "customer_id": ctx.record.get(CUSTOMER_ID_FIELD),
                    "customer_name": ctx.record.get("customer_name"),
                    "order_id": ctx.record_id()?,
                    "currency": ctx.record.get("currency"),
                    "tax_rate": ctx.record.get("tax_rate"),
                    "discount": ctx.record.get("discount"),
                    "items": items,
                }),
            )
            .await?;
        let id = created
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::internal("invoice missing id"))?;
        ctx.execute(INVOICE_ENTITY, id, "issue", json!({})).await?;
        ctx.set_navigate(INVOICE_ENTITY, id);
        ctx.set_message("Invoice issued");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for RecordPayment {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        if ctx.status() != "Issued" && ctx.status() != INVOICE_PAID {
            return Err(OperationCtx::fail(
                "invalid_state",
                "Payments can only be recorded against an issued invoice",
            ));
        }
        let invoice_total = document_total(ctx, INVOICE_ITEM_ENTITY, "invoice_id").await?;
        let already = parse_money(ctx.record.get("paid_amount").unwrap_or(&json!(0)))?;
        let remaining = round_money(invoice_total - already);
        let amount = if let Some(v) = ctx.input.get("amount") {
            parse_money(v)?
        } else {
            remaining
        };
        if amount <= Decimal::ZERO {
            return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                "amount",
                "min_value",
                "Payment amount must be greater than zero",
            )]));
        }
        if amount > remaining {
            return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                "amount",
                "max_value",
                "Payment cannot exceed the outstanding balance",
            )]));
        }
        let method = ctx
            .input
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("Cash");
        let payment = ctx
            .create(
                SALES_PAYMENT_ENTITY,
                json!({
                    "customer_type": ctx.record.get(CUSTOMER_TYPE_FIELD),
                    "customer_id": ctx.record.get(CUSTOMER_ID_FIELD),
                    "customer_name": ctx.record.get("customer_name"),
                    "amount": money_json(amount),
                    "currency": ctx.record.get("currency"),
                    "method": method,
                    "allocations": [{
                        "invoice_id": ctx.record_id()?,
                        "amount": money_json(amount),
                    }],
                }),
            )
            .await?;
        let pay_id = payment
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::internal("payment missing id"))?;
        ctx.execute(SALES_PAYMENT_ENTITY, pay_id, "receive", json!({}))
            .await?;
        let fresh = ctx.get(INVOICE_ENTITY, ctx.record_id()?).await?;
        let allocated = parse_money(fresh.get("paid_amount").unwrap_or(&json!(0)))?;
        if allocated > already {
            ctx.set_field(
                "paid_amount",
                fresh
                    .get("paid_amount")
                    .cloned()
                    .unwrap_or(money_json(allocated)),
            );
        } else {
            ctx.set_field("paid_amount", money_json(round_money(already + amount)));
        }
        let paid_now = parse_money(ctx.record.get("paid_amount").unwrap_or(&json!(0)))?;
        if paid_now >= invoice_total {
            ctx.apply_transition("record_payment")?;
        }
        ctx.set_message("Payment recorded");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ReceivePayment {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let amount = parse_money(ctx.record.get("amount").unwrap_or(&json!(0)))?;
        let doc = ctx
            .record
            .get("doc_no")
            .and_then(|v| v.as_str())
            .unwrap_or("payment")
            .to_string();
        if let Some(journal) = crate::post_ledger(
            ctx,
            LedgerPosting::new(format!("Payment {doc}"), doc)
                .debit(ACCOUNT_KEY_CASH, money_json(amount))
                .credit(ACCOUNT_KEY_RECEIVABLE, money_json(amount)),
        )
        .await?
        {
            if let Some(id) = journal.get("id").cloned() {
                ctx.set_field("journal_id", id);
            }
        }
        ctx.apply_transition("receive")?;
        apply_payment_allocations(ctx).await?;
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ReceiveReturn {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        inventory_restore(ctx, &ctx.record.clone()).await?;
        ctx.apply_transition("receive")?;
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for RefundReturn {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let lines = child_lines(ctx, SALES_RETURN_ITEM_ENTITY, "return_id").await?;
        let mut total = Decimal::ZERO;
        for line in &lines {
            let qty = line.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
            let Some(item_id) = line
                .get("order_item_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                continue;
            };
            let item = ctx.get(SALES_ORDER_ITEM_ENTITY, item_id).await?;
            let price = parse_money(item.get("unit_price").unwrap_or(&json!(0)))?;
            total += money_mul_qty(price, qty);
        }
        let doc = ctx
            .record
            .get("doc_no")
            .and_then(|v| v.as_str())
            .unwrap_or("return")
            .to_string();
        if total > Decimal::ZERO {
            let _ = crate::post_ledger(
                ctx,
                LedgerPosting::new(format!("Return {doc}"), doc.clone())
                    .debit(ACCOUNT_KEY_SALES, money_json(total))
                    .credit(ACCOUNT_KEY_RECEIVABLE, money_json(total)),
            )
            .await?;
        }
        ctx.apply_transition("refund")?;
        ctx.emit(
            "payment.refunded",
            json!({
                "entity_id": ctx.record_id()?,
                "amount": money_json(total),
            }),
        );
        ctx.emit("return.completed", json!({ "entity_id": ctx.record_id()? }));
        ctx.set_message("Return refunded");
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ShipShipment {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        ctx.apply_transition("ship")?;
        ctx.set_field(
            "shipped_at",
            json!(chrono::Utc::now().date_naive().to_string()),
        );
        Ok(ctx.record.clone())
    }
}

async fn apply_payment_allocations(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<()> {
    let allocs = child_lines(ctx, PAYMENT_ALLOCATION_ENTITY, "payment_id").await?;
    for alloc in allocs {
        let Some(invoice_id) = alloc
            .get("invoice_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        let amount = parse_money(alloc.get("amount").unwrap_or(&json!(0)))?;
        if amount <= Decimal::ZERO {
            continue;
        }
        let invoice = ctx.get(INVOICE_ENTITY, invoice_id).await?;
        let status = invoice.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status != "Issued" && status != INVOICE_PAID {
            return Err(OperationCtx::fail(
                "invalid_state",
                "Payments can only be allocated to an issued invoice",
            ));
        }
        let lines = ctx
            .list(
                INVOICE_ITEM_ENTITY,
                "invoice_id",
                json!(invoice_id.to_string()),
            )
            .await?;
        let total = totals_from(&invoice, &lines)?;
        let already = parse_money(invoice.get("paid_amount").unwrap_or(&json!(0)))?;
        let remaining = round_money(total - already);
        if amount > remaining {
            return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                "amount",
                "max_value",
                "Allocation cannot exceed the outstanding invoice balance",
            )]));
        }
        ctx.update(
            INVOICE_ENTITY,
            invoice_id,
            json!({ "paid_amount": money_json(round_money(already + amount)) }),
        )
        .await?;
    }
    Ok(())
}

async fn document_total(
    ctx: &mut OperationCtx<'_, '_>,
    item_entity: &str,
    parent_field: &str,
) -> QefroResult<Decimal> {
    let lines = child_lines(ctx, item_entity, parent_field).await?;
    totals_from(&ctx.record, &lines)
}

fn totals_from(header: &Value, lines: &[Value]) -> QefroResult<Decimal> {
    let mut subtotal = Decimal::ZERO;
    for line in lines {
        let qty = line.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
        let price = parse_money(line.get("unit_price").unwrap_or(&json!(0)))?;
        subtotal += money_mul_qty(price, qty);
    }
    let tax_rate = parse_money(header.get("tax_rate").unwrap_or(&json!(0)))?;
    let discount = parse_money(header.get("discount").unwrap_or(&json!(0)))?;
    let tax = round_money(subtotal * tax_rate / Decimal::from(100));
    Ok(round_money(subtotal + tax - discount))
}

async fn stamp_quote_prices(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<()> {
    stamp_child_prices(ctx, QUOTE_ITEM_ENTITY, "quote_id").await
}

async fn stamp_order_prices(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<()> {
    stamp_child_prices(ctx, SALES_ORDER_ITEM_ENTITY, "order_id").await
}

async fn stamp_child_prices(
    ctx: &mut OperationCtx<'_, '_>,
    entity: &str,
    parent_field: &str,
) -> QefroResult<()> {
    let id = ctx.record_id()?;
    let lines = ctx
        .list(entity, parent_field, json!(id.to_string()))
        .await?;
    for line in lines {
        let Some(line_id) = line
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        let Some(product_id) = line
            .get("product_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        let product = ctx.get(PRODUCT_ENTITY, product_id).await?;
        if product.get("enabled") == Some(&json!(false)) {
            return Err(QefroError::validation(vec![qefro_core::FieldError::new(
                "product_id",
                "disabled",
                "Cannot sell a disabled product",
            )]));
        }
        let price = product.get("unit_price").cloned().unwrap_or(json!(0));
        let desc = line
            .get("description")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| {
                product
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });
        let mut patch = json!({ "unit_price": price });
        if let Some(d) = desc {
            patch["description"] = json!(d);
        }
        ctx.update(entity, line_id, patch).await?;
    }
    Ok(())
}

async fn child_lines(
    ctx: &mut OperationCtx<'_, '_>,
    entity: &str,
    parent_field: &str,
) -> QefroResult<Vec<Value>> {
    let id = ctx.record_id()?;
    ctx.list(entity, parent_field, json!(id.to_string())).await
}

fn copy_line(line: &Value, fields: &[&str]) -> Value {
    let mut out = serde_json::Map::new();
    for f in fields {
        if let Some(v) = line.get(*f) {
            out.insert((*f).into(), v.clone());
        }
    }
    Value::Object(out)
}

async fn refresh_fulfillment(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<()> {
    let lines = ctx
        .list(
            SALES_ORDER_ITEM_ENTITY,
            "order_id",
            json!(ctx.record_id()?.to_string()),
        )
        .await?;
    let mut ordered = 0i64;
    let mut fulfilled = 0i64;
    for line in &lines {
        let qty = line.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
        let mut done = line
            .get("qty_fulfilled")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let id = line
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let requested = ctx.input.get("items").and_then(|v| v.as_array());
        let extra = requested.and_then(|rows| {
            rows.iter()
                .find(|r| {
                    r.get("order_item_id").and_then(|v| v.as_str())
                        == line.get("id").and_then(|v| v.as_str())
                })
                .and_then(|r| r.get("quantity").and_then(|v| v.as_i64()))
        });
        if requested.is_none() {
            done = qty;
        } else if let Some(add) = extra {
            done += add;
        }
        if let Some(id) = id {
            ctx.update(
                SALES_ORDER_ITEM_ENTITY,
                id,
                json!({ "qty_fulfilled": done }),
            )
            .await?;
        }
        ordered += qty;
        fulfilled += done.min(qty);
    }
    let status = if fulfilled <= 0 {
        FULFILL_UNFULFILLED
    } else if fulfilled < ordered {
        FULFILL_PARTIAL
    } else {
        FULFILL_FULFILLED
    };
    ctx.set_field("fulfillment_status", json!(status));
    if status == FULFILL_FULFILLED {
        ctx.apply_transition("fulfill")?;
        ctx.emit("order.fulfilled", json!({ "entity_id": ctx.record_id()? }));
    }
    Ok(())
}

fn money_json(value: Decimal) -> Value {
    json!(value.normalize().to_string())
}

/// Extension point: Inventory Runtime should reserve stock on confirm.
pub async fn inventory_reserve(_ctx: &mut OperationCtx<'_, '_>, _order: &Value) -> QefroResult<()> {
    Ok(())
}

/// Extension point: Inventory Runtime should consume stock on fulfill.
pub async fn inventory_consume(_ctx: &mut OperationCtx<'_, '_>, _order: &Value) -> QefroResult<()> {
    Ok(())
}

/// Extension point: Inventory Runtime should release reservations on cancel.
pub async fn inventory_release(_ctx: &mut OperationCtx<'_, '_>, _order: &Value) -> QefroResult<()> {
    Ok(())
}

/// Extension point: Inventory Runtime should restore stock on return receive.
pub async fn inventory_restore(_ctx: &mut OperationCtx<'_, '_>, _ret: &Value) -> QefroResult<()> {
    Ok(())
}
