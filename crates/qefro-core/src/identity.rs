//! Qefro identity foundation (1.1 Person, 1.2 Organization / party).
//!
//! Person (canonical individual once linked) ≠ User (optional login) ≠
//! Organization (canonical company once linked) ≠ Customer / Patient /
//! Employee / Supplier (business). These are not a second auth runtime —
//! User maps onto the existing `users` / `user_tenants` / sessions tables.
//! Business entities link with nullable `person_id` and/or `organization_id`.

use crate::entity::EntityDef;
use crate::field::{FieldDef, RelationKind};
use crate::ident::snake_case;
use crate::ui::{
    CardViewSpec, DetailViewSpec, EntityViews, ListColumnSpec, ListViewSpec, SortSpec, UiConfig,
    ViewSectionSpec,
};
use serde_json::{json, Value};

pub const USER_ENTITY: &str = "User";
pub const PERSON_ENTITY: &str = "Person";
pub const ORGANIZATION_ENTITY: &str = "Organization";
pub const USER_SLUG: &str = "users";
pub const PERSON_SLUG: &str = "people";
pub const ORGANIZATION_SLUG: &str = "organizations";
/// Conventional FK from a business entity (Customer, Patient, Employee, …)
/// to Person. When set, Person is the source of truth for name / email / phone.
pub const PERSON_LINK_FIELD: &str = "person_id";
/// Conventional FK from a business entity (Customer, Supplier, Partner, …)
/// to Organization.
pub const ORGANIZATION_LINK_FIELD: &str = "organization_id";
/// Optional discriminator when a business entity may reference Person or Organization.
pub const PARTY_TYPE_FIELD: &str = "party_type";
pub const PARTY_TYPE_PERSON: &str = "Person";
pub const PARTY_TYPE_ORGANIZATION: &str = "Organization";

/// Column / JSON keys that must never leave EntityService, meta payloads, or UI.
pub const SECRET_KEYS: &[&str] = &[
    "password",
    "password_hash",
    "token",
    "token_hash",
    "access_token",
    "refresh_token",
    "secret",
    "jwt",
    "session_token",
    "session_hash",
    "reset_token",
    "private_key",
    "storage_credentials",
];

pub fn is_secret_key(name: &str) -> bool {
    SECRET_KEYS.iter().any(|k| k.eq_ignore_ascii_case(name))
}

/// Drop password hashes, tokens, and fields marked `secret` on an entity.
/// Recurses into nested objects so audit / activity / expansions stay clean.
pub fn strip_secrets(entity: Option<&EntityDef>, record: &mut Value) {
    match record {
        Value::Object(obj) => {
            obj.retain(|k, _| !is_secret_key(k) && !k.starts_with("password"));
            if let Some(entity) = entity {
                for field in &entity.fields {
                    if field.secret {
                        obj.remove(&field.name);
                    }
                }
            }
            for value in obj.values_mut() {
                strip_secrets(None, value);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_secrets(entity, item);
            }
        }
        _ => {}
    }
}

/// Changed fields for audit / activity. Secrets are omitted.
pub fn field_changes(old: Option<&Value>, new: Option<&Value>) -> Value {
    let mut changes = serde_json::Map::new();
    let old_obj = old.and_then(|v| v.as_object());
    let new_obj = new.and_then(|v| v.as_object());
    let mut names = std::collections::BTreeSet::new();
    if let Some(obj) = old_obj {
        names.extend(obj.keys().cloned());
    }
    if let Some(obj) = new_obj {
        names.extend(obj.keys().cloned());
    }
    for name in names {
        if is_secret_key(&name)
            || name.starts_with("password")
            || name.starts_with('_')
            || name == "updated_at"
            || name == "created_at"
            || name == "tenant_id"
        {
            continue;
        }
        let before = old_obj
            .and_then(|o| o.get(&name))
            .cloned()
            .unwrap_or(Value::Null);
        let after = new_obj
            .and_then(|o| o.get(&name))
            .cloned()
            .unwrap_or(Value::Null);
        if before == after {
            continue;
        }
        changes.insert(name, json!({ "old": before, "new": after }));
    }
    Value::Object(changes)
}

pub fn contains_secret_key(record: &Value) -> bool {
    let Some(obj) = record.as_object() else {
        return false;
    };
    obj.keys().any(|k| is_secret_key(k))
}

pub fn is_person_link_field(field: &FieldDef) -> bool {
    if field.name != PERSON_LINK_FIELD {
        return false;
    }
    matches!(
        field.relation.as_ref(),
        Some(rel)
            if rel.kind == RelationKind::ManyToOne && rel.target_entity == PERSON_ENTITY
    )
}

pub fn person_backref_name(entity: &EntityDef) -> String {
    let from_slug = snake_case(&entity.slug);
    if crate::ident::assert_safe_ident(&from_slug).is_ok() {
        from_slug
    } else {
        format!("{}_records", snake_case(&entity.name))
    }
}

pub fn person_backref_field(entity: &EntityDef) -> FieldDef {
    FieldDef::one_to_many(person_backref_name(entity), &entity.name, PERSON_LINK_FIELD)
        .label(entity.label_plural.clone())
}

/// Inverse one-to-many fields so Person detail lists every business entity
/// that points at it via `person_id` (Customer, CrmCustomer, Patient, …).
pub fn person_backrefs<'a>(entities: impl IntoIterator<Item = &'a EntityDef>) -> Vec<FieldDef> {
    let mut fields = Vec::new();
    let mut used = std::collections::HashSet::new();
    for entity in entities {
        if entity.name == PERSON_ENTITY {
            continue;
        }
        if !entity.fields.iter().any(is_person_link_field) {
            continue;
        }
        let mut field = person_backref_field(entity);
        if !used.insert(field.name.clone()) {
            field = FieldDef::one_to_many(
                format!("{}_{}", field.name, snake_case(&entity.name)),
                &entity.name,
                PERSON_LINK_FIELD,
            )
            .label(entity.label_plural.clone());
            used.insert(field.name.clone());
        }
        fields.push(field);
    }
    fields
}

pub fn is_organization_link_field(field: &FieldDef) -> bool {
    if field.name != ORGANIZATION_LINK_FIELD {
        return false;
    }
    matches!(
        field.relation.as_ref(),
        Some(rel)
            if rel.kind == RelationKind::ManyToOne && rel.target_entity == ORGANIZATION_ENTITY
    )
}

pub fn organization_backref_name(entity: &EntityDef) -> String {
    person_backref_name(entity)
}

pub fn organization_backref_field(entity: &EntityDef) -> FieldDef {
    FieldDef::one_to_many(
        organization_backref_name(entity),
        &entity.name,
        ORGANIZATION_LINK_FIELD,
    )
    .label(entity.label_plural.clone())
}

pub fn organization_backrefs<'a>(
    entities: impl IntoIterator<Item = &'a EntityDef>,
) -> Vec<FieldDef> {
    let mut fields = Vec::new();
    let mut used = std::collections::HashSet::new();
    for entity in entities {
        if entity.name == ORGANIZATION_ENTITY {
            continue;
        }
        if !entity.fields.iter().any(is_organization_link_field) {
            continue;
        }
        let mut field = organization_backref_field(entity);
        if !used.insert(field.name.clone()) {
            field = FieldDef::one_to_many(
                format!("{}_{}", field.name, snake_case(&entity.name)),
                &entity.name,
                ORGANIZATION_LINK_FIELD,
            )
            .label(entity.label_plural.clone());
            used.insert(field.name.clone());
        }
        fields.push(field);
    }
    fields
}

pub fn apply_organization_backrefs(organization: &mut EntityDef, backrefs: Vec<FieldDef>) -> bool {
    apply_person_backrefs(organization, backrefs)
}

pub fn party_type_field() -> FieldDef {
    FieldDef::enum_(
        PARTY_TYPE_FIELD,
        vec![PARTY_TYPE_PERSON, PARTY_TYPE_ORGANIZATION],
    )
    .nullable()
    .label("Party type")
    .help("Person for an individual, Organization for a company. Leave empty for unlinked records.")
    .section("Identity")
    .filterable()
}

pub fn person_party_field() -> FieldDef {
    FieldDef::many_to_one(PERSON_LINK_FIELD, PERSON_ENTITY)
        .nullable()
        .label("Person")
        .help("Optional. Set when this record is a known individual. When linked, Person is the source of truth for name, email, and phone.")
        .section("Identity")
        .filterable()
}

pub fn organization_party_field() -> FieldDef {
    FieldDef::many_to_one(ORGANIZATION_LINK_FIELD, ORGANIZATION_ENTITY)
        .nullable()
        .label("Organization")
        .help("Optional. Set when this record is a company, supplier, or partner.")
        .section("Identity")
        .visible_when(PARTY_TYPE_FIELD, serde_json::json!(PARTY_TYPE_ORGANIZATION))
        .filterable()
}

/// Add optional `party_type`, `person_id`, and `organization_id` if missing.
pub fn apply_party_fields(entity: &mut EntityDef) -> bool {
    let mut added = false;
    if !entity.fields.iter().any(|f| f.name == PARTY_TYPE_FIELD) {
        let idx = entity
            .fields
            .iter()
            .position(|f| f.name == PERSON_LINK_FIELD || f.name == ORGANIZATION_LINK_FIELD)
            .unwrap_or(0);
        entity.fields.insert(idx, party_type_field());
        added = true;
    }
    if !entity.fields.iter().any(is_person_link_field) {
        let idx = entity
            .fields
            .iter()
            .position(|f| f.name == PARTY_TYPE_FIELD)
            .map(|i| i + 1)
            .unwrap_or(0);
        entity.fields.insert(idx, person_party_field());
        added = true;
    }
    if !entity.fields.iter().any(is_organization_link_field) {
        let idx = entity
            .fields
            .iter()
            .position(|f| f.name == PERSON_LINK_FIELD)
            .map(|i| i + 1)
            .or_else(|| {
                entity
                    .fields
                    .iter()
                    .position(|f| f.name == PARTY_TYPE_FIELD)
                    .map(|i| i + 1)
            })
            .unwrap_or(entity.fields.len());
        entity.fields.insert(idx, organization_party_field());
        added = true;
    }
    if added {
        entity.normalize();
    }
    added
}

fn json_id(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(v) => Some(v.to_string()),
    }
}

/// Validate Person / Organization party fields. Partial updates pass `current`.
pub fn validate_party(
    entity: &EntityDef,
    data: &Value,
    current: Option<&Value>,
) -> crate::error::QefroResult<()> {
    let has_party = entity.fields.iter().any(|f| f.name == PARTY_TYPE_FIELD)
        || entity.fields.iter().any(is_person_link_field)
        || entity.fields.iter().any(is_organization_link_field);
    if !has_party {
        return Ok(());
    }
    let merged = |field: &str| -> Option<&Value> {
        data.get(field)
            .or_else(|| current.and_then(|c| c.get(field)))
    };
    let party_type = merged(PARTY_TYPE_FIELD).and_then(|v| v.as_str());
    let person = json_id(merged(PERSON_LINK_FIELD));
    let org = json_id(merged(ORGANIZATION_LINK_FIELD));
    if person.is_some() && org.is_some() {
        return Err(crate::error::QefroError::bad_request(
            "set person_id or organization_id, not both",
        ));
    }
    if let Some(kind) = party_type {
        if kind != PARTY_TYPE_PERSON && kind != PARTY_TYPE_ORGANIZATION {
            return Err(crate::error::QefroError::bad_request(
                "party_type must be Person or Organization",
            ));
        }
        if kind == PARTY_TYPE_PERSON && org.is_some() {
            return Err(crate::error::QefroError::bad_request(
                "organization_id is not valid when party_type is Person",
            ));
        }
        if kind == PARTY_TYPE_ORGANIZATION && person.is_some() {
            return Err(crate::error::QefroError::bad_request(
                "person_id is not valid when party_type is Organization",
            ));
        }
    }
    Ok(())
}

/// Attach discovered `person_id` inverses onto Person. Returns whether fields were added.
pub fn apply_person_backrefs(person: &mut EntityDef, backrefs: Vec<FieldDef>) -> bool {
    let existing_targets: std::collections::HashSet<String> = person
        .fields
        .iter()
        .filter_map(|f| {
            let rel = f.relation.as_ref()?;
            (rel.kind == RelationKind::OneToMany).then(|| rel.target_entity.clone())
        })
        .collect();
    let mut existing_names: std::collections::HashSet<String> =
        person.fields.iter().map(|f| f.name.clone()).collect();
    let mut added = false;
    for mut field in backrefs {
        let Some(target) = field.relation.as_ref().map(|rel| rel.target_entity.clone()) else {
            continue;
        };
        if existing_targets.contains(&target) {
            continue;
        }
        if existing_names.contains(&field.name) {
            field.name = format!("{}_{}", field.name, snake_case(&target));
        }
        existing_names.insert(field.name.clone());
        person.fields.push(field);
        added = true;
    }
    if added {
        person.normalize();
    }
    added
}

/// Tenant-scoped individual. Optional `user_id` links a login; most people
/// never have one (walk-in customer, family member, vendor contact).
pub fn person_entity() -> EntityDef {
    EntityDef::new(PERSON_ENTITY)
        .label("Person")
        .label_plural("People")
        .table_name("people")
        .slug_name(PERSON_SLUG)
        .icon("user")
        .description("Canonical individual identity once linked. Not a login (User) and not a Customer/Patient/Employee record.")
        .field(
            FieldDef::string("name")
                .required()
                .searchable()
                .max_length(200)
                .filterable(),
        )
        .field(
            FieldDef::string("email")
                .nullable()
                .email()
                .searchable()
                .filterable(),
        )
        .field(FieldDef::string("phone").nullable().phone().searchable())
        .field(
            FieldDef::many_to_one("user_id", USER_ENTITY)
                .nullable()
                .label("Login")
                .help("Optional. Only set when this person should sign in."),
        )
        .field(
            FieldDef::boolean("create_account")
                .ephemeral()
                .label("Create login")
                .help("Create a User for this person. Requires permission to create users.")
                .default_value(json!(false))
                .section("Login"),
        )
        .field(
            FieldDef::string("password")
                .write_only()
                .min_length(8)
                .ui(UiConfig::password())
                .label("Password")
                .help("Set when creating a login. Never stored on Person and never returned.")
                .section("Login")
                .visible_when("create_account", json!(true)),
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
                        field: "user_id".into(),
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
            card: Some(CardViewSpec {
                title: Some("name".into()),
                subtitle: Some("email".into()),
                fields: vec!["phone".into(), "user_id".into()],
                ..Default::default()
            }),
            detail: Some(DetailViewSpec {
                sections: vec![
                    ViewSectionSpec::new("Identity").fields(&["name", "email", "phone"]),
                    ViewSectionSpec::new("Login").fields(&["user_id"]),
                ],
            }),
            ..Default::default()
        })
        .build()
}

/// Tenant-scoped company / legal entity. Not a User and not a Customer/Supplier
/// business record. Business entities optionally link via `organization_id`.
pub fn organization_entity() -> EntityDef {
    EntityDef::new(ORGANIZATION_ENTITY)
        .label("Organization")
        .label_plural("Organizations")
        .table_name("organizations")
        .slug_name(ORGANIZATION_SLUG)
        .icon("building")
        .description("Canonical organization identity once linked. Not a login (User) and not a Customer/Supplier/Partner record.")
        .field(
            FieldDef::string("name")
                .required()
                .searchable()
                .max_length(200)
                .filterable(),
        )
        .field(
            FieldDef::string("legal_name")
                .nullable()
                .searchable()
                .max_length(200)
                .label("Legal name"),
        )
        .field(
            FieldDef::string("email")
                .nullable()
                .email()
                .searchable()
                .filterable(),
        )
        .field(FieldDef::string("phone").nullable().phone().searchable())
        .field(FieldDef::string("website").nullable().url())
        .field(
            FieldDef::text("address")
                .nullable()
                .list(false)
                .label("Address"),
        )
        .field(
            FieldDef::string("logo")
                .nullable()
                .image()
                .list(false)
                .label("Logo"),
        )
        .field(
            FieldDef::boolean("enabled")
                .required()
                .default_value(json!(true))
                .filterable()
                .label("Enabled"),
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
                        field: "legal_name".into(),
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
                        field: "enabled".into(),
                        width: None,
                        widget: None,
                    },
                ],
                default_sort: Some(SortSpec {
                    field: "name".into(),
                    direction: Some("asc".into()),
                }),
                ..Default::default()
            }),
            card: Some(CardViewSpec {
                title: Some("name".into()),
                subtitle: Some("email".into()),
                fields: vec!["phone".into(), "website".into(), "enabled".into()],
                ..Default::default()
            }),
            detail: Some(DetailViewSpec {
                sections: vec![
                    ViewSectionSpec::new("Identity").fields(&[
                        "name",
                        "legal_name",
                        "email",
                        "phone",
                        "website",
                    ]),
                    ViewSectionSpec::new("Profile").fields(&["address", "logo", "enabled"]),
                ],
            }),
            ..Default::default()
        })
        .build()
}

/// Login principal. Table and secrets live in qefro-auth; EntityService never
/// selects `password_hash` or session tokens.
pub fn user_entity() -> EntityDef {
    EntityDef::new(USER_ENTITY)
        .label("User")
        .label_plural("Users")
        .table_name("users")
        .slug_name(USER_SLUG)
        .icon("key")
        .description("Authentication account for this tenant. Not a Customer, Patient, or Person.")
        .no_tenant()
        .no_soft_delete()
        .skip_ddl()
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
                .searchable()
                .filterable(),
        )
        .field(
            FieldDef::boolean("enabled")
                .required()
                .default_value(json!(true))
                .filterable()
                .label("Enabled"),
        )
        .field(
            FieldDef::json("roles")
                .ephemeral()
                .tags()
                .label("Roles")
                .help("Tenant membership roles. Assignment requires User update permission.")
                .default_value(json!(["Staff"])),
        )
        .field(
            FieldDef::string("password")
                .write_only()
                .min_length(8)
                .ui(UiConfig::password())
                .label("Password")
                .help("Write-only. Required on create; omit on update to leave unchanged."),
        )
        .field(FieldDef::one_to_many("people", PERSON_ENTITY, "user_id"))
        .build()
}

pub fn identity_entities() -> Vec<EntityDef> {
    vec![person_entity(), organization_entity(), user_entity()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_is_tenant_owned_and_has_optional_user() {
        let person = person_entity();
        assert!(person.tenant_owned);
        assert!(!person.skip_ddl);
        assert_eq!(person.slug, PERSON_SLUG);
        let user = person.get_field("user_id").unwrap();
        assert!(!user.required);
        assert!(!person.fields.iter().any(|f| f.name == "password_hash"));
        assert!(person.get_field("create_account").unwrap().ephemeral);
        assert!(person.get_field("password").unwrap().secret);
        assert!(!person.get_field("password").unwrap().stores_column());
    }

    #[test]
    fn user_skips_ddl_and_hides_secrets() {
        let user = user_entity();
        assert!(user.skip_ddl);
        assert!(!user.tenant_owned);
        assert!(!user.soft_delete);
        assert!(user.get_field("password").unwrap().secret);
        assert!(!user.get_field("password").unwrap().stores_column());
        assert!(user.get_field("roles").unwrap().ephemeral);
        assert!(!user
            .fields
            .iter()
            .any(|f| is_secret_key(&f.name) && f.name != "password"));
        let mut record = json!({
            "id": "x",
            "email": "a@b.c",
            "password_hash": "argon2",
            "password": "secret",
            "token_hash": "abc"
        });
        strip_secrets(Some(&user), &mut record);
        assert!(record.get("password_hash").is_none());
        assert!(record.get("password").is_none());
        assert!(record.get("token_hash").is_none());
        assert_eq!(record["email"], "a@b.c");
    }

    #[test]
    fn ui_schema_stays_at_one() {
        assert_eq!(person_entity().to_ui_meta().schema_version, "1");
        assert_eq!(user_entity().to_ui_meta().schema_version, "1");
        let meta = user_entity().to_ui_meta();
        assert!(!meta.fields.iter().any(|f| f.name == "password_hash"));
        let password = meta.fields.iter().find(|f| f.name == "password").unwrap();
        assert!(password.secret);
        assert!(!password.list_visible);
        assert!(!password.detail_visible);
        assert!(password.form_visible);
        let person_ui = person_entity().to_ui_meta();
        assert_eq!(person_ui.schema_version, "1");
        assert!(person_ui.views.is_some());
    }

    #[test]
    fn person_backrefs_follow_person_id_convention() {
        let customer = EntityDef::new("ShopCustomer")
            .table_name("id_shop_customers")
            .slug_name("shop-customers")
            .label_plural("Shop customers")
            .field(FieldDef::string("name").required())
            .field(
                FieldDef::many_to_one(PERSON_LINK_FIELD, PERSON_ENTITY)
                    .nullable()
                    .label("Person"),
            )
            .build();
        assert!(is_person_link_field(
            customer.get_field(PERSON_LINK_FIELD).unwrap()
        ));
        let refs = person_backrefs([&customer]);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "shop_customers");
        let rel = refs[0].relation.as_ref().unwrap();
        assert_eq!(rel.target_entity, "ShopCustomer");
        assert_eq!(rel.inverse_field.as_deref(), Some(PERSON_LINK_FIELD));
        let mut person = person_entity();
        assert!(apply_person_backrefs(&mut person, refs));
        assert!(person.get_field("shop_customers").is_some());
        assert!(!apply_person_backrefs(
            &mut person,
            person_backrefs([&customer])
        ));
    }

    #[test]
    fn organization_is_tenant_owned_and_has_no_login() {
        let org = organization_entity();
        assert!(org.tenant_owned);
        assert!(!org.skip_ddl);
        assert_eq!(org.slug, ORGANIZATION_SLUG);
        assert!(org.get_field("legal_name").is_some());
        assert!(org.get_field("website").is_some());
        assert!(org.get_field("enabled").unwrap().required);
        assert!(!org.fields.iter().any(|f| f.name == "user_id"));
        assert!(!org.fields.iter().any(|f| is_secret_key(&f.name)));
        assert_eq!(org.to_ui_meta().schema_version, "1");
    }

    #[test]
    fn organization_backrefs_follow_organization_id_convention() {
        let supplier = EntityDef::new("Supplier")
            .table_name("suppliers")
            .slug_name("suppliers")
            .label_plural("Suppliers")
            .field(FieldDef::string("name").required())
            .field(
                FieldDef::many_to_one(ORGANIZATION_LINK_FIELD, ORGANIZATION_ENTITY)
                    .nullable()
                    .label("Organization"),
            )
            .build();
        assert!(is_organization_link_field(
            supplier.get_field(ORGANIZATION_LINK_FIELD).unwrap()
        ));
        let refs = organization_backrefs([&supplier]);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "suppliers");
        let mut org = organization_entity();
        assert!(apply_organization_backrefs(&mut org, refs));
        assert!(org.get_field("suppliers").is_some());
    }

    #[test]
    fn party_fields_and_validation() {
        let mut customer = EntityDef::new("PartyCustomer")
            .table_name("party_customers")
            .slug_name("party-customers")
            .field(FieldDef::string("name").required())
            .build();
        assert!(apply_party_fields(&mut customer));
        assert!(customer.get_field(PARTY_TYPE_FIELD).is_some());
        assert!(is_person_link_field(
            customer.get_field(PERSON_LINK_FIELD).unwrap()
        ));
        assert!(is_organization_link_field(
            customer.get_field(ORGANIZATION_LINK_FIELD).unwrap()
        ));
        assert!(!apply_party_fields(&mut customer));
        validate_party(
            &customer,
            &json!({ "party_type": "Person", "person_id": "11111111-1111-1111-1111-111111111111" }),
            None,
        )
        .unwrap();
        assert!(validate_party(
            &customer,
            &json!({
                "person_id": "11111111-1111-1111-1111-111111111111",
                "organization_id": "22222222-2222-2222-2222-222222222222"
            }),
            None,
        )
        .is_err());
        assert!(validate_party(
            &customer,
            &json!({
                "party_type": "Person",
                "organization_id": "22222222-2222-2222-2222-222222222222"
            }),
            None,
        )
        .is_err());
        let changes = field_changes(
            Some(&json!({ "status": "Lead", "password_hash": "x", "phone": "1" })),
            Some(&json!({ "status": "Qualified", "password_hash": "y", "phone": "2" })),
        );
        assert_eq!(changes["status"]["old"], "Lead");
        assert_eq!(changes["status"]["new"], "Qualified");
        assert!(changes.get("password_hash").is_none());
    }
}
