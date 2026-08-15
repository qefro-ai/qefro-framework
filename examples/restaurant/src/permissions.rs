use qefro_permissions::{Action, PermissionGrant, ROLE_CUSTOMER, ROLE_MANAGER, ROLE_STAFF};

pub fn grants() -> Vec<PermissionGrant> {
    let mut grants = Vec::new();
    let manager_entities = [
        "Customer",
        "Restaurant",
        "Branch",
        "DiningTable",
        "MenuCategory",
        "MenuItem",
        "Reservation",
        "Order",
        "OrderItem",
        "Payment",
        "UiShowcase",
        "ShowcaseLine",
    ];
    for entity in manager_entities {
        grants.push(PermissionGrant::crud(ROLE_MANAGER, entity));
        grants.push(PermissionGrant::new(
            ROLE_MANAGER,
            entity,
            vec![Action::Export],
        ));
    }

    for entity in [
        "Reservation",
        "Order",
        "OrderItem",
        "Customer",
        "MenuItem",
        "DiningTable",
        "UiShowcase",
        "ShowcaseLine",
    ] {
        grants.push(PermissionGrant::new(
            ROLE_STAFF,
            entity,
            vec![Action::Read, Action::Update, Action::List, Action::Create],
        ));
    }
    grants.push(PermissionGrant::read(ROLE_STAFF, "Payment"));
    grants.push(PermissionGrant::read(ROLE_STAFF, "MenuCategory"));
    grants.push(PermissionGrant::read(ROLE_STAFF, "Branch"));
    grants.push(PermissionGrant::read(ROLE_STAFF, "Restaurant"));

    grants.push(PermissionGrant::read(ROLE_CUSTOMER, "Reservation"));
    grants.push(PermissionGrant::read(ROLE_CUSTOMER, "Order"));
    grants.push(PermissionGrant::read(ROLE_CUSTOMER, "MenuItem"));
    grants.push(PermissionGrant::read(ROLE_CUSTOMER, "MenuCategory"));
    grants
}
