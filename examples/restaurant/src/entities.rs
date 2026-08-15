use qefro_core::{EntityDef, FieldDef};
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
        .field(FieldDef::string("phone").nullable().searchable())
        .field(FieldDef::text("notes").nullable().list(false))
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
                .default_value(json!("UTC")),
        )
        .field(FieldDef::string("phone").nullable())
        .field(FieldDef::one_to_many("branches", "Branch", "restaurant_id"))
        .field(FieldDef::one_to_many("menu_categories", "MenuCategory", "restaurant_id"))
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
        .field(FieldDef::decimal("price").required().min(0.0))
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
        .field(
            FieldDef::many_to_one("customer_id", "Customer")
                .required()
                .label("Customer"),
        )
        .field(
            FieldDef::many_to_one("table_id", "DiningTable")
                .required()
                .label("Table"),
        )
        .field(FieldDef::date("reservation_date").required().filterable())
        .field(FieldDef::string("reservation_time").required())
        .field(
            FieldDef::integer("party_size")
                .required()
                .min(1.0)
                .max(50.0),
        )
        .field(
            FieldDef::enum_values(
                "status",
                vec!["Pending", "Confirmed", "Seated", "Completed", "Cancelled"],
            )
            .required()
            .default_value(json!("Pending"))
            .filterable(),
        )
        .field(FieldDef::text("notes").nullable().list(false))
        .field(FieldDef::one_to_many("orders", "Order", "reservation_id"))
        .build()
}

pub fn order() -> EntityDef {
    EntityDef::new("Order")
        .label("Order")
        .label_plural("Orders")
        .table_name("orders")
        .workflow("order")
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
        .field(
            FieldDef::decimal("total")
                .nullable()
                .min(0.0)
                .default_value(json!(0)),
        )
        .field(FieldDef::text("notes").nullable().list(false))
        .field(FieldDef::one_to_many("items", "OrderItem", "order_id"))
        .field(FieldDef::one_to_many("payments", "Payment", "order_id"))
        .build()
}

pub fn order_item() -> EntityDef {
    EntityDef::new("OrderItem")
        .label("Order Item")
        .label_plural("Order Items")
        .table_name("order_items")
        .field(
            FieldDef::many_to_one("order_id", "Order")
                .required()
                .label("Order"),
        )
        .field(
            FieldDef::many_to_one("menu_item_id", "MenuItem")
                .required()
                .label("Menu Item"),
        )
        .field(
            FieldDef::integer("quantity")
                .required()
                .min(1.0)
                .default_value(json!(1)),
        )
        .field(FieldDef::decimal("unit_price").required().min(0.0))
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
        .field(FieldDef::decimal("amount").required().min(0.0))
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
