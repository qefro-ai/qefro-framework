use qefro_core::ui::{
    ChartMeasureSpec, ChartViewSpec, DetailViewSpec, FormViewSpec, ListColumnSpec, SortSpec,
    ViewColumnSpec, ViewSectionSpec,
};
use qefro_core::{
    CalendarViewSpec, ChildTableDef, DocumentConfig, EntityActionDef, EntityDef, EntityViews,
    FieldDef, KanbanCardSpec, KanbanViewSpec, LinkDef, ListViewSpec, NamingConfig, PrintFormat,
    PublicFormDef, UiConfig,
};
use serde_json::json;

pub fn customer() -> EntityDef {
    EntityDef::new("Customer")
        .label("Customer")
        .label_plural("Customers")
        .table_name("customers")
        .icon("users")
        .description("Guest record. Optionally link a Person; keep name, email, and phone for walk-ins.")
        .field(
            FieldDef::many_to_one("person_id", "Person")
                .nullable()
                .label("Person")
                .help("Optional. Link a Person for a known individual. Walk-ins leave this empty and use name, email, and phone on this Customer. When linked, Person is the source of truth for those fields. A User is only needed if they should sign in.")
                .section("Identity")
                .filterable(),
        )
        .with_party()
        .field(
            FieldDef::string("name")
                .required()
                .search_weight(10)
                .max_length(200)
                .filterable()
                .section("Contact"),
        )
        .field(
            FieldDef::string("email")
                .required()
                .email()
                .unique()
                .searchable()
                .filterable()
                .section("Contact"),
        )
        .field(
            FieldDef::string("phone")
                .nullable()
                .phone()
                .searchable()
                .section("Contact"),
        )
        .field(
            FieldDef::text("notes")
                .nullable()
                .list(false)
                .permission_level(1)
                .section("Contact"),
        )
        .views(EntityViews {
            list: Some(ListViewSpec {
                columns: vec![
                    ListColumnSpec {
                        field: "name".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "email".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "phone".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "person_id".into(),
                        width: None,
                        widget: Some("relation".into()),
                    },
                ],
                default_sort: Some(SortSpec {
                    field: "name".into(),
                    direction: Some("asc".into()),
                }),
                ..Default::default()
            }),
            form: Some(FormViewSpec::sections(vec![
                ViewSectionSpec::new("Customer Information").columns(&[
                    ViewColumnSpec::fields(&["name", "email", "phone"]),
                    ViewColumnSpec::fields(&["party_type", "person_id"]),
                ]),
                ViewSectionSpec::new("Organization Details")
                    .fields(&["organization_id"])
                    .visible_when("party_type", json!("Organization")),
                ViewSectionSpec::new("Notes").fields(&["notes"]),
            ])),
            detail: Some(DetailViewSpec::sections(vec![
                ViewSectionSpec::new("Customer").fields(&["name", "email", "phone"]),
                ViewSectionSpec::new("Business").fields(&["party_type", "person_id"]),
                ViewSectionSpec::new("Organization Details")
                    .fields(&["organization_id"])
                    .visible_when("party_type", json!("Organization")),
            ])),
            ..Default::default()
        })
        .field(FieldDef::one_to_many(
            "reservations",
            "Reservation",
            "customer_id",
        ))
        .field(FieldDef::one_to_many("orders", "Order", "customer_id"))
        .with_commerce()
        .link(
            LinkDef::new("Orders", "Order", "customer_id")
                .columns(&["doc_no", "status", "grand_total"])
                .limit(20),
        )
        .link(
            LinkDef::new("Reservations", "Reservation", "customer_id")
                .columns(&["guest_name", "reservation_date", "status"])
                .limit(20),
        )
        .with_tasks()
        .with_archive()
        .attachments()
        .build()
}

pub fn restaurant() -> EntityDef {
    EntityDef::new("Restaurant")
        .label("Restaurant")
        .label_plural("Restaurants")
        .table_name("restaurants")
        .description("Locations, hours, and brand details for each restaurant")
        .field(FieldDef::string("name").required().searchable())
        .field(
            FieldDef::string("timezone")
                .required()
                .default_from("tenant_timezone"),
        )
        .field(FieldDef::string("phone").nullable().phone())
        .field(
            FieldDef::string("brand_color")
                .nullable()
                .ui(UiConfig::color())
                .section("Branding"),
        )
        .field(
            FieldDef::string("logo")
                .nullable()
                .ui(UiConfig::image())
                .list(false)
                .section("Branding"),
        )
        .field(FieldDef::one_to_many("branches", "Branch", "restaurant_id"))
        .field(FieldDef::one_to_many(
            "menu_categories",
            "MenuCategory",
            "restaurant_id",
        ))
        .build()
}

pub fn restaurant_settings() -> EntityDef {
    EntityDef::single("RestaurantSettings")
        .label("Restaurant Settings")
        .label_plural("Restaurant Settings")
        .table_name("restaurant_settings")
        .slug_name("restaurant-settings")
        .description("Tax, currency, and service defaults for this workspace")
        .field(FieldDef::string("restaurant_name").searchable().nullable())
        .field(
            FieldDef::string("timezone")
                .nullable()
                .default_from("tenant_timezone"),
        )
        .field(
            FieldDef::string("currency")
                .nullable()
                .default_from("tenant_currency"),
        )
        .field(FieldDef::decimal("default_tax").nullable().with_currency())
        .build()
}

pub fn branch() -> EntityDef {
    EntityDef::new("Branch")
        .label("Branch")
        .label_plural("Branches")
        .table_name("branches")
        .slug_name("branches")
        .description("Dining rooms and street addresses")
        .field(FieldDef::string("name").required().searchable())
        .field(
            FieldDef::many_to_one("restaurant_id", "Restaurant")
                .required()
                .label("Restaurant"),
        )
        .field(FieldDef::string("address").nullable())
        .field(FieldDef::string("phone").nullable())
        .field(FieldDef::one_to_many("tables", "DiningTable", "branch_id"))
        .build()
}

pub fn table() -> EntityDef {
    EntityDef::new("DiningTable")
        .label("Table")
        .label_plural("Tables")
        .table_name("dining_tables")
        .slug_name("tables")
        .icon("layout")
        .display_field("code")
        .field(
            FieldDef::string("code")
                .required()
                .searchable()
                .filterable(),
        )
        .field(
            FieldDef::many_to_one("branch_id", "Branch")
                .required()
                .label("Branch"),
        )
        .field(
            FieldDef::integer("seats")
                .required()
                .min(1.0)
                .max(50.0)
                .default_value(json!(2)),
        )
        .field(
            FieldDef::enum_values("status", vec!["available", "occupied", "reserved"])
                .required()
                .default_value(json!("available"))
                .filterable(),
        )
        .field(FieldDef::one_to_many(
            "reservations",
            "Reservation",
            "table_id",
        ))
        .field(FieldDef::one_to_many("orders", "Order", "table_id"))
        .build()
}

pub fn menu_category() -> EntityDef {
    EntityDef::new("MenuCategory")
        .label("Menu Category")
        .label_plural("Menu Categories")
        .table_name("menu_categories")
        .description("Groups on the menu")
        .field(FieldDef::string("name").required().searchable())
        .field(FieldDef::many_to_one("restaurant_id", "Restaurant").required())
        .field(
            FieldDef::integer("sort_order")
                .nullable()
                .default_value(json!(0)),
        )
        .field(FieldDef::one_to_many("items", "MenuItem", "category_id"))
        .build()
}

pub fn menu_item() -> EntityDef {
    EntityDef::new("MenuItem")
        .label("Menu Item")
        .label_plural("Menu Items")
        .table_name("menu_items")
        .description("Dishes, prices, and availability")
        .field(FieldDef::string("name").required().searchable())
        .field(FieldDef::text("description").nullable().list(false))
        .field(
            FieldDef::many_to_one("category_id", "MenuCategory")
                .required()
                .label("Category"),
        )
        .field(
            FieldDef::decimal("price")
                .required()
                .min(0.0)
                .with_currency(),
        )
        .field(
            FieldDef::string("image")
                .nullable()
                .ui(UiConfig::image())
                .list(false),
        )
        .field(
            FieldDef::boolean("available")
                .required()
                .default_value(json!(true))
                .filterable(),
        )
        .build()
}

pub fn reservation() -> EntityDef {
    EntityDef::new("Reservation")
        .label("Reservation")
        .label_plural("Reservations")
        .table_name("reservations")
        .icon("calendar")
        .display_field("guest_name")
        .workflow("reservation")
        .attachments()
        .action(EntityActionDef::new("confirm").label("Confirm"))
        .action(
            EntityActionDef::new("cancel")
                .label("Cancel")
                .confirm("Cancel this reservation?"),
        )
        .link(LinkDef::new("Orders", "Order", "reservation_id"))
        .public_form(
            PublicFormDef::new("book-table")
                .title("Book a table")
                .fields(&[
                    "guest_name",
                    "guest_phone",
                    "reservation_date",
                    "reservation_time",
                    "party_size",
                ])
                .success_message("Reservation received. We'll contact you shortly."),
        )
        .field(
            FieldDef::relation("customer_id", "Customer")
                .nullable()
                .label("Customer")
                .section("Booking Details")
                .filterable()
                .search_related(),
        )
        .field(
            FieldDef::relation("table_id", "DiningTable")
                .nullable()
                .label("Table")
                .section("Booking Details"),
        )
        .field(
            FieldDef::date("reservation_date")
                .required()
                .filterable()
                .sortable()
                .ui(UiConfig::date())
                .section("Booking Details")
                .default_from("current_date"),
        )
        .field(
            FieldDef::time("reservation_time")
                .required()
                .ui(UiConfig::time().minute_step(15))
                .section("Booking Details"),
        )
        .field(
            FieldDef::time("end_time")
                .nullable()
                .label("End")
                .help("Must be after the start time when set.")
                .ui(UiConfig::time().minute_step(15))
                .section("Booking Details"),
        )
        .field(
            FieldDef::integer("party_size")
                .required()
                .min(1.0)
                .max(50.0)
                .label("Guests")
                .section("Booking Details")
                .readonly_when("status", json!("Completed")),
        )
        .field(
            FieldDef::string("guest_name")
                .nullable()
                .search_weight(8)
                .label("Name")
                .section("Booking Details"),
        )
        .field(
            FieldDef::string("guest_phone")
                .nullable()
                .phone()
                .searchable()
                .label("Phone")
                .section("Booking Details"),
        )
        .field(
            FieldDef::enum_(
                "status",
                vec!["Pending", "Confirmed", "Seated", "Completed", "Cancelled"],
            )
            .required()
            .default_value(json!("Pending"))
            .filterable()
            .section("Booking Details"),
        )
        .field(
            FieldDef::text("notes")
                .nullable()
                .list(false)
                .section("Additional Information"),
        )
        .field(
            FieldDef::text("cancellation_reason")
                .nullable()
                .list(false)
                .section("Additional Information")
                .visible_when("status", json!("Cancelled")),
        )
        .field(FieldDef::one_to_many("orders", "Order", "reservation_id"))
        .views(EntityViews {
            kanban: Some(KanbanViewSpec {
                group_by: Some("status".into()),
                card: Some(KanbanCardSpec {
                    title: Some("guest_name".into()),
                    subtitle: Some("reservation_time".into()),
                    fields: vec!["party_size".into(), "reservation_date".into()],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            calendar: Some(CalendarViewSpec {
                start: Some("reservation_date".into()),
                time: Some("reservation_time".into()),
                title: Some("guest_name".into()),
                subtitle: Some("status".into()),
                ..Default::default()
            }),
            form: Some(FormViewSpec::sections(vec![
                ViewSectionSpec::new("Booking Details").columns(&[
                    ViewColumnSpec::fields(&[
                        "customer_id",
                        "table_id",
                        "guest_name",
                        "guest_phone",
                    ]),
                    ViewColumnSpec::fields(&[
                        "reservation_date",
                        "reservation_time",
                        "end_time",
                        "party_size",
                        "status",
                    ]),
                ]),
                ViewSectionSpec::new("Additional Information")
                    .fields(&["notes", "cancellation_reason"]),
            ])),
            detail: Some(DetailViewSpec::sections(vec![
                ViewSectionSpec::new("Reservation").fields(&[
                    "guest_name",
                    "guest_phone",
                    "customer_id",
                    "table_id",
                    "reservation_date",
                    "reservation_time",
                    "end_time",
                    "party_size",
                    "status",
                ]),
                ViewSectionSpec::new("Notes").fields(&["notes", "cancellation_reason"]),
            ])),
            ..Default::default()
        })
        .validation_rule(qefro_core::ValidationRule::compare(
            "end_time",
            "greater_than",
            "reservation_time",
        ))
        .build()
}

pub fn order() -> EntityDef {
    EntityDef::new("Order")
        .label("Order")
        .label_plural("Orders")
        .table_name("orders")
        .icon("receipt")
        .workflow("order")
        .action(EntityActionDef::new("schedule").label("Schedule Pickup"))
        .views(EntityViews {
            default: Some("kanban".into()),
            list: Some(ListViewSpec {
                columns: vec![
                    ListColumnSpec {
                        field: "doc_no".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "order_type".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "status".into(),
                        width: None,
                        widget: Some("status".into()),
                    },
                    ListColumnSpec {
                        field: "customer_id".into(),
                        width: None,
                        widget: Some("relation".into()),
                    },
                    ListColumnSpec {
                        field: "table_id".into(),
                        width: None,
                        widget: Some("relation".into()),
                    },
                    ListColumnSpec {
                        field: "pickup_at".into(),
                        width: None,
                        widget: Some("datetime".into()),
                    },
                    ListColumnSpec {
                        field: "order_date".into(),
                        width: None,
                        widget: Some("date".into()),
                    },
                    ListColumnSpec {
                        field: "grand_total".into(),
                        width: None,
                        widget: Some("currency".into()),
                    },
                ],
                default_sort: Some(SortSpec {
                    field: "created_at".into(),
                    direction: Some("desc".into()),
                }),
                ..Default::default()
            }),
            kanban: Some(KanbanViewSpec {
                group_by: Some("status".into()),
                card: Some(KanbanCardSpec {
                    title: Some("doc_no".into()),
                    subtitle: Some("order_type".into()),
                    fields: vec!["pickup_at".into(), "table_id".into()],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            chart: Some(ChartViewSpec {
                enabled: true,
                chart_type: Some("bar".into()),
                dimension: Some("status".into()),
                measure: Some(ChartMeasureSpec {
                    field: Some("grand_total".into()),
                    aggregation: Some("sum".into()),
                }),
            }),
            form: Some(FormViewSpec::sections(vec![
                ViewSectionSpec::new("Order")
                    .tab("Details")
                    .columns(&[
                        ViewColumnSpec::fields(&["order_type", "customer_id", "table_id"]),
                        ViewColumnSpec::fields(&["reservation_id", "order_date", "status"]),
                    ]),
                ViewSectionSpec::new("Takeaway")
                    .tab("Details")
                    .fields(&["pickup_at"])
                    .visible_when("order_type", json!("Takeaway")),
                ViewSectionSpec::new("Line items")
                    .tab("Items")
                    .fields(&["items"]),
                ViewSectionSpec::new("Totals")
                    .tab("Totals")
                    .fields(&[
                        "subtotal",
                        "tax_rate",
                        "tax",
                        "discount",
                        "grand_total",
                        "notes",
                        "delivery_note",
                    ]),
            ])),
            detail: Some(DetailViewSpec::sections(vec![
                ViewSectionSpec::new("Order").fields(&[
                    "doc_no",
                    "order_type",
                    "customer_id",
                    "table_id",
                    "reservation_id",
                    "order_date",
                    "status",
                    "pickup_at",
                ]),
                ViewSectionSpec::new("Totals").fields(&[
                    "subtotal",
                    "tax_rate",
                    "tax",
                    "discount",
                    "grand_total",
                    "notes",
                ]),
            ])),
            ..Default::default()
        })
        .attachments()
        .document(
            DocumentConfig::new()
                .submit()
                .cancel()
                .duplicate()
                .lock_states(&["Confirmed", "Preparing", "Ready", "Completed", "Cancelled"])
                .number_on("submit"),
        )
        .naming(
            NamingConfig::new("ORD-{YYYY}-{#####}")
                .field("doc_no")
                .assign_on("submit"),
        )
        .print_format(
            PrintFormat::new("Order Standard", "Order")
                .title("Order")
                .item_table("items")
                .total_fields(&["subtotal", "tax", "discount", "grand_total"]),
        )
        .field(
            FieldDef::enum_values("order_type", vec!["Dine-in", "Takeaway"])
                .required()
                .default_value(json!("Dine-in"))
                .label("Type")
                .ui(UiConfig::radio())
                .help("Dine-in is served at a table. Takeaway is collected at the counter — walk-in or a scheduled pickup.")
                .filterable()
                .section("Details"),
        )
        .field(
            FieldDef::many_to_one("customer_id", "Customer")
                .nullable()
                .label("Customer")
                .search_related()
                .section("Details")
                .readonly_when("status", json!("Completed")),
        )
        .field(
            FieldDef::many_to_one("table_id", "DiningTable")
                .nullable()
                .label("Table")
                .help("Required when confirming a dine-in order.")
                .visible_when("order_type", json!("Dine-in"))
                .section("Details"),
        )
        .field(
            FieldDef::many_to_one("reservation_id", "Reservation")
                .nullable()
                .label("Reservation")
                .list(false)
                .visible_when("order_type", json!("Dine-in"))
                .section("Details"),
        )
        .field(
            FieldDef::datetime("pickup_at")
                .nullable()
                .label("Pickup at")
                .ui(UiConfig::datetime().tenant_timezone())
                .help("When the guest will collect. Required to schedule a prebooked takeaway; leave empty for walk-in.")
                .filterable()
                .sortable()
                .visible_when("order_type", json!("Takeaway"))
                .section("Takeaway"),
        )
        .field(
            FieldDef::date("order_date")
                .required()
                .default_from("current_date")
                .filterable()
                .sortable()
                .indexed()
                .ui(UiConfig::date())
                .section("Details"),
        )
        .field(
            FieldDef::enum_values(
                "status",
                vec![
                    "Draft",
                    "Scheduled",
                    "Confirmed",
                    "Preparing",
                    "Ready",
                    "Completed",
                    "Cancelled",
                ],
            )
            .required()
            .default_value(json!("Draft"))
            .filterable()
            .section("Details"),
        )
        .child_table(
            ChildTableDef::new("items", "OrderItem")
                .parent_field("order_id")
                .columns(&["menu_item_id", "quantity", "unit_price", "amount"]),
        )
        .field(
            FieldDef::currency("subtotal")
                .computed("SUM(items.amount)")
                .label("Subtotal"),
        )
        .field(
            FieldDef::decimal("tax_rate")
                .nullable()
                .min(0.0)
                .max(100.0)
                .percentage()
                .default_value(json!(0))
                .label("Tax %"),
        )
        .field(FieldDef::currency("tax").computed("ROUND(subtotal * tax_rate / 100, 2)"))
        .field(
            FieldDef::currency("discount")
                .nullable()
                .min(0.0)
                .default_value(json!(0))
                .readonly_when("status", json!("Completed")),
        )
        .field(FieldDef::currency("grand_total").computed("subtotal + tax - discount"))
        .field(FieldDef::currency("total").computed("grand_total"))
        .field(FieldDef::text("notes").nullable().list(false))
        .field(
            FieldDef::text("delivery_note")
                .nullable()
                .list(false)
                .allow_on_submit()
                .label("Delivery Note"),
        )
        .field(FieldDef::one_to_many("payments", "Payment", "order_id"))
        .with_tasks()
        .build()
}

pub fn order_item() -> EntityDef {
    EntityDef::new("OrderItem")
        .label("Order Item")
        .label_plural("Order Items")
        .table_name("order_items")
        .child_of("Order", "items")
        .field(
            FieldDef::many_to_one("order_id", "Order")
                .required()
                .label("Order")
                .hidden(),
        )
        .field(
            FieldDef::many_to_one("menu_item_id", "MenuItem")
                .required()
                .label("Product"),
        )
        .field(
            FieldDef::integer("quantity")
                .required()
                .min(1.0)
                .default_value(json!(1)),
        )
        .field(
            FieldDef::currency("unit_price")
                .required()
                .min(0.0)
                .label("Rate"),
        )
        .field(FieldDef::currency("amount").computed("quantity * unit_price"))
        .field(FieldDef::text("notes").nullable().list(false))
        .build()
}

pub fn payment() -> EntityDef {
    EntityDef::new("Payment")
        .label("Payment")
        .label_plural("Payments")
        .table_name("payments")
        .description("Captured, pending, and refunded tender")
        .field(
            FieldDef::many_to_one("order_id", "Order")
                .required()
                .label("Order"),
        )
        .field(
            FieldDef::decimal("amount")
                .required()
                .min(0.0)
                .with_currency(),
        )
        .field(
            FieldDef::enum_values("method", vec!["cash", "card", "other"])
                .required()
                .default_value(json!("card"))
                .filterable(),
        )
        .field(
            FieldDef::enum_values("status", vec!["pending", "captured", "refunded", "failed"])
                .required()
                .default_value(json!("pending"))
                .filterable(),
        )
        .build()
}

/// Framework UI showcase: every core widget, no custom React page.
pub fn ui_showcase() -> EntityDef {
    EntityDef::new("UiShowcase")
        .label("UI Showcase")
        .label_plural("UI Showcases")
        .table_name("ui_showcases")
        .slug_name("ui-showcases")
        .description("Reference entity covering the V0.6 document and widget set")
        .document(
            DocumentConfig::new()
                .duplicate()
                .lock_states(&["Cancelled"]),
        )
        .naming(NamingConfig::new("UI-{YYYY}-{####}").assign_on("create"))
        .print_format(PrintFormat::new("Showcase Standard", "UiShowcase").title("UI Showcase"))
        .field(
            FieldDef::string("name")
                .required()
                .searchable()
                .section("Basics")
                .tab("Details"),
        )
        .field(
            FieldDef::text("description")
                .nullable()
                .list(false)
                .section("Basics")
                .tab("Details"),
        )
        .field(
            FieldDef::integer("age")
                .nullable()
                .min(0.0)
                .max(120.0)
                .section("Basics")
                .tab("Details"),
        )
        .field(
            FieldDef::decimal("price")
                .nullable()
                .min(0.0)
                .with_currency()
                .section("Money")
                .tab("Details"),
        )
        .field(
            FieldDef::decimal("discount")
                .nullable()
                .percentage()
                .section("Money")
                .tab("Details"),
        )
        .field(
            FieldDef::date("birth_date")
                .nullable()
                .ui(UiConfig::date())
                .section("Schedule")
                .tab("Details"),
        )
        .field(
            FieldDef::time("appointment_time")
                .nullable()
                .ui(UiConfig::time())
                .section("Schedule")
                .tab("Details"),
        )
        .field(
            FieldDef::datetime("appointment_at")
                .nullable()
                .ui(UiConfig::datetime().tenant_timezone())
                .section("Schedule")
                .tab("Details"),
        )
        .field(
            FieldDef::string("brand_color")
                .nullable()
                .ui(UiConfig::color())
                .section("Appearance")
                .tab("Details"),
        )
        .field(
            FieldDef::enum_("status", vec!["Draft", "Active", "Cancelled"])
                .required()
                .default_value(json!("Draft"))
                .filterable()
                .section("Status")
                .tab("Details"),
        )
        .field(
            FieldDef::json("categories")
                .nullable()
                .ui(UiConfig::tags())
                .list(false)
                .section("Status")
                .tab("Details"),
        )
        .field(
            FieldDef::relation("customer_id", "Customer")
                .nullable()
                .label("Customer")
                .section("Relations")
                .tab("Details"),
        )
        .field(
            FieldDef::boolean("active")
                .required()
                .default_value(json!(true))
                .ui(UiConfig::checkbox())
                .section("Flags")
                .tab("Details"),
        )
        .field(
            FieldDef::boolean("enabled")
                .required()
                .default_value(json!(true))
                .ui(UiConfig::switch())
                .section("Flags")
                .tab("Details"),
        )
        .field(
            FieldDef::string("phone")
                .nullable()
                .phone()
                .section("Contact")
                .tab("Details"),
        )
        .field(
            FieldDef::string("email")
                .nullable()
                .email()
                .section("Contact")
                .tab("Details"),
        )
        .field(
            FieldDef::string("website")
                .nullable()
                .url()
                .section("Contact")
                .tab("Details"),
        )
        .field(
            FieldDef::json("tags")
                .nullable()
                .ui(UiConfig::tags())
                .list(false)
                .section("Content")
                .tab("Media"),
        )
        .field(
            FieldDef::text("rich_description")
                .nullable()
                .rich_text()
                .list(false)
                .section("Content")
                .tab("Media"),
        )
        .field(
            FieldDef::json("metadata")
                .nullable()
                .ui(UiConfig::json())
                .list(false)
                .section("Content")
                .tab("Media"),
        )
        .field(
            FieldDef::string("image")
                .nullable()
                .ui(UiConfig::image())
                .list(false)
                .section("Files")
                .tab("Media"),
        )
        .field(
            FieldDef::string("attachment")
                .nullable()
                .ui(UiConfig::file())
                .list(false)
                .section("Files")
                .tab("Media"),
        )
        .child_table(ChildTableDef::new("lines", "ShowcaseLine").parent_field("showcase_id"))
        .field(
            FieldDef::currency("line_total")
                .computed("SUM(lines.amount)")
                .section("Document")
                .tab("Details"),
        )
        .build()
}

pub fn showcase_line() -> EntityDef {
    EntityDef::new("ShowcaseLine")
        .label("Showcase Line")
        .label_plural("Showcase Lines")
        .table_name("showcase_lines")
        .child_of("UiShowcase", "lines")
        .field(
            FieldDef::many_to_one("showcase_id", "UiShowcase")
                .required()
                .hidden(),
        )
        .field(FieldDef::string("description").required())
        .field(
            FieldDef::integer("quantity")
                .required()
                .min(1.0)
                .default_value(json!(1)),
        )
        .field(FieldDef::currency("rate").required().min(0.0))
        .field(FieldDef::currency("amount").computed("quantity * rate"))
        .build()
}
