use qefro_core::{EntityDef, EntityRegistry, FieldType, OpContext, QefroError, QefroResult};
use qefro_permissions::{Action, PermissionRegistry};
use qefro_search::{parse_query, Query};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub entity: String,
    pub operation: String,
    pub action: String,
    pub input_schema: Value,
    pub required_permissions: Vec<String>,
    pub permissions: Vec<String>,
    #[serde(default)]
    pub required_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub name: String,
    pub ok: bool,
    pub data: Value,
}

/// Tools never talk to the database. They call the same entity operations
/// used by HTTP after auth, tenant context, and permission checks.
pub trait EntityOps: Send + Sync {
    fn list(
        &self,
        ctx: &OpContext,
        entity: &str,
        query: Query,
    ) -> impl std::future::Future<Output = QefroResult<Value>> + Send;
    fn get(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: uuid::Uuid,
    ) -> impl std::future::Future<Output = QefroResult<Value>> + Send;
    fn create(
        &self,
        ctx: &OpContext,
        entity: &str,
        data: Value,
    ) -> impl std::future::Future<Output = QefroResult<Value>> + Send;
    fn update(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: uuid::Uuid,
        data: Value,
    ) -> impl std::future::Future<Output = QefroResult<Value>> + Send;
    fn delete(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: uuid::Uuid,
    ) -> impl std::future::Future<Output = QefroResult<Value>> + Send;
    fn transition(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: uuid::Uuid,
        transition: &str,
    ) -> impl std::future::Future<Output = QefroResult<Value>> + Send;
    fn execute(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: uuid::Uuid,
        name: &str,
        input: Value,
    ) -> impl std::future::Future<Output = QefroResult<Value>> + Send;
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolDef>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: ToolDef) {
        self.tools.insert(tool.name.clone(), tool);
    }

    pub fn register_entity(&mut self, entity: &EntityDef, permissions: &PermissionRegistry) {
        let _ = permissions;
        let snake = qefro_core::ident::snake_case(&entity.name);
        let plural = entity.table.clone();
        self.register(ToolDef {
            name: format!("create_{snake}"),
            description: format!("Create a {}", entity.label),
            entity: entity.name.clone(),
            operation: "create".into(),
            action: Action::Create.as_str().into(),
            input_schema: input_schema(entity, false),
            required_permissions: vec![format!("{}.create", snake)],
            permissions: vec![format!("{}:create", entity.name)],
            required_roles: Vec::new(),
        });
        self.register(ToolDef {
            name: format!("get_{snake}"),
            description: format!("Get a {} by id", entity.label),
            entity: entity.name.clone(),
            operation: "get".into(),
            action: Action::Read.as_str().into(),
            input_schema: json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string", "format": "uuid" } }
            }),
            required_permissions: vec![format!("{}.read", snake)],
            permissions: vec![format!("{}:read", entity.name)],
            required_roles: Vec::new(),
        });
        self.register(ToolDef {
            name: format!("update_{snake}"),
            description: format!("Update a {}", entity.label),
            entity: entity.name.clone(),
            operation: "update".into(),
            action: Action::Update.as_str().into(),
            input_schema: {
                let mut schema = input_schema(entity, true);
                schema["required"] = json!(["id"]);
                schema["properties"]["id"] = json!({ "type": "string", "format": "uuid" });
                schema
            },
            required_permissions: vec![format!("{}.update", snake)],
            permissions: vec![format!("{}:update", entity.name)],
            required_roles: Vec::new(),
        });
        self.register(ToolDef {
            name: format!("delete_{snake}"),
            description: format!("Delete a {}", entity.label),
            entity: entity.name.clone(),
            operation: "delete".into(),
            action: Action::Delete.as_str().into(),
            input_schema: json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string", "format": "uuid" } }
            }),
            required_permissions: vec![format!("{}.delete", snake)],
            permissions: vec![format!("{}:delete", entity.name)],
            required_roles: Vec::new(),
        });
        self.register(ToolDef {
            name: format!("find_{plural}"),
            description: format!(
                "Find {} with filters, search, and pagination",
                entity.label_plural
            ),
            entity: entity.name.clone(),
            operation: "find".into(),
            action: Action::List.as_str().into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "search": { "type": "string" },
                    "page": { "type": "integer", "minimum": 1 },
                    "page_size": { "type": "integer", "minimum": 1, "maximum": 200 },
                    "filters": { "type": "object" }
                }
            }),
            required_permissions: vec![format!("{}.list", snake)],
            permissions: vec![format!("{}:list", entity.name)],
            required_roles: Vec::new(),
        });
        if entity.workflow.is_some() {
            self.register(ToolDef {
                name: format!("transition_{snake}"),
                description: format!("Apply a workflow transition on {}", entity.label),
                entity: entity.name.clone(),
                operation: "transition".into(),
                action: Action::Update.as_str().into(),
                input_schema: json!({
                    "type": "object",
                    "required": ["id", "transition"],
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "transition": { "type": "string" }
                    }
                }),
                required_permissions: vec![format!("{}.update", snake)],
                permissions: vec![format!("{}:update", entity.name)],
                required_roles: Vec::new(),
            });
        }
    }

    pub fn register_operation(&mut self, def: &qefro_core::OperationDef) {
        let mut schema = def.input_schema.clone();
        if schema.get("type").and_then(|v| v.as_str()) != Some("object") {
            schema = json!({ "type": "object", "properties": {} });
        }
        if let Some(obj) = schema.as_object_mut() {
            let props = obj
                .entry("properties")
                .or_insert_with(|| json!({}))
                .as_object_mut();
            if let Some(props) = props {
                props
                    .entry("id")
                    .or_insert_with(|| json!({ "type": "string", "format": "uuid" }));
            }
            let required = obj
                .entry("required")
                .or_insert_with(|| json!([]));
            if let Some(arr) = required.as_array_mut() {
                if !arr.iter().any(|v| v.as_str() == Some("id")) {
                    arr.insert(0, json!("id"));
                }
            }
        }
        let description = if def.description.is_empty() {
            def.label.clone()
        } else {
            def.description.clone()
        };
        self.register(ToolDef {
            name: def.tool_name.clone(),
            description,
            entity: def.entity.clone(),
            operation: def.name.clone(),
            action: Action::Update.as_str().into(),
            input_schema: schema,
            required_permissions: vec![def.permission.clone()],
            permissions: vec![format!("{}:update", def.entity)],
            required_roles: def.roles.clone(),
        });
    }

    pub fn from_registry(entities: &EntityRegistry, permissions: &PermissionRegistry) -> Self {
        let mut tools = Self::new();
        for entity in entities.list() {
            tools.register_entity(&entity, permissions);
        }
        tools
    }

    pub fn get(&self, name: &str) -> QefroResult<&ToolDef> {
        self.tools
            .get(name)
            .ok_or_else(|| QefroError::not_found(format!("tool '{name}' not found")))
    }

    pub fn list(&self) -> Vec<&ToolDef> {
        let mut tools: Vec<_> = self.tools.values().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }

    /// Tools the current user may invoke. Still re-checked on invoke.
    pub fn available(&self, ctx: &OpContext, perms: &PermissionRegistry) -> Vec<&ToolDef> {
        self.list()
            .into_iter()
            .filter(|tool| {
                let allowed = parse_action(&tool.action)
                    .map(|action| perms.allows(&ctx.roles, &tool.entity, action))
                    .unwrap_or(false);
                if !allowed {
                    return false;
                }
                if ctx.is_admin() || tool.required_roles.is_empty() {
                    return true;
                }
                tool.required_roles.iter().any(|r| ctx.has_role(r))
            })
            .collect()
    }

    pub async fn invoke<O: EntityOps>(
        &self,
        ops: &O,
        ctx: &OpContext,
        name: &str,
        mut input: Value,
    ) -> QefroResult<ToolResult> {
        let tool = self.get(name)?.clone();
        if !tool.required_roles.is_empty()
            && !ctx.is_admin()
            && !tool.required_roles.iter().any(|r| ctx.has_role(r))
        {
            return Err(QefroError::forbidden(
                "not allowed to invoke this tool",
            ));
        }
        let action = parse_action(&tool.action)?;
        let _ = action;

        let data = match tool.operation.as_str() {
            "create" => {
                ops.create(ctx, &tool.entity, take_record(&mut input))
                    .await?
            }
            "get" | "read" => {
                let id = require_id(&input)?;
                ops.get(ctx, &tool.entity, id).await?
            }
            "update" => {
                let id = require_id(&input)?;
                if let Some(obj) = input.as_object_mut() {
                    obj.remove("id");
                }
                ops.update(ctx, &tool.entity, id, input).await?
            }
            "delete" => {
                let id = require_id(&input)?;
                ops.delete(ctx, &tool.entity, id).await?
            }
            "find" | "list" => {
                let query = query_from_input(&input);
                ops.list(ctx, &tool.entity, query).await?
            }
            "transition" => {
                let id = require_id(&input)?;
                let transition = input
                    .get("transition")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| QefroError::bad_request("transition is required"))?;
                ops.transition(ctx, &tool.entity, id, transition).await?
            }
            _ if name.starts_with("transition_") => {
                let id = require_id(&input)?;
                let transition = input
                    .get("transition")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| QefroError::bad_request("transition is required"))?;
                ops.transition(ctx, &tool.entity, id, transition).await?
            }
            _ => {
                let id = require_id(&input)?;
                if let Some(obj) = input.as_object_mut() {
                    obj.remove("id");
                }
                ops.execute(ctx, &tool.entity, id, &tool.operation, input)
                    .await?
            }
        };

        Ok(ToolResult {
            name: tool.name,
            ok: true,
            data,
        })
    }
}

fn parse_action(s: &str) -> QefroResult<Action> {
    Action::parse(s).ok_or_else(|| QefroError::internal(format!("unknown action {s}")))
}

fn require_id(input: &Value) -> QefroResult<uuid::Uuid> {
    let id = input
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| QefroError::bad_request("id is required"))?;
    uuid::Uuid::parse_str(id).map_err(|_| QefroError::bad_request("id must be a UUID"))
}

fn take_record(input: &mut Value) -> Value {
    if let Some(record) = input.get("record").cloned() {
        record
    } else {
        input.take()
    }
}

fn query_from_input(input: &Value) -> Query {
    let mut q = Query::default();
    if let Some(s) = input.get("search").and_then(|v| v.as_str()) {
        q.search = Some(s.to_string());
    }
    if let Some(p) = input.get("page").and_then(|v| v.as_u64()) {
        q.page = p as u32;
    }
    if let Some(p) = input.get("page_size").and_then(|v| v.as_u64()) {
        q.page_size = p as u32;
    }
    if let Some(obj) = input.get("filters").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            q.filters.push(qefro_search::Filter::Eq {
                field: k.clone(),
                value: v.clone(),
            });
        }
    }
    let _ = parse_query;
    q
}

fn input_schema(entity: &EntityDef, partial: bool) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for field in entity.business_fields() {
        if field.computed {
            continue;
        }
        if !field.stores_column() && !field.is_child_table() {
            continue;
        }
        properties.insert(field.name.clone(), json_schema_for_field(field));
        if field.required && !partial && !field.computed {
            required.push(Value::String(field.name.clone()));
        }
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required
    })
}

fn json_schema_for_field(field: &qefro_core::FieldDef) -> Value {
    let mut schema = match &field.field_type {
        FieldType::String | FieldType::Text => {
            json!({ "type": "string", "description": field.label })
        }
        FieldType::Date => {
            json!({ "type": "string", "format": "date", "description": field.label })
        }
        FieldType::Time => {
            json!({ "type": "string", "format": "time", "description": field.label })
        }
        FieldType::DateTime => {
            json!({ "type": "string", "format": "date-time", "description": field.label })
        }
        FieldType::Integer => json!({ "type": "integer", "description": field.label }),
        FieldType::Decimal => json!({ "type": "number", "description": field.label }),
        FieldType::Boolean => json!({ "type": "boolean", "description": field.label }),
        FieldType::Uuid | FieldType::Relation => {
            json!({ "type": "string", "format": "uuid", "description": field.label })
        }
        FieldType::Enum { values } => json!({
            "type": "string",
            "enum": values,
            "description": field.label
        }),
        FieldType::Json => json!({ "description": field.label }),
        FieldType::ChildTable => json!({
            "type": "array",
            "items": { "type": "object" },
            "description": field.label
        }),
    };
    if let Value::Object(map) = &mut schema {
        map.insert("x-widget".into(), json!(field.ui.widget));
        if field.ui.widget == "currency" {
            map.insert(
                "x-currency".into(),
                json!(field.ui.widget_options.currency),
            );
        }
        if let Some(rel) = &field.relation {
            map.insert("x-relation".into(), json!(rel.target_entity));
        }
    }
    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use qefro_core::{EntityDef, FieldDef};

    #[test]
    fn generates_tools_from_entity() {
        let entity = EntityDef::new("Reservation")
            .workflow("reservation")
            .field(FieldDef::string("status").required())
            .build();
        let mut tools = ToolRegistry::new();
        tools.register_entity(&entity, &PermissionRegistry::new());
        assert!(tools.get("create_reservation").is_ok());
        assert_eq!(tools.get("create_reservation").unwrap().operation, "create");
        assert!(tools.get("find_reservations").is_ok());
        assert!(tools.get("transition_reservation").is_ok());
        assert!(tools.get("drop_database").is_err());
        assert!(
            tools
                .get("create_reservation")
                .unwrap()
                .required_permissions
                .contains(&"reservation.create".into())
        );
    }

    #[test]
    fn agent_crate_has_no_sqlx_dependency() {
        let manifest = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let deps = manifest
            .split("[dependencies]")
            .nth(1)
            .unwrap_or(manifest);
        assert!(
            !deps.lines().any(|l| l.trim_start().starts_with("sqlx")),
            "qefro-agent [dependencies] must not include sqlx"
        );
        let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));
        let block = lock
            .split("[[package]]")
            .find(|p| {
                p.contains("name = \"qefro-agent\"")
                    && p.contains(&format!("version = \"{}\"", env!("CARGO_PKG_VERSION")))
            })
            .expect("qefro-agent package in Cargo.lock");
        let deps = block
            .split("dependencies = [")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .unwrap_or("");
        for forbidden in ["sqlx", "postgres", "tokio-postgres", "deadpool"] {
            assert!(
                !deps.contains(forbidden),
                "qefro-agent lockfile must not depend on {forbidden}: {deps}"
            );
        }
    }

    #[test]
    fn operation_tools_hide_manager_roles_from_staff() {
        use qefro_core::OperationDef;
        use qefro_permissions::PermissionGrant;
        let mut perms = PermissionRegistry::new();
        perms.grant(PermissionGrant::crud("Staff", "Reservation"));
        let mut tools = ToolRegistry::new();
        tools.register_operation(
            &OperationDef::new("explode", "Reservation").roles(&["Manager"]),
        );
        let staff = OpContext::new(uuid::Uuid::nil(), uuid::Uuid::nil(), vec!["Staff".into()]);
        let names: Vec<_> = tools
            .available(&staff, &perms)
            .into_iter()
            .map(|t| t.name.as_str())
            .collect();
        assert!(!names.contains(&"explode_reservation"));
    }

    #[test]
    fn registers_business_operation_tools() {
        use qefro_core::OperationDef;
        let mut tools = ToolRegistry::new();
        tools.register_operation(
            &OperationDef::new("confirm", "Reservation")
                .description("Confirm a pending restaurant reservation"),
        );
        let tool = tools.get("confirm_reservation").unwrap();
        assert_eq!(tool.entity, "Reservation");
        assert_eq!(tool.operation, "confirm");
        assert!(tool.required_permissions.contains(&"reservation.confirm".into()));
    }
}
