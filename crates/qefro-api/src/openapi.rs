use qefro_core::{EntityDef, FieldType};
use qefro_workflow::WorkflowDef;
use serde_json::{json, Value};

use crate::state::AppState;

pub fn spec(state: &AppState) -> Value {
    let mut paths = serde_json::Map::new();
    paths.insert(
        "/health".into(),
        json!({
            "get": {
                "tags": ["system"],
                "summary": "Liveness check",
                "responses": { "200": { "description": "OK" } }
            }
        }),
    );
    paths.insert(
        "/ready".into(),
        json!({
            "get": {
                "tags": ["system"],
                "summary": "Readiness check (database reachable)",
                "responses": { "200": { "description": "Ready" }, "500": { "description": "Not ready" } }
            }
        }),
    );
    paths.insert(
        "/metrics".into(),
        json!({
            "get": {
                "tags": ["system"],
                "summary": "Process metrics (no tenant PII)",
                "responses": { "200": { "description": "OK" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/meta/version".into(),
        json!({
            "get": {
                "tags": ["system"],
                "summary": "Framework and schema versions",
                "responses": { "200": { "description": "OK" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/auth/register".into(),
        json!({
            "post": {
                "tags": ["auth"],
                "summary": "Register a user and tenant",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object" } } } },
                "responses": { "200": { "description": "Token" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/auth/login".into(),
        json!({
            "post": {
                "tags": ["auth"],
                "summary": "Password login",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object" } } } },
                "responses": { "200": { "description": "Token" }, "401": { "description": "Unauthorized" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/auth/logout".into(),
        json!({
            "post": {
                "tags": ["auth"],
                "summary": "Revoke the current session",
                "security": [{ "bearerAuth": [] }],
                "responses": { "204": { "description": "Logged out" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/auth/me".into(),
        json!({
            "get": {
                "tags": ["auth"],
                "summary": "Current user",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "User" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/tenants".into(),
        json!({
            "get": {
                "tags": ["tenants"],
                "summary": "Current tenant only (never lists other tenants)",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Tenants" } }
            },
            "post": {
                "tags": ["tenants"],
                "summary": "Create tenant",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Tenant" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/tenant".into(),
        json!({
            "get": {
                "tags": ["tenants"],
                "summary": "Current tenant identity, branding, apps, features, and locale",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Tenant" } }
            },
            "patch": {
                "tags": ["tenants"],
                "summary": "Replace tenant configuration (Admin)",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Config" }, "403": { "description": "Forbidden" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/tenant/branding".into(),
        json!({
            "get": {
                "tags": ["tenants"],
                "summary": "Tenant branding",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Branding" } }
            },
            "patch": {
                "tags": ["tenants"],
                "summary": "Update branding (Admin)",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Branding" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/tenant/apps".into(),
        json!({
            "get": {
                "tags": ["tenants"],
                "summary": "Installed vs enabled applications",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Apps" } }
            },
            "patch": {
                "tags": ["tenants"],
                "summary": "Enable applications for this tenant (Admin)",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Apps" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/tenant/features".into(),
        json!({
            "get": {
                "tags": ["tenants"],
                "summary": "Feature flags",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Flags" } }
            },
            "patch": {
                "tags": ["tenants"],
                "summary": "Update feature flags (Admin)",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Flags" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/meta/ui".into(),
        json!({
            "get": {
                "tags": ["metadata"],
                "summary": "UI metadata for all entities",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "UI schema" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/operations".into(),
        json!({
            "get": {
                "tags": ["operations"],
                "summary": "List business operations the current user may invoke",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Operations" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/agent/tools".into(),
        json!({
            "get": {
                "tags": ["agent"],
                "summary": "List agent tools",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Tools" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/agent/tools/{name}/invoke".into(),
        json!({
            "post": {
                "tags": ["agent"],
                "summary": "Invoke an agent tool through the authorization pipeline",
                "security": [{ "bearerAuth": [] }],
                "parameters": [{ "name": "name", "in": "path", "required": true, "schema": { "type": "string" } }],
                "responses": { "200": { "description": "Tool result" }, "403": { "description": "Forbidden" } }
            }
        }),
    );
    paths.insert(
        "/api/v1/audit".into(),
        json!({
            "get": {
                "tags": ["audit"],
                "summary": "Tenant-scoped audit log",
                "security": [{ "bearerAuth": [] }],
                "responses": { "200": { "description": "Audit records" } }
            }
        }),
    );

    for entity in state.entities.registry().list() {
        let workflow = state.entities.workflows().for_entity(&entity.name);
        add_entity_paths(&mut paths, &entity, workflow.as_ref());
    }

    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Qefro Framework API",
            "version": "0.4.0",
            "description": "Metadata-driven, multi-tenant business application API. Tenant identity comes from the session, never from the client."
        },
        "servers": [{ "url": "/" }],
        "components": {
            "securitySchemes": {
                "bearerAuth": { "type": "http", "scheme": "bearer", "bearerFormat": "JWT" }
            }
        },
        "paths": paths
    })
}

fn add_entity_paths(
    paths: &mut serde_json::Map<String, Value>,
    entity: &EntityDef,
    workflow: Option<&WorkflowDef>,
) {
    let collection = format!("/api/v1/{}", entity.slug);
    let item = format!("/api/v1/{}/{{id}}", entity.slug);
    let schema = entity_schema(entity);
    paths.insert(
        collection,
        json!({
            "get": {
                "tags": [entity.module.clone().unwrap_or_else(|| "entities".into())],
                "summary": format!("List {}", entity.label_plural),
                "security": [{ "bearerAuth": [] }],
                "parameters": [
                    { "name": "search", "in": "query", "schema": { "type": "string" } },
                    { "name": "page", "in": "query", "schema": { "type": "integer" } },
                    { "name": "page_size", "in": "query", "schema": { "type": "integer" } },
                    { "name": "sort", "in": "query", "schema": { "type": "string" }, "description": "e.g. -created_at" }
                ],
                "responses": { "200": { "description": "Page" }, "401": { "description": "Unauthorized" } }
            },
            "post": {
                "tags": [entity.module.clone().unwrap_or_else(|| "entities".into())],
                "summary": format!("Create {}", entity.label),
                "security": [{ "bearerAuth": [] }],
                "requestBody": { "required": true, "content": { "application/json": { "schema": schema.clone() } } },
                "responses": { "201": { "description": "Created" } }
            }
        }),
    );
    paths.insert(
        item.clone(),
        json!({
            "get": {
                "tags": [entity.module.clone().unwrap_or_else(|| "entities".into())],
                "summary": format!("Get {}", entity.label),
                "security": [{ "bearerAuth": [] }],
                "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }],
                "responses": { "200": { "description": "Record" }, "404": { "description": "Not found" } }
            },
            "patch": {
                "tags": [entity.module.clone().unwrap_or_else(|| "entities".into())],
                "summary": format!("Update {}", entity.label),
                "security": [{ "bearerAuth": [] }],
                "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }],
                "requestBody": { "content": { "application/json": { "schema": schema } } },
                "responses": { "200": { "description": "Updated" } }
            },
            "delete": {
                "tags": [entity.module.clone().unwrap_or_else(|| "entities".into())],
                "summary": format!("Delete {}", entity.label),
                "security": [{ "bearerAuth": [] }],
                "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }],
                "responses": { "204": { "description": "Deleted" } }
            }
        }),
    );
    if workflow.is_some() {
        paths.insert(
            format!("{item}/transition"),
            json!({
                "post": {
                    "tags": [entity.module.clone().unwrap_or_else(|| "entities".into())],
                    "summary": format!("Transition {} workflow", entity.label),
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }],
                    "requestBody": { "required": true, "content": { "application/json": { "schema": {
                        "type": "object",
                        "required": ["transition"],
                        "properties": { "transition": { "type": "string" } }
                    } } } },
                    "responses": { "200": { "description": "Transitioned" }, "409": { "description": "Invalid transition" } }
                }
            }),
        );
    }
    paths.insert(
        format!("{item}/actions"),
        json!({
            "get": {
                "tags": [entity.module.clone().unwrap_or_else(|| "entities".into())],
                "summary": format!("List available actions for this {}", entity.label),
                "security": [{ "bearerAuth": [] }],
                "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }],
                "responses": { "200": { "description": "Actions" } }
            }
        }),
    );
    paths.insert(
        format!("{item}/actions/{{name}}"),
        json!({
            "post": {
                "tags": [entity.module.clone().unwrap_or_else(|| "entities".into())],
                "summary": format!("Execute a business operation on {}", entity.label),
                "security": [{ "bearerAuth": [] }],
                "parameters": [
                    { "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } },
                    { "name": "name", "in": "path", "required": true, "schema": { "type": "string" } }
                ],
                "responses": {
                    "200": { "description": "Updated record" },
                    "403": { "description": "Permission denied" },
                    "404": { "description": "Not found" },
                    "409": { "description": "Business rule or workflow failed" }
                }
            }
        }),
    );
}

fn entity_schema(entity: &EntityDef) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for field in entity.business_fields() {
        if !field.stores_column() {
            continue;
        }
        properties.insert(field.name.clone(), field_schema(field));
        if field.required {
            required.push(field.name.clone());
        }
    }
    json!({ "type": "object", "properties": properties, "required": required })
}

fn field_schema(field: &qefro_core::FieldDef) -> Value {
    match &field.field_type {
        FieldType::Integer => json!({ "type": "integer" }),
        FieldType::Decimal => json!({ "type": "number" }),
        FieldType::Boolean => json!({ "type": "boolean" }),
        FieldType::Enum { values } => json!({ "type": "string", "enum": values }),
        FieldType::Json => json!({ "type": "object" }),
        FieldType::Uuid | FieldType::Relation => json!({ "type": "string", "format": "uuid" }),
        FieldType::Date => json!({ "type": "string", "format": "date" }),
        FieldType::Time => json!({ "type": "string", "format": "time" }),
        FieldType::DateTime => json!({ "type": "string", "format": "date-time" }),
        FieldType::ChildTable => json!({ "type": "array", "items": { "type": "object" } }),
        _ => json!({ "type": "string" }),
    }
}
