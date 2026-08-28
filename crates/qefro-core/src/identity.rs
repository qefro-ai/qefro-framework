//! Qefro 1.1 identity foundation.
//!
//! Person (canonical identity once linked) ≠ User (optional login) ≠
//! Customer / Patient / Employee (business). These are not a second auth
//! runtime — User maps onto the existing `users` / `user_tenants` / sessions
//! tables. Business entities link with nullable `person_id`; Person is then
//! the source of truth for name / email / phone. Customer columns stay.

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
pub const USER_SLUG: &str = "users";
pub const PERSON_SLUG: &str = "people";
/// Conventional FK from a business entity (Customer, Patient, Employee, …)
/// to Person. When set, Person is the source of truth for name / email / phone.
pub const PERSON_LINK_FIELD: &str = "person_id";

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
];

pub fn is_secret_key(name: &str) -> bool {
    SECRET_KEYS.iter().any(|k| k.eq_ignore_ascii_case(name))
}

/// Drop password hashes, tokens, and fields marked `secret` on an entity.
pub fn strip_secrets(entity: Option<&EntityDef>, record: &mut Value) {
    let Some(obj) = record.as_object_mut() else {
        return;
    };
    obj.retain(|k, _| !is_secret_key(k) && !k.starts_with("password"));
    if let Some(entity) = entity {
        for field in &entity.fields {
            if field.secret {
                obj.remove(&field.name);
            }
        }
    }
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
                    ViewSectionSpec {
                        title: "Identity".into(),
                        fields: vec!["name".into(), "email".into(), "phone".into()],
                        visible_when: None,
                    },
                    ViewSectionSpec {
                        title: "Login".into(),
                        fields: vec!["user_id".into()],
                        visible_when: None,
                    },
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
    vec![person_entity(), user_entity()]
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
}
