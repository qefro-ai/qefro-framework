use qefro_core::{
    CalendarViewSpec, ChildTableDef, DocumentConfig, EntityActionDef, EntityDef, EntityViews,
    FieldDef, KanbanCardSpec, KanbanViewSpec, LinkDef, NamingConfig, PrintFormat, PublicFormDef,
    UiConfig,
};
use serde_json::json;

pub fn customer() -> EntityDef {
    EntityDef::new("Customer")
        .label("Customer")
        .label_plural("Customers")
        .table_name("customers")
        .icon("users")
        .field(
            FieldDef::string("name")
                .required()
                .searchable()
                .max_length(200)
                .filterable(),
        )
        .field(
            FieldDef::string("email")
                .required()
                .email()
                .unique()
                .searchable()
                .filterable(),
        )
        .field(FieldDef::string("phone").nullable().phone().searchable())
        .field(FieldDef::text("notes").nullable().list(false).permission_level(1))
        .field(FieldDef::one_to_many("reservations", "Reservation", "customer_id"))
        .field(FieldDef::one_to_many("orders", "Order", "customer_id"))
        .build()
}

pub fn restaurant() -> EntityDef {
    EntityDef::new("Restaurant")
        .label("Restaurant")
        .label_plural("Restaurants")
        .table_name("restaurants")
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
        .field(FieldDef::one_to_many("menu_categories", "MenuCategory", "restaurant_id"))
        .build()
}

pub fn restaurant_settings() -> EntityDef {
    EntityDef::single("RestaurantSettings")
        .label("Restaurant Settings")
        .label_plural("Restaurant Settings")
        .table_name("restaurant_settings")
        .slug_name("restaurant-settings")
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
        .field(FieldDef::one_to_many("reservations", "Reservation", "table_id"))
        .field(FieldDef::one_to_many("orders", "Order", "table_id"))
        .build()
}

pub fn menu_category() -> EntityDef {
    EntityDef::new("MenuCategory")
        .label("Menu Category")
        .label_plural("Menu Categories")
        .table_name("menu_categories")
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
        .field(FieldDef::string("name").required().searchable())
        .field(FieldDef::text("description").nullable().list(false))
        .field(
            FieldDef::many_to_one("category_id", "MenuCategory")
                .required()
                .label("Category"),
        )
        .field(FieldDef::decimal("price").required().min(0.0).with_currency())
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
        .workflow("reservation")
        .attachments()
        .action(EntityActionDef::new("confirm").label("Confirm"))
        .action(EntityActionDef::new("cancel").label("Cancel").confirm("Cancel this reservation?"))
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
                .filterable(),
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
            FieldDef::integer("party_size")
                .required()
                .min(1.0)
                .max(50.0)
                .label("Guests")
                .section("Booking Details"),
        )
        .field(
            FieldDef::string("guest_name")
                .nullable()
                .searchable()
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
            ..Default::default()
        })
        .build()
}

pub fn order() -> EntityDef {
    EntityDef::new("Order")
        .label("Order")
        .label_plural("Orders")
        .table_name("orders")
        .workflow("order")
        .views(EntityViews {
            kanban: Some(KanbanViewSpec {
                group_by: Some("status".into()),
                card: Some(KanbanCardSpec {
                    title: Some("doc_no".into()),
                    subtitle: Some("status".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
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
            FieldDef::many_to_one("customer_id", "Customer")
                .nullable()
                .label("Customer"),
        )
        .field(
            FieldDef::many_to_one("table_id", "DiningTable")
                .nullable()
                .label("Table"),
        )
        .field(
            FieldDef::many_to_one("reservation_id", "Reservation")
                .nullable()
                .label("Reservation"),
        )
        .field(
            FieldDef::date("order_date")
                .required()
                .default_from("current_date")
                .filterable()
                .sortable()
                .ui(UiConfig::date()),
        )
        .field(
            FieldDef::enum_values(
                "status",
                vec![
                    "Draft",
                    "Confirmed",
                    "Preparing",
                    "Ready",
                    "Completed",
                    "Cancelled",
                ],
            )
            .required()
            .default_value(json!("Draft"))
            .filterable(),
        )
        .child_table(ChildTableDef::new("items", "OrderItem").parent_field("order_id"))
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
                .default_value(json!(0)),
        )
        .field(
            FieldDef::currency("grand_total").computed("subtotal + tax - discount"),
        )
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
        .field(
            FieldDef::many_to_one("order_id", "Order")
                .required()
                .label("Order"),
        )
        .field(FieldDef::decimal("amount").required().min(0.0).with_currency())
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
