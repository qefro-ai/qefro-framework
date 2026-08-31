//! Generic commerce primitives: Quote → Sales Order → Fulfillment → Invoice → Payment → Return.
//!
//! These are normal [`EntityDef`] values. Restaurant `Order` / `Payment` stay
//! hospitality-specific. There is no second e-commerce framework.

use crate::app::NavItem;
use crate::automation::{
    AutomationAction, AutomationDef, AutomationStep, AutomationTrigger, NotifyAction,
};
use crate::communication::{
    CommunicationDef, CHANNEL_EMAIL, CHANNEL_IN_APP, CHANNEL_WHATSAPP, PURPOSE_TRANSACTIONAL,
};
use crate::condition::Condition;
use crate::document::{DocumentConfig, NamingConfig, PrintFormat, PrintSection, ReportDef};
use crate::entity::EntityDef;
use crate::field::{ChildTableDef, FieldDef, OnDelete};
use crate::platform::{LinkDef, NotificationDef};
use crate::ui::{
    DashboardCard, DashboardDef, DetailViewSpec, EntityViews, FormViewSpec, ListColumnSpec,
    ListViewSpec, ViewSectionSpec,
};
use serde_json::json;

pub const PRODUCT_ENTITY: &str = "Product";
pub const PRODUCT_SLUG: &str = "products";
pub const QUOTE_ENTITY: &str = "Quote";
pub const QUOTE_SLUG: &str = "quotes";
pub const QUOTE_ITEM_ENTITY: &str = "QuoteItem";
pub const QUOTE_ITEM_SLUG: &str = "quote-items";
pub const SALES_ORDER_ENTITY: &str = "SalesOrder";
pub const SALES_ORDER_SLUG: &str = "sales-orders";
pub const SALES_ORDER_ITEM_ENTITY: &str = "SalesOrderItem";
pub const SALES_ORDER_ITEM_SLUG: &str = "sales-order-items";
pub const SHIPMENT_ENTITY: &str = "Shipment";
pub const SHIPMENT_SLUG: &str = "shipments";
pub const SHIPMENT_ITEM_ENTITY: &str = "ShipmentItem";
pub const SHIPMENT_ITEM_SLUG: &str = "shipment-items";
pub const INVOICE_ENTITY: &str = "Invoice";
pub const INVOICE_SLUG: &str = "invoices";
pub const INVOICE_ITEM_ENTITY: &str = "InvoiceItem";
pub const INVOICE_ITEM_SLUG: &str = "invoice-items";
pub const SALES_PAYMENT_ENTITY: &str = "SalesPayment";
pub const SALES_PAYMENT_SLUG: &str = "sales-payments";
pub const PAYMENT_ALLOCATION_ENTITY: &str = "PaymentAllocation";
pub const PAYMENT_ALLOCATION_SLUG: &str = "payment-allocations";
pub const SALES_RETURN_ENTITY: &str = "SalesReturn";
pub const SALES_RETURN_SLUG: &str = "sales-returns";
pub const SALES_RETURN_ITEM_ENTITY: &str = "SalesReturnItem";
pub const SALES_RETURN_ITEM_SLUG: &str = "sales-return-items";

pub const CUSTOMER_TYPE_FIELD: &str = "customer_type";
pub const CUSTOMER_ID_FIELD: &str = "customer_id";

pub const QUOTE_WORKFLOW: &str = "quote";
pub const SALES_ORDER_WORKFLOW: &str = "sales_order";
pub const SHIPMENT_WORKFLOW: &str = "shipment";
pub const INVOICE_WORKFLOW: &str = "invoice";
pub const PAYMENT_WORKFLOW: &str = "sales_payment";
pub const RETURN_WORKFLOW: &str = "sales_return";

pub const QUOTE_DRAFT: &str = "Draft";
pub const QUOTE_SENT: &str = "Sent";
pub const QUOTE_ACCEPTED: &str = "Accepted";
pub const QUOTE_CONVERTED: &str = "Converted";

pub const ORDER_DRAFT: &str = "Draft";
pub const ORDER_CONFIRMED: &str = "Confirmed";
pub const ORDER_FULFILLED: &str = "Fulfilled";
pub const ORDER_COMPLETED: &str = "Completed";
pub const ORDER_CANCELLED: &str = "Cancelled";

pub const FULFILL_UNFULFILLED: &str = "Unfulfilled";
pub const FULFILL_PARTIAL: &str = "Partial";
pub const FULFILL_FULFILLED: &str = "Fulfilled";

pub const SHIP_PENDING: &str = "Pending";
pub const SHIP_READY: &str = "Ready";
pub const SHIP_SHIPPED: &str = "Shipped";
pub const SHIP_DELIVERED: &str = "Delivered";

pub const INVOICE_DRAFT: &str = "Draft";
pub const INVOICE_ISSUED: &str = "Issued";
pub const INVOICE_PAID: &str = "Paid";

pub const PAY_DRAFT: &str = "Draft";
pub const PAY_RECEIVED: &str = "Received";

pub const RETURN_REQUESTED: &str = "Requested";
pub const RETURN_APPROVED: &str = "Approved";
pub const RETURN_RECEIVED: &str = "Received";
pub const RETURN_REFUNDED: &str = "Refunded";

fn party_fields(section: &'static str) -> [FieldDef; 3] {
    [
        FieldDef::string(CUSTOMER_TYPE_FIELD)
            .nullable()
            .filterable()
            .indexed()
            .label("Customer type")
            .section(section),
        FieldDef::uuid(CUSTOMER_ID_FIELD)
            .nullable()
            .indexed()
            .label("Customer")
            .section(section),
        FieldDef::string("customer_name")
            .nullable()
            .searchable()
            .search_weight(8)
            .label("Customer name")
            .section(section),
    ]
}

fn document_print(name: &str, entity: &str, title: &str) -> PrintFormat {
    PrintFormat::new(name, entity)
        .title(title)
        .item_table("items")
        .total_fields(&["subtotal", "tax", "discount", "total"])
        .filename_field("doc_no")
        .section(PrintSection::kind("header"))
        .section(PrintSection::kind("customer").fields(&["customer_name"]))
        .section(PrintSection::kind("items").loop_over("items"))
        .section(PrintSection::kind("totals"))
        .section(PrintSection::kind("footer"))
}

fn money_header(section: &'static str) -> Vec<FieldDef> {
    vec![
        FieldDef::string("currency")
            .required()
            .default_from("tenant_currency")
            .filterable()
            .section(section),
        FieldDef::decimal("tax_rate")
            .nullable()
            .min(0.0)
            .max(100.0)
            .percentage()
            .default_value(json!(0))
            .label("Tax %")
            .section(section),
        FieldDef::currency("discount")
            .nullable()
            .min(0.0)
            .default_value(json!(0))
            .section(section),
        FieldDef::currency("subtotal")
            .computed("SUM(items.amount)")
            .label("Subtotal")
            .section("Totals"),
        FieldDef::currency("tax")
            .computed("ROUND(subtotal * tax_rate / 100, 2)")
            .section("Totals"),
        FieldDef::currency("total")
            .computed("subtotal + tax - discount")
            .section("Totals"),
    ]
}

fn line_fields(parent_entity: &str, parent_field: &str) -> Vec<FieldDef> {
    vec![
        FieldDef::many_to_one("parent_placeholder", parent_entity)
            .required()
            .hidden()
            .indexed()
            .on_delete(OnDelete::Cascade),
        FieldDef::many_to_one("product_id", PRODUCT_ENTITY)
            .nullable()
            .label("Product")
            .search_related(),
        FieldDef::string("description").nullable().searchable(),
        FieldDef::integer("quantity")
            .required()
            .min(1.0)
            .default_value(json!(1)),
        FieldDef::currency("unit_price")
            .required()
            .min(0.0)
            .label("Rate"),
        FieldDef::currency("amount").computed("quantity * unit_price"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, f)| {
        if i == 0 {
            FieldDef::many_to_one(parent_field, parent_entity)
                .required()
                .hidden()
                .indexed()
                .on_delete(OnDelete::Cascade)
        } else {
            f
        }
    })
    .collect()
}

/// Inverse related lists on Customer / CrmCustomer / Person. Polymorphic, like Task.
pub fn apply_commerce_links(entity: &mut EntityDef) -> bool {
    if matches!(
        entity.name.as_str(),
        PRODUCT_ENTITY
            | QUOTE_ENTITY
            | QUOTE_ITEM_ENTITY
            | SALES_ORDER_ENTITY
            | SALES_ORDER_ITEM_ENTITY
            | SHIPMENT_ENTITY
            | SHIPMENT_ITEM_ENTITY
            | INVOICE_ENTITY
            | INVOICE_ITEM_ENTITY
            | SALES_PAYMENT_ENTITY
            | PAYMENT_ALLOCATION_ENTITY
            | SALES_RETURN_ENTITY
            | SALES_RETURN_ITEM_ENTITY
    ) {
        return false;
    }
    let mut added = false;
    for (label, target, columns) in [
        (
            "Quotes",
            QUOTE_ENTITY,
            &["doc_no", "status", "total"] as &[&str],
        ),
        (
            "Sales Orders",
            SALES_ORDER_ENTITY,
            &["doc_no", "status", "total"],
        ),
        ("Invoices", INVOICE_ENTITY, &["doc_no", "status", "total"]),
        (
            "Payments",
            SALES_PAYMENT_ENTITY,
            &["doc_no", "status", "amount"],
        ),
        ("Returns", SALES_RETURN_ENTITY, &["doc_no", "status"]),
    ] {
        if entity
            .links
            .iter()
            .any(|l| l.entity == target && l.relation == CUSTOMER_ID_FIELD)
        {
            continue;
        }
        entity.links.push(
            LinkDef::new(label, target, CUSTOMER_ID_FIELD)
                .columns(columns)
                .limit(20)
                .filter(CUSTOMER_TYPE_FIELD, &entity.name),
        );
        added = true;
    }
    if added {
        entity.normalize();
    }
    added
}

pub fn product_entity() -> EntityDef {
    EntityDef::new(PRODUCT_ENTITY)
        .label("Product")
        .label_plural("Products")
        .table_name("products")
        .slug_name(PRODUCT_SLUG)
        .icon("box")
        .description("Sellable item. Restaurant MenuItem stays hospitality-specific.")
        .display_field("name")
        .audit()
        .field(
            FieldDef::string("sku")
                .required()
                .unique()
                .searchable()
                .search_weight(10)
                .filterable()
                .max_length(64)
                .section("Product"),
        )
        .field(
            FieldDef::string("name")
                .required()
                .searchable()
                .search_weight(8)
                .filterable()
                .max_length(200)
                .section("Product"),
        )
        .field(
            FieldDef::currency("unit_price")
                .required()
                .min(0.0)
                .label("Price")
                .section("Product"),
        )
        .field(
            FieldDef::string("currency")
                .nullable()
                .default_from("tenant_currency")
                .section("Product"),
        )
        .field(
            FieldDef::boolean("enabled")
                .required()
                .default_value(json!(true))
                .filterable()
                .section("Product"),
        )
        .views(EntityViews {
            default: Some("list".into()),
            list: Some(ListViewSpec {
                columns: vec![col("sku"), col("name"), col("unit_price"), col("enabled")],
                ..Default::default()
            }),
            form: Some(FormViewSpec::sections(vec![ViewSectionSpec::new(
                "Product",
            )
            .fields(&["sku", "name", "unit_price", "currency", "enabled"])])),
            detail: Some(DetailViewSpec::sections(vec![ViewSectionSpec::new(
                "Product",
            )
            .fields(&["sku", "name", "unit_price", "currency", "enabled"])])),
            ..Default::default()
        })
        .build()
}

fn col(field: &str) -> ListColumnSpec {
    ListColumnSpec {
        field: field.into(),
        width: None,
        widget: None,
    }
}

pub fn quote_entity() -> EntityDef {
    let mut b = EntityDef::new(QUOTE_ENTITY)
        .label("Quote")
        .label_plural("Quotes")
        .table_name("quotes")
        .slug_name(QUOTE_SLUG)
        .icon("file")
        .description("Offer to a customer. Convert to a sales order after accept.")
        .workflow(QUOTE_WORKFLOW)
        .display_field("doc_no")
        .audit()
        .attachments()
        .document(
            DocumentConfig::new()
                .submit()
                .duplicate()
                .lock_states(&[QUOTE_CONVERTED]),
        )
        .naming(NamingConfig::new("QT-{YYYY}-{#####}"))
        .print_format(document_print("Quote", QUOTE_ENTITY, "Quote"))
        .field(
            FieldDef::string("doc_no")
                .nullable()
                .searchable()
                .search_weight(10)
                .readonly()
                .label("Number")
                .section("Quote"),
        );
    for f in party_fields("Quote") {
        b = b.field(f);
    }
    b = b
        .field(
            FieldDef::date("quote_date")
                .required()
                .default_from("current_date")
                .filterable()
                .section("Quote"),
        )
        .field(
            FieldDef::enum_values(
                "status",
                vec![QUOTE_DRAFT, QUOTE_SENT, QUOTE_ACCEPTED, QUOTE_CONVERTED],
            )
            .required()
            .default_value(json!(QUOTE_DRAFT))
            .filterable()
            .readonly()
            .section("Quote"),
        )
        .field(
            FieldDef::text("notes")
                .nullable()
                .list(false)
                .section("Quote"),
        )
        .child_table(
            ChildTableDef::new("items", QUOTE_ITEM_ENTITY)
                .parent_field("quote_id")
                .columns(&[
                    "product_id",
                    "description",
                    "quantity",
                    "unit_price",
                    "amount",
                ]),
        );
    for f in money_header("Quote") {
        b = b.field(f);
    }
    b.field(FieldDef::one_to_many(
        "sales_orders",
        SALES_ORDER_ENTITY,
        "quote_id",
    ))
    .views(quote_views())
    .with_tasks()
    .build()
}

fn quote_views() -> EntityViews {
    EntityViews {
        default: Some("list".into()),
        list: Some(ListViewSpec {
            columns: vec![
                col("doc_no"),
                col("customer_name"),
                col("quote_date"),
                col("status"),
                col("total"),
            ],
            ..Default::default()
        }),
        form: Some(FormViewSpec::sections(vec![
            ViewSectionSpec::new("Quote").fields(&[
                "quote_date",
                "customer_type",
                "customer_id",
                "customer_name",
                "currency",
                "tax_rate",
                "discount",
                "status",
                "notes",
            ]),
            ViewSectionSpec::new("Lines").fields(&["items"]),
            ViewSectionSpec::new("Totals").fields(&["subtotal", "tax", "total"]),
        ])),
        detail: Some(DetailViewSpec::sections(vec![
            ViewSectionSpec::new("Quote").fields(&[
                "doc_no",
                "quote_date",
                "customer_name",
                "status",
                "total",
            ]),
            ViewSectionSpec::new("Lines").fields(&["items"]),
            ViewSectionSpec::new("Totals").fields(&["subtotal", "discount", "tax"]),
        ])),
        ..Default::default()
    }
}

pub fn quote_item_entity() -> EntityDef {
    let mut b = EntityDef::new(QUOTE_ITEM_ENTITY)
        .label("Quote Item")
        .label_plural("Quote Items")
        .table_name("quote_items")
        .slug_name(QUOTE_ITEM_SLUG)
        .child_of(QUOTE_ENTITY, "items")
        .display_field("description")
        .no_activity()
        .no_comments();
    for f in line_fields(QUOTE_ENTITY, "quote_id") {
        b = b.field(f);
    }
    b.build()
}

pub fn sales_order_entity() -> EntityDef {
    let mut b = EntityDef::new(SALES_ORDER_ENTITY)
        .label("Sales Order")
        .label_plural("Sales Orders")
        .table_name("sales_orders")
        .slug_name(SALES_ORDER_SLUG)
        .icon("shopping")
        .description(
            "Generic sales order. Restaurant Order remains dine-in/takeaway and is not replaced.",
        )
        .workflow(SALES_ORDER_WORKFLOW)
        .display_field("doc_no")
        .audit()
        .attachments()
        .document(
            DocumentConfig::new()
                .submit()
                .cancel()
                .duplicate()
                .lock_states(&[ORDER_FULFILLED, ORDER_COMPLETED, ORDER_CANCELLED]),
        )
        .naming(NamingConfig::new("SO-{YYYY}-{#####}"))
        .print_format(document_print(
            "Sales Order",
            SALES_ORDER_ENTITY,
            "Sales Order",
        ))
        .field(
            FieldDef::string("doc_no")
                .nullable()
                .searchable()
                .search_weight(10)
                .readonly()
                .label("Number")
                .section("Order"),
        );
    for f in party_fields("Order") {
        b = b.field(f);
    }
    b = b
        .field(
            FieldDef::many_to_one("quote_id", QUOTE_ENTITY)
                .nullable()
                .label("Quote")
                .readonly()
                .on_delete(OnDelete::SetNull)
                .section("Order"),
        )
        .field(
            FieldDef::date("order_date")
                .required()
                .default_from("current_date")
                .filterable()
                .indexed()
                .section("Order"),
        )
        .field(
            FieldDef::enum_values(
                "status",
                vec![
                    ORDER_DRAFT,
                    ORDER_CONFIRMED,
                    ORDER_FULFILLED,
                    ORDER_COMPLETED,
                    ORDER_CANCELLED,
                ],
            )
            .required()
            .default_value(json!(ORDER_DRAFT))
            .filterable()
            .readonly()
            .section("Order"),
        )
        .field(
            FieldDef::enum_values(
                "fulfillment_status",
                vec![FULFILL_UNFULFILLED, FULFILL_PARTIAL, FULFILL_FULFILLED],
            )
            .required()
            .default_value(json!(FULFILL_UNFULFILLED))
            .filterable()
            .server_managed()
            .label("Fulfillment")
            .section("Order"),
        )
        .field(
            FieldDef::text("notes")
                .nullable()
                .list(false)
                .section("Order"),
        )
        .child_table(
            ChildTableDef::new("items", SALES_ORDER_ITEM_ENTITY)
                .parent_field("order_id")
                .columns(&[
                    "product_id",
                    "description",
                    "quantity",
                    "qty_fulfilled",
                    "unit_price",
                    "amount",
                ]),
        );
    for f in money_header("Order") {
        b = b.field(f);
    }
    b.field(FieldDef::one_to_many(
        "shipments",
        SHIPMENT_ENTITY,
        "order_id",
    ))
    .field(FieldDef::one_to_many(
        "invoices",
        INVOICE_ENTITY,
        "order_id",
    ))
    .field(FieldDef::one_to_many(
        "returns",
        SALES_RETURN_ENTITY,
        "order_id",
    ))
    .views(order_views())
    .with_tasks()
    .build()
}

fn order_views() -> EntityViews {
    EntityViews {
        default: Some("list".into()),
        list: Some(ListViewSpec {
            columns: vec![
                col("doc_no"),
                col("customer_name"),
                col("order_date"),
                col("status"),
                col("fulfillment_status"),
                col("total"),
            ],
            ..Default::default()
        }),
        form: Some(FormViewSpec::sections(vec![
            ViewSectionSpec::new("Order").fields(&[
                "order_date",
                "customer_type",
                "customer_id",
                "customer_name",
                "quote_id",
                "currency",
                "tax_rate",
                "discount",
                "status",
                "fulfillment_status",
                "notes",
            ]),
            ViewSectionSpec::new("Lines").fields(&["items"]),
            ViewSectionSpec::new("Totals").fields(&["subtotal", "tax", "total"]),
        ])),
        detail: Some(DetailViewSpec::sections(vec![
            ViewSectionSpec::new("Order").fields(&[
                "doc_no",
                "customer_name",
                "status",
                "fulfillment_status",
                "total",
            ]),
            ViewSectionSpec::new("Lines").fields(&["items"]),
            ViewSectionSpec::new("Totals").fields(&["subtotal", "discount", "tax"]),
        ])),
        ..Default::default()
    }
}

pub fn sales_order_item_entity() -> EntityDef {
    let mut b = EntityDef::new(SALES_ORDER_ITEM_ENTITY)
        .label("Sales Order Item")
        .label_plural("Sales Order Items")
        .table_name("sales_order_items")
        .slug_name(SALES_ORDER_ITEM_SLUG)
        .child_of(SALES_ORDER_ENTITY, "items")
        .display_field("description")
        .no_activity()
        .no_comments();
    for f in line_fields(SALES_ORDER_ENTITY, "order_id") {
        b = b.field(f);
    }
    b.field(
        FieldDef::integer("qty_fulfilled")
            .nullable()
            .min(0.0)
            .default_value(json!(0))
            .server_managed()
            .label("Fulfilled"),
    )
    .build()
}

pub fn shipment_entity() -> EntityDef {
    EntityDef::new(SHIPMENT_ENTITY)
        .label("Shipment")
        .label_plural("Shipments")
        .table_name("shipments")
        .slug_name(SHIPMENT_SLUG)
        .icon("truck")
        .description("Fulfillment against a sales order. No carrier integration.")
        .workflow(SHIPMENT_WORKFLOW)
        .display_field("doc_no")
        .audit()
        .document(DocumentConfig::new().lock_states(&[SHIP_DELIVERED]))
        .naming(NamingConfig::new("SHP-{YYYY}-{#####}"))
        .field(
            FieldDef::string("doc_no")
                .nullable()
                .searchable()
                .readonly()
                .label("Number")
                .section("Shipment"),
        )
        .field(
            FieldDef::many_to_one("order_id", SALES_ORDER_ENTITY)
                .required()
                .label("Order")
                .indexed()
                .on_delete(OnDelete::Restrict)
                .section("Shipment"),
        )
        .field(
            FieldDef::string("warehouse")
                .nullable()
                .filterable()
                .section("Shipment"),
        )
        .field(
            FieldDef::enum_values(
                "status",
                vec![SHIP_PENDING, SHIP_READY, SHIP_SHIPPED, SHIP_DELIVERED],
            )
            .required()
            .default_value(json!(SHIP_PENDING))
            .filterable()
            .readonly()
            .section("Shipment"),
        )
        .field(
            FieldDef::date("shipped_at")
                .nullable()
                .filterable()
                .server_managed()
                .section("Shipment"),
        )
        .child_table(
            ChildTableDef::new("items", SHIPMENT_ITEM_ENTITY)
                .parent_field("shipment_id")
                .columns(&["order_item_id", "product_id", "quantity"]),
        )
        .views(EntityViews {
            list: Some(ListViewSpec {
                columns: vec![
                    col("doc_no"),
                    col("order_id"),
                    col("status"),
                    col("warehouse"),
                ],
                ..Default::default()
            }),
            form: Some(FormViewSpec::sections(vec![
                ViewSectionSpec::new("Shipment").fields(&["order_id", "warehouse", "status"]),
                ViewSectionSpec::new("Lines").fields(&["items"]),
            ])),
            detail: Some(DetailViewSpec::sections(vec![
                ViewSectionSpec::new("Shipment").fields(&[
                    "doc_no",
                    "order_id",
                    "warehouse",
                    "status",
                    "shipped_at",
                ]),
                ViewSectionSpec::new("Lines").fields(&["items"]),
            ])),
            ..Default::default()
        })
        .build()
}

pub fn shipment_item_entity() -> EntityDef {
    EntityDef::new(SHIPMENT_ITEM_ENTITY)
        .label("Shipment Item")
        .label_plural("Shipment Items")
        .table_name("shipment_items")
        .slug_name(SHIPMENT_ITEM_SLUG)
        .child_of(SHIPMENT_ENTITY, "items")
        .no_activity()
        .no_comments()
        .field(
            FieldDef::many_to_one("shipment_id", SHIPMENT_ENTITY)
                .required()
                .hidden()
                .indexed()
                .on_delete(OnDelete::Cascade),
        )
        .field(
            FieldDef::many_to_one("order_item_id", SALES_ORDER_ITEM_ENTITY)
                .required()
                .label("Order line")
                .on_delete(OnDelete::Restrict),
        )
        .field(
            FieldDef::many_to_one("product_id", PRODUCT_ENTITY)
                .nullable()
                .label("Product"),
        )
        .field(
            FieldDef::integer("quantity")
                .required()
                .min(1.0)
                .default_value(json!(1)),
        )
        .build()
}

pub fn invoice_entity() -> EntityDef {
    let mut b = EntityDef::new(INVOICE_ENTITY)
        .label("Invoice")
        .label_plural("Invoices")
        .table_name("invoices")
        .slug_name(INVOICE_SLUG)
        .icon("file")
        .description("Customer invoice. Issue posts the accounting journal when mappings exist.")
        .workflow(INVOICE_WORKFLOW)
        .display_field("doc_no")
        .audit()
        .attachments()
        .document(
            DocumentConfig::new()
                .submit()
                .duplicate()
                .lock_states(&[INVOICE_ISSUED, INVOICE_PAID]),
        )
        .naming(NamingConfig::new("INV-{YYYY}-{#####}"))
        .print_format(document_print("Invoice", INVOICE_ENTITY, "Invoice"))
        .print_format(
            document_print("Invoice Compact", INVOICE_ENTITY, "Invoice").variant("compact"),
        )
        .field(
            FieldDef::string("doc_no")
                .nullable()
                .searchable()
                .search_weight(10)
                .readonly()
                .label("Number")
                .section("Invoice"),
        );
    for f in party_fields("Invoice") {
        b = b.field(f);
    }
    b = b
        .field(
            FieldDef::many_to_one("order_id", SALES_ORDER_ENTITY)
                .nullable()
                .label("Order")
                .on_delete(OnDelete::SetNull)
                .section("Invoice"),
        )
        .field(
            FieldDef::date("invoice_date")
                .required()
                .default_from("current_date")
                .filterable()
                .indexed()
                .section("Invoice"),
        )
        .field(
            FieldDef::date("due_date")
                .nullable()
                .filterable()
                .section("Invoice"),
        )
        .field(
            FieldDef::enum_values("status", vec![INVOICE_DRAFT, INVOICE_ISSUED, INVOICE_PAID])
                .required()
                .default_value(json!(INVOICE_DRAFT))
                .filterable()
                .readonly()
                .section("Invoice"),
        )
        .field(
            FieldDef::currency("paid_amount")
                .nullable()
                .min(0.0)
                .default_value(json!(0))
                .server_managed()
                .label("Paid")
                .section("Totals"),
        )
        .field(
            FieldDef::uuid("journal_id")
                .nullable()
                .hidden()
                .server_managed(),
        )
        .field(
            FieldDef::text("notes")
                .nullable()
                .list(false)
                .section("Invoice"),
        )
        .child_table(
            ChildTableDef::new("items", INVOICE_ITEM_ENTITY)
                .parent_field("invoice_id")
                .columns(&[
                    "product_id",
                    "description",
                    "quantity",
                    "unit_price",
                    "amount",
                ]),
        );
    for f in money_header("Invoice") {
        b = b.field(f);
    }
    b.field(FieldDef::one_to_many(
        "allocations",
        PAYMENT_ALLOCATION_ENTITY,
        "invoice_id",
    ))
    .views(invoice_views())
    .with_tasks()
    .build()
}

fn invoice_views() -> EntityViews {
    EntityViews {
        default: Some("list".into()),
        list: Some(ListViewSpec {
            columns: vec![
                col("doc_no"),
                col("customer_name"),
                col("invoice_date"),
                col("due_date"),
                col("status"),
                col("total"),
                col("paid_amount"),
            ],
            ..Default::default()
        }),
        form: Some(FormViewSpec::sections(vec![
            ViewSectionSpec::new("Invoice").fields(&[
                "invoice_date",
                "due_date",
                "customer_type",
                "customer_id",
                "customer_name",
                "order_id",
                "currency",
                "tax_rate",
                "discount",
                "status",
                "notes",
            ]),
            ViewSectionSpec::new("Lines").fields(&["items"]),
            ViewSectionSpec::new("Totals").fields(&["subtotal", "tax", "total", "paid_amount"]),
        ])),
        detail: Some(DetailViewSpec::sections(vec![
            ViewSectionSpec::new("Invoice").fields(&[
                "doc_no",
                "customer_name",
                "status",
                "due_date",
                "total",
                "paid_amount",
            ]),
            ViewSectionSpec::new("Lines").fields(&["items"]),
            ViewSectionSpec::new("Totals").fields(&["subtotal", "discount", "tax"]),
        ])),
        ..Default::default()
    }
}

pub fn invoice_item_entity() -> EntityDef {
    let mut b = EntityDef::new(INVOICE_ITEM_ENTITY)
        .label("Invoice Item")
        .label_plural("Invoice Items")
        .table_name("invoice_items")
        .slug_name(INVOICE_ITEM_SLUG)
        .child_of(INVOICE_ENTITY, "items")
        .display_field("description")
        .no_activity()
        .no_comments();
    for f in line_fields(INVOICE_ENTITY, "invoice_id") {
        b = b.field(f);
    }
    b.build()
}

pub fn sales_payment_entity() -> EntityDef {
    let mut b = EntityDef::new(SALES_PAYMENT_ENTITY)
        .label("Payment")
        .label_plural("Payments")
        .table_name("sales_payments")
        .slug_name(SALES_PAYMENT_SLUG)
        .icon("credit-card")
        .description(
            "Customer payment allocated to invoices. Restaurant Payment remains order tender.",
        )
        .workflow(PAYMENT_WORKFLOW)
        .display_field("doc_no")
        .audit()
        .document(DocumentConfig::new().lock_states(&[PAY_RECEIVED]))
        .naming(NamingConfig::new("PAY-{YYYY}-{#####}"))
        .print_format(
            PrintFormat::new("Receipt", SALES_PAYMENT_ENTITY)
                .title("Receipt")
                .filename_field("doc_no")
                .item_table("allocations")
                .total_fields(&["amount"])
                .section(PrintSection::kind("header"))
                .section(PrintSection::kind("customer").fields(&["customer_name"]))
                .section(PrintSection::kind("items").loop_over("allocations"))
                .section(PrintSection::kind("totals"))
                .section(PrintSection::kind("footer")),
        )
        .field(
            FieldDef::string("doc_no")
                .nullable()
                .searchable()
                .readonly()
                .label("Number")
                .section("Payment"),
        );
    for f in party_fields("Payment") {
        b = b.field(f);
    }
    b.field(
        FieldDef::currency("amount")
            .required()
            .min(0.0)
            .section("Payment"),
    )
    .field(
        FieldDef::string("currency")
            .required()
            .default_from("tenant_currency")
            .section("Payment"),
    )
    .field(
        FieldDef::enum_values("method", vec!["Cash", "Bank", "Card", "Online", "Other"])
            .required()
            .default_value(json!("Cash"))
            .filterable()
            .section("Payment"),
    )
    .field(
        FieldDef::enum_values("status", vec![PAY_DRAFT, PAY_RECEIVED])
            .required()
            .default_value(json!(PAY_DRAFT))
            .filterable()
            .readonly()
            .section("Payment"),
    )
    .field(
        FieldDef::date("received_at")
            .nullable()
            .default_from("current_date")
            .filterable()
            .section("Payment"),
    )
    .field(
        FieldDef::uuid("journal_id")
            .nullable()
            .hidden()
            .server_managed(),
    )
    .child_table(
        ChildTableDef::new("allocations", PAYMENT_ALLOCATION_ENTITY)
            .parent_field("payment_id")
            .columns(&["invoice_id", "amount"]),
    )
    .views(EntityViews {
        list: Some(ListViewSpec {
            columns: vec![
                col("doc_no"),
                col("customer_name"),
                col("amount"),
                col("method"),
                col("status"),
            ],
            ..Default::default()
        }),
        form: Some(FormViewSpec::sections(vec![
            ViewSectionSpec::new("Payment").fields(&[
                "customer_type",
                "customer_id",
                "customer_name",
                "amount",
                "currency",
                "method",
                "received_at",
                "status",
            ]),
            ViewSectionSpec::new("Allocations").fields(&["allocations"]),
        ])),
        detail: Some(DetailViewSpec::sections(vec![
            ViewSectionSpec::new("Payment").fields(&[
                "doc_no",
                "customer_name",
                "amount",
                "method",
                "status",
            ]),
            ViewSectionSpec::new("Allocations").fields(&["allocations"]),
        ])),
        ..Default::default()
    })
    .build()
}

pub fn payment_allocation_entity() -> EntityDef {
    EntityDef::new(PAYMENT_ALLOCATION_ENTITY)
        .label("Payment Allocation")
        .label_plural("Payment Allocations")
        .table_name("payment_allocations")
        .slug_name(PAYMENT_ALLOCATION_SLUG)
        .child_of(SALES_PAYMENT_ENTITY, "allocations")
        .no_activity()
        .no_comments()
        .field(
            FieldDef::many_to_one("payment_id", SALES_PAYMENT_ENTITY)
                .required()
                .hidden()
                .indexed()
                .on_delete(OnDelete::Cascade),
        )
        .field(
            FieldDef::many_to_one("invoice_id", INVOICE_ENTITY)
                .required()
                .label("Invoice")
                .indexed()
                .on_delete(OnDelete::Restrict),
        )
        .field(FieldDef::currency("amount").required().min(0.0))
        .build()
}

pub fn sales_return_entity() -> EntityDef {
    let mut b = EntityDef::new(SALES_RETURN_ENTITY)
        .label("Return")
        .label_plural("Returns")
        .table_name("sales_returns")
        .slug_name(SALES_RETURN_SLUG)
        .icon("undo")
        .description("Return against a sales order. Refund emits an event; no payment gateway.")
        .workflow(RETURN_WORKFLOW)
        .display_field("doc_no")
        .audit()
        .attachments()
        .document(DocumentConfig::new().lock_states(&[RETURN_REFUNDED]))
        .naming(NamingConfig::new("RET-{YYYY}-{#####}"))
        .field(
            FieldDef::string("doc_no")
                .nullable()
                .searchable()
                .readonly()
                .label("Number")
                .section("Return"),
        );
    for f in party_fields("Return") {
        b = b.field(f);
    }
    b.field(
        FieldDef::many_to_one("order_id", SALES_ORDER_ENTITY)
            .required()
            .label("Order")
            .indexed()
            .on_delete(OnDelete::Restrict)
            .section("Return"),
    )
    .field(
        FieldDef::enum_values(
            "status",
            vec![
                RETURN_REQUESTED,
                RETURN_APPROVED,
                RETURN_RECEIVED,
                RETURN_REFUNDED,
            ],
        )
        .required()
        .default_value(json!(RETURN_REQUESTED))
        .filterable()
        .readonly()
        .section("Return"),
    )
    .field(
        FieldDef::text("notes")
            .nullable()
            .list(false)
            .section("Return"),
    )
    .child_table(
        ChildTableDef::new("items", SALES_RETURN_ITEM_ENTITY)
            .parent_field("return_id")
            .columns(&["order_item_id", "product_id", "quantity"]),
    )
    .views(EntityViews {
        list: Some(ListViewSpec {
            columns: vec![
                col("doc_no"),
                col("customer_name"),
                col("order_id"),
                col("status"),
            ],
            ..Default::default()
        }),
        form: Some(FormViewSpec::sections(vec![
            ViewSectionSpec::new("Return").fields(&[
                "order_id",
                "customer_type",
                "customer_id",
                "customer_name",
                "status",
                "notes",
            ]),
            ViewSectionSpec::new("Lines").fields(&["items"]),
        ])),
        detail: Some(DetailViewSpec::sections(vec![
            ViewSectionSpec::new("Return").fields(&[
                "doc_no",
                "order_id",
                "customer_name",
                "status",
            ]),
            ViewSectionSpec::new("Lines").fields(&["items"]),
        ])),
        ..Default::default()
    })
    .build()
}

pub fn sales_return_item_entity() -> EntityDef {
    EntityDef::new(SALES_RETURN_ITEM_ENTITY)
        .label("Return Item")
        .label_plural("Return Items")
        .table_name("sales_return_items")
        .slug_name(SALES_RETURN_ITEM_SLUG)
        .child_of(SALES_RETURN_ENTITY, "items")
        .no_activity()
        .no_comments()
        .field(
            FieldDef::many_to_one("return_id", SALES_RETURN_ENTITY)
                .required()
                .hidden()
                .indexed()
                .on_delete(OnDelete::Cascade),
        )
        .field(
            FieldDef::many_to_one("order_item_id", SALES_ORDER_ITEM_ENTITY)
                .required()
                .label("Order line")
                .on_delete(OnDelete::Restrict),
        )
        .field(
            FieldDef::many_to_one("product_id", PRODUCT_ENTITY)
                .nullable()
                .label("Product"),
        )
        .field(
            FieldDef::integer("quantity")
                .required()
                .min(1.0)
                .default_value(json!(1)),
        )
        .build()
}

pub fn commerce_entities() -> Vec<EntityDef> {
    vec![
        product_entity(),
        quote_entity(),
        quote_item_entity(),
        sales_order_entity(),
        sales_order_item_entity(),
        shipment_entity(),
        shipment_item_entity(),
        invoice_entity(),
        invoice_item_entity(),
        sales_payment_entity(),
        payment_allocation_entity(),
        sales_return_entity(),
        sales_return_item_entity(),
    ]
}

pub fn commerce_nav_items() -> Vec<NavItem> {
    vec![
        NavItem::new("Products", PRODUCT_ENTITY).section("Sales"),
        NavItem::new("Quotes", QUOTE_ENTITY).section("Sales"),
        NavItem::new("Sales Orders", SALES_ORDER_ENTITY).section("Sales"),
        NavItem::new("Shipments", SHIPMENT_ENTITY).section("Sales"),
        NavItem::new("Invoices", INVOICE_ENTITY).section("Sales"),
        NavItem::new("Payments", SALES_PAYMENT_ENTITY).section("Sales"),
        NavItem::new("Returns", SALES_RETURN_ENTITY).section("Sales"),
    ]
}

pub fn commerce_child_slugs() -> Vec<&'static str> {
    vec![
        QUOTE_ITEM_SLUG,
        SALES_ORDER_ITEM_SLUG,
        SHIPMENT_ITEM_SLUG,
        INVOICE_ITEM_SLUG,
        PAYMENT_ALLOCATION_SLUG,
        SALES_RETURN_ITEM_SLUG,
    ]
}

pub fn is_commerce_entity(name: &str) -> bool {
    matches!(
        name,
        PRODUCT_ENTITY
            | QUOTE_ENTITY
            | QUOTE_ITEM_ENTITY
            | SALES_ORDER_ENTITY
            | SALES_ORDER_ITEM_ENTITY
            | SHIPMENT_ENTITY
            | SHIPMENT_ITEM_ENTITY
            | INVOICE_ENTITY
            | INVOICE_ITEM_ENTITY
            | SALES_PAYMENT_ENTITY
            | PAYMENT_ALLOCATION_ENTITY
            | SALES_RETURN_ENTITY
            | SALES_RETURN_ITEM_ENTITY
    )
}

pub fn commerce_reports() -> Vec<ReportDef> {
    vec![
        ReportDef::new("sales-by-customer", SALES_ORDER_ENTITY)
            .label("Sales by Customer")
            .fields(&["customer_name", "total"])
            .group_by(&["customer_name"])
            .sum("total")
            .chart("bar"),
        ReportDef::new("sales-by-product", SALES_ORDER_ITEM_ENTITY)
            .label("Sales by Product")
            .fields(&["product_id", "amount"])
            .group_by(&["product_id"])
            .sum("amount")
            .chart("bar"),
        ReportDef::new("orders-by-status", SALES_ORDER_ENTITY)
            .label("Orders by Status")
            .fields(&["status", "total"])
            .group_by(&["status"])
            .sum("total")
            .count("id")
            .chart("bar"),
        ReportDef::new("invoices-outstanding", INVOICE_ENTITY)
            .label("Invoices Outstanding")
            .fields(&["customer_name", "total", "paid_amount"])
            .group_by(&["customer_name"])
            .sum("total")
            .sum("paid_amount")
            .filter_eq("status", json!(INVOICE_ISSUED)),
        ReportDef::new("payments-received", SALES_PAYMENT_ENTITY)
            .label("Payments")
            .fields(&["method", "amount"])
            .group_by(&["method"])
            .sum("amount")
            .filter_eq("status", json!(PAY_RECEIVED)),
        ReportDef::new("returns-by-status", SALES_RETURN_ENTITY)
            .label("Returns")
            .fields(&["status"])
            .group_by(&["status"])
            .count("id"),
    ]
}

pub fn commerce_dashboard() -> DashboardDef {
    DashboardDef::new("commerce", "Commerce")
        .card(
            DashboardCard::sum("Today's sales", SALES_ORDER_ENTITY, "total")
                .filter("status", ORDER_COMPLETED)
                .size("sm"),
        )
        .card(
            DashboardCard::count("Open orders", SALES_ORDER_ENTITY)
                .filter("status", ORDER_CONFIRMED)
                .size("sm"),
        )
        .card(
            DashboardCard::count("Pending fulfillment", SALES_ORDER_ENTITY)
                .filter("fulfillment_status", FULFILL_UNFULFILLED)
                .size("sm"),
        )
        .card(
            DashboardCard::count("Outstanding invoices", INVOICE_ENTITY)
                .filter("status", INVOICE_ISSUED)
                .size("sm"),
        )
        .card(
            DashboardCard::sum("Payments", SALES_PAYMENT_ENTITY, "amount")
                .filter("status", PAY_RECEIVED)
                .size("sm"),
        )
        .card(
            DashboardCard::count("Returns", SALES_RETURN_ENTITY)
                .filter("status", RETURN_REQUESTED)
                .size("sm"),
        )
}

pub fn commerce_notifications() -> Vec<NotificationDef> {
    vec![
        NotificationDef::new("order_confirmed", "order.confirmed")
            .channels(&["in_app"])
            .recipients(&["Manager", "Staff"])
            .title("Order confirmed")
            .body("A sales order was confirmed."),
        NotificationDef::new("invoice_issued", "invoice.issued")
            .channels(&["in_app"])
            .recipients(&["Manager", "Staff"])
            .title("Invoice issued")
            .body("An invoice was issued."),
        NotificationDef::new("payment_received", "payment.received")
            .channels(&["in_app"])
            .recipients(&["Manager", "Staff"])
            .title("Payment received")
            .body("A customer payment was recorded."),
    ]
}

pub fn commerce_communications() -> Vec<CommunicationDef> {
    vec![
        CommunicationDef::new("invoice_issued", "invoice.issued", INVOICE_ENTITY)
            .channels(&[CHANNEL_EMAIL, CHANNEL_WHATSAPP, CHANNEL_IN_APP])
            .purpose(PURPOSE_TRANSACTIONAL)
            .subject("Invoice {{ number }}")
            .body("Hello {{ customer_name }},\nyour invoice {{ number }} for {{ total | currency }} has been issued.")
            .preferred_channel_field("communication_channel")
            .opt_out_field("marketing_opt_out")
            .attach_document(),
        CommunicationDef::new("invoice_overdue_reminder", "invoice.issued", INVOICE_ENTITY)
            .channels(&[CHANNEL_EMAIL, CHANNEL_IN_APP])
            .purpose(PURPOSE_TRANSACTIONAL)
            .subject("Invoice {{ number }} is due")
            .body("Hello {{ customer_name }},\ninvoice {{ number }} for {{ total | currency }} is due. Please send payment.")
            .preferred_channel_field("communication_channel")
            .opt_out_field("marketing_opt_out"),
        CommunicationDef::new("payment_received", "payment.received", SALES_PAYMENT_ENTITY)
            .channels(&[CHANNEL_EMAIL, CHANNEL_IN_APP])
            .purpose(PURPOSE_TRANSACTIONAL)
            .subject("Payment received")
            .body("Hello {{ customer_name }},\nwe received your payment of {{ amount | currency }}.")
            .preferred_channel_field("communication_channel")
            .opt_out_field("marketing_opt_out"),
        CommunicationDef::new("sales_order_confirmed", "order.confirmed", SALES_ORDER_ENTITY)
            .channels(&[CHANNEL_EMAIL, CHANNEL_WHATSAPP, CHANNEL_IN_APP])
            .purpose(PURPOSE_TRANSACTIONAL)
            .subject("Order {{ number }} confirmed")
            .body("Hello {{ customer_name }},\nyour order {{ number }} is confirmed.\nTotal: {{ total | currency }}")
            .preferred_channel_field("communication_channel")
            .opt_out_field("marketing_opt_out"),
    ]
}

pub fn commerce_automations() -> Vec<AutomationDef> {
    vec![
        AutomationDef::new(
            "order_confirmed_notify",
            AutomationTrigger::event("order.confirmed"),
        )
        .description("Notify staff when a sales order is confirmed")
        .action(AutomationAction::Notify {
            notify: NotifyAction {
                notification: Some("order_confirmed".into()),
                recipients: vec!["Staff".into()],
                title: Some("Order confirmed".into()),
                ..Default::default()
            },
        }),
        AutomationDef::new(
            "invoice_issued_notify",
            AutomationTrigger::event("invoice.issued"),
        )
        .description("Notify staff when an invoice is issued")
        .action(AutomationAction::Notify {
            notify: NotifyAction {
                notification: Some("invoice_issued".into()),
                recipients: vec!["Staff".into()],
                title: Some("Invoice issued".into()),
                ..Default::default()
            },
        }),
        AutomationDef::new(
            "payment_received_notify",
            AutomationTrigger::event("payment.received"),
        )
        .description("Notify staff when a payment is received")
        .action(AutomationAction::Notify {
            notify: NotifyAction {
                notification: Some("payment_received".into()),
                recipients: vec!["Staff".into()],
                title: Some("Payment received".into()),
                ..Default::default()
            },
        }),
        AutomationDef::new(
            "invoice_overdue_reminder",
            AutomationTrigger::event("invoice.issued"),
        )
        .description("Wait until the due date, then remind if the invoice is still unpaid")
        .step(AutomationStep::wait_until("due_date"))
        .step(AutomationStep::branch(
            Condition::field_equals("status", INVOICE_ISSUED),
            vec![AutomationStep::action(
                AutomationAction::send_communication("invoice_overdue_reminder"),
            )],
            vec![AutomationStep::End { end: true }],
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::UI_SCHEMA_VERSION;

    #[test]
    fn sales_order_does_not_replace_restaurant_order() {
        assert_ne!(SALES_ORDER_ENTITY, "Order");
        assert_ne!(SALES_PAYMENT_ENTITY, "Payment");
        let order = sales_order_entity();
        assert!(order.tenant_owned);
        assert_eq!(order.slug, SALES_ORDER_SLUG);
        assert_eq!(order.to_ui_meta().schema_version, UI_SCHEMA_VERSION);
        for entity in commerce_entities() {
            entity
                .validate_ui_layout()
                .unwrap_or_else(|e| panic!("{}: {e}", entity.name));
        }
    }

    #[test]
    fn with_commerce_adds_customer_links() {
        let mut customer = EntityDef::new("Customer")
            .table_name("customers")
            .slug_name("customers")
            .field(FieldDef::string("name").required())
            .build();
        assert!(apply_commerce_links(&mut customer));
        assert!(customer.links.iter().any(|l| l.entity == QUOTE_ENTITY));
        assert!(customer
            .links
            .iter()
            .any(|l| l.entity == SALES_ORDER_ENTITY));
        assert!(!apply_commerce_links(&mut customer));
    }
}
