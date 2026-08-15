use qefro_permissions::{Action, PermissionGrant, ROLE_CUSTOMER, ROLE_MANAGER, ROLE_STAFF};

pub fn grants() -> Vec<PermissionGrant> {
    let mut grants = Vec::new();
    for entity in [
        "CrmCustomer",
        "Lead",
        "Contact",
        "Opportunity",
        "OpportunityItem",
        "Activity",
    ] {
        grants.push(PermissionGrant::crud(ROLE_MANAGER, entity));
        grants.push(PermissionGrant::new(
            ROLE_MANAGER,
            entity,
            vec![Action::Export],
        ));
        grants.push(PermissionGrant::new(
            ROLE_STAFF,
            entity,
            vec![Action::Read, Action::Update, Action::List, Action::Create],
        ));
    }
    grants.push(PermissionGrant::read(ROLE_CUSTOMER, "Opportunity"));
    grants
}
