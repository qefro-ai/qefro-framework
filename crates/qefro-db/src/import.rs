//! Generic CSV/JSON import. Every row is written through EntityService.
//!
//! Import is another way of writing business data — never a bypass around
//! validation, permissions, workflow, relations, audit, or tenant isolation.

use crate::activity::TYPE_SYSTEM;
use crate::attachments::{max_upload_bytes, sanitize_filename};
use crate::blobs::{BlobMeta, BlobMetaStore};
use crate::bulk::csv_escape;
use crate::jobs::JobHandler;
use crate::notifications::{InAppNotification, NotificationStore};
use crate::service::EntityService;
use async_trait::async_trait;
use chrono::Utc;
use qefro_core::{
    is_secret_key, strip_secrets, BlobStore, EntityDef, FieldDef, FieldType, OpContext, QefroError,
    QefroResult,
};
use qefro_permissions::Action;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

pub const IMPORT_RUN_JOB: &str = "import.run";

const DEFAULT_MAX_ROWS: usize = 100_000;
const DEFAULT_MAX_COLUMNS: usize = 64;
const SYNC_MAX_ROWS: usize = 200;
const SYNC_MAX_BYTES: usize = 256 * 1024;
const PREVIEW_SAMPLE: usize = 25;
const DEFAULT_BATCH: usize = 100;
const PROTECTED: &[&str] = &[
    "id",
    "tenant_id",
    "created_at",
    "created_by",
    "updated_at",
    "updated_by",
    "deleted_at",
    "archived_at",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMapping {
    pub column: String,
    pub field: Option<String>,
    #[serde(default)]
    pub default: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    #[default]
    Create,
    Update,
    Upsert,
}

impl ImportMode {
    pub fn parse(raw: Option<&str>) -> QefroResult<Self> {
        match raw.unwrap_or("create").trim().to_ascii_lowercase().as_str() {
            "" | "create" | "create_only" => Ok(Self::Create),
            "update" | "update_only" => Ok(Self::Update),
            "upsert" => Ok(Self::Upsert),
            other => Err(QefroError::bad_request(format!(
                "unknown import mode '{other}'"
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Upsert => "upsert",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DuplicatePolicy {
    #[default]
    Fail,
    Skip,
    Update,
}

impl DuplicatePolicy {
    pub fn parse(raw: Option<&str>) -> QefroResult<Self> {
        match raw.unwrap_or("fail").trim().to_ascii_lowercase().as_str() {
            "" | "fail" | "fail_row" => Ok(Self::Fail),
            "skip" | "skip_row" => Ok(Self::Skip),
            "update" | "update_existing" => Ok(Self::Update),
            other => Err(QefroError::bad_request(format!(
                "unknown duplicate policy '{other}'"
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Update => "update",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    #[default]
    Csv,
    Json,
}

impl ImportFormat {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("csv").trim().to_ascii_lowercase().as_str() {
            "json" | "application/json" => Self::Json,
            _ => Self::Csv,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportOptions {
    #[serde(default)]
    pub mapping: Vec<ImportMapping>,
    #[serde(default)]
    pub mode: ImportMode,
    #[serde(default)]
    pub duplicate_policy: DuplicatePolicy,
    #[serde(default)]
    pub match_field: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub batch_size: usize,
    #[serde(default)]
    pub strict: bool,
    #[serde(default)]
    pub format: ImportFormat,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportFieldInfo {
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub unique: bool,
    pub relation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub rows: usize,
    pub valid: usize,
    pub invalid: usize,
    pub warnings: usize,
    pub columns: Vec<String>,
    pub mapping: Vec<ImportMapping>,
    pub fields: Vec<ImportFieldInfo>,
    pub ignored: Vec<String>,
    pub errors: Vec<ImportRowError>,
    pub sample: Vec<Value>,
    pub match_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowError {
    pub row: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub warnings: usize,
    pub processed: usize,
    pub total: usize,
    pub dry_run: bool,
    pub async_job: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<Uuid>,
    pub status: String,
    pub errors: Vec<ImportRowError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_report_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ImportJobRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub entity: String,
    pub status: String,
    pub mode: String,
    pub duplicate_policy: String,
    pub match_field: Option<String>,
    pub mapping: Value,
    pub format: String,
    pub dry_run: bool,
    pub strict: bool,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub blob_key: Option<String>,
    pub error_report_key: Option<String>,
    pub total_rows: i32,
    pub processed: i32,
    pub created_count: i32,
    pub updated_count: i32,
    pub skipped_count: i32,
    pub failed_count: i32,
    pub warning_count: i32,
    pub checkpoint: i32,
    pub last_error: Option<String>,
    pub cancel_requested: bool,
    pub retry_count: i32,
    pub idempotency_key: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

impl ImportJobRecord {
    pub fn to_client_json(&self) -> Value {
        json!({
            "id": self.id,
            "entity": self.entity,
            "status": self.status,
            "mode": self.mode,
            "duplicate_policy": self.duplicate_policy,
            "match_field": self.match_field,
            "mapping": self.mapping,
            "format": self.format,
            "dry_run": self.dry_run,
            "strict": self.strict,
            "filename": self.filename,
            "total": self.total_rows,
            "processed": self.processed,
            "created": self.created_count,
            "updated": self.updated_count,
            "skipped": self.skipped_count,
            "failed": self.failed_count,
            "warnings": self.warning_count,
            "imported": self.created_count + self.updated_count,
            "error_report_key": self.error_report_key,
            "last_error": self.last_error,
            "cancel_requested": self.cancel_requested,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }
}

pub struct ImportRunJob {
    entities: OnceLock<Arc<EntityService>>,
    blobs: OnceLock<Arc<dyn BlobStore>>,
    blob_meta: OnceLock<Arc<BlobMetaStore>>,
    notifications: OnceLock<Arc<NotificationStore>>,
}

impl ImportRunJob {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entities: OnceLock::new(),
            blobs: OnceLock::new(),
            blob_meta: OnceLock::new(),
            notifications: OnceLock::new(),
        })
    }

    pub fn bind(
        &self,
        entities: Arc<EntityService>,
        blobs: Arc<dyn BlobStore>,
        blob_meta: Arc<BlobMetaStore>,
        notifications: Arc<NotificationStore>,
    ) {
        let _ = self.entities.set(entities);
        let _ = self.blobs.set(blobs);
        let _ = self.blob_meta.set(blob_meta);
        let _ = self.notifications.set(notifications);
    }
}

#[async_trait]
impl JobHandler for ImportRunJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        let Some(entities) = self.entities.get() else {
            return Err(QefroError::internal("import job is not bound"));
        };
        let Some(blobs) = self.blobs.get() else {
            return Err(QefroError::internal("import blob store is not bound"));
        };
        let job_id = payload
            .get("import_job_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::bad_request("import_job_id is required"))?;
        let user_ctx = restore_user_ctx(ctx, payload);
        entities
            .process_import_job(
                &user_ctx,
                job_id,
                blobs.as_ref(),
                self.blob_meta.get().map(|s| s.as_ref()),
                self.notifications.get().map(|s| s.as_ref()),
            )
            .await?;
        Ok(())
    }
}

fn restore_user_ctx(worker: &OpContext, payload: &Value) -> OpContext {
    let roles = payload
        .get("roles")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| vec!["Staff".into()]);
    let user_id = payload
        .get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or(worker.user_id);
    let mut ctx = OpContext::new(worker.tenant_id, user_id, roles);
    ctx.request_id = worker.request_id;
    ctx.enabled_apps = worker.enabled_apps.clone();
    ctx.actor_name = payload
        .get("actor_name")
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
        .or(worker.actor_name.clone());
    ctx.source = "user".into();
    ctx
}

pub fn max_import_bytes() -> usize {
    max_upload_bytes().clamp(1024, 32 * 1024 * 1024) as usize
}

pub fn max_import_rows() -> usize {
    std::env::var("QEFRO_MAX_IMPORT_ROWS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_MAX_ROWS)
}

pub fn max_import_columns() -> usize {
    std::env::var("QEFRO_MAX_IMPORT_COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_MAX_COLUMNS)
}

pub fn decode_text(bytes: &[u8]) -> QefroResult<String> {
    let slice = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };
    String::from_utf8(slice.to_vec())
        .map_err(|_| QefroError::bad_request("invalid encoding: file must be UTF-8"))
}

pub fn parse_csv(text: &str) -> QefroResult<(Vec<String>, Vec<Map<String, Value>>)> {
    let mut rows = Vec::new();
    let (headers, iter) = csv_header_iter(text)?;
    for rec in iter {
        let rec = rec.map_err(|e| QefroError::bad_request(format!("csv: {e}")))?;
        rows.push(record_to_map(&headers, &rec));
        if rows.len() > max_import_rows() {
            return Err(QefroError::payload_too_large(format!(
                "import exceeds {} rows",
                max_import_rows()
            )));
        }
    }
    Ok((headers, rows))
}

fn csv_header_iter(text: &str) -> QefroResult<(Vec<String>, csv::StringRecordsIntoIter<&[u8]>)> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| QefroError::bad_request(format!("csv: {e}")))?
        .iter()
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();
    validate_headers(&headers)?;
    Ok((headers, reader.into_records()))
}

fn record_to_map(headers: &[String], rec: &csv::StringRecord) -> Map<String, Value> {
    let mut map = Map::new();
    for (i, col) in headers.iter().enumerate() {
        let val = rec.get(i).unwrap_or("").trim();
        map.insert(col.clone(), json!(val));
    }
    map
}

pub fn parse_json(text: &str) -> QefroResult<(Vec<String>, Vec<Map<String, Value>>)> {
    let value: Value = serde_json::from_str(text)
        .map_err(|e| QefroError::bad_request(format!("invalid JSON: {e}")))?;
    let items = match value {
        Value::Array(items) => items,
        Value::Object(_) => {
            return Err(QefroError::bad_request(
                "JSON import expects an array of objects",
            ));
        }
        _ => {
            return Err(QefroError::bad_request("invalid JSON: expected an array"));
        }
    };
    if items.len() > max_import_rows() {
        return Err(QefroError::payload_too_large(format!(
            "import exceeds {} rows",
            max_import_rows()
        )));
    }
    let mut columns: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for item in items {
        let obj = item
            .as_object()
            .ok_or_else(|| QefroError::bad_request("JSON import expects an array of objects"))?;
        reject_nested(obj)?;
        for key in obj.keys() {
            if seen.insert(key.clone()) {
                columns.push(key.clone());
            }
        }
        if columns.len() > max_import_columns() {
            return Err(QefroError::payload_too_large(format!(
                "import exceeds {} columns",
                max_import_columns()
            )));
        }
        rows.push(obj.clone());
    }
    Ok((columns, rows))
}

fn reject_nested(obj: &Map<String, Value>) -> QefroResult<()> {
    for (key, value) in obj {
        match value {
            Value::Object(_) => {
                return Err(QefroError::bad_request(format!(
                    "Nested relation import is not supported ({key})"
                )));
            }
            Value::Array(items) if items.iter().any(|v| v.is_object() || v.is_array()) => {
                return Err(QefroError::bad_request(
                    "Nested relation import is not supported.",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_headers(headers: &[String]) -> QefroResult<()> {
    if headers.is_empty() || headers.iter().all(|h| h.is_empty()) {
        return Err(QefroError::bad_request("missing headers"));
    }
    if headers.len() > max_import_columns() {
        return Err(QefroError::payload_too_large(format!(
            "import exceeds {} columns",
            max_import_columns()
        )));
    }
    let mut seen = HashSet::new();
    for header in headers {
        if header.is_empty() {
            return Err(QefroError::bad_request("missing headers"));
        }
        if !seen.insert(header.to_ascii_lowercase()) {
            return Err(QefroError::bad_request(format!(
                "duplicate header '{header}'"
            )));
        }
    }
    Ok(())
}

pub fn parse_source(
    text: &str,
    format: ImportFormat,
) -> QefroResult<(Vec<String>, Vec<Map<String, Value>>)> {
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Err(QefroError::bad_request("import file is empty"));
    }
    match format {
        ImportFormat::Csv => parse_csv(text),
        ImportFormat::Json => parse_json(text),
    }
}

pub fn importable_fields<'a>(entity: &'a EntityDef, wf_field: Option<&str>) -> Vec<&'a FieldDef> {
    entity
        .business_fields()
        .iter()
        .filter(|field| is_importable(field, wf_field))
        .collect()
}

fn is_importable(field: &FieldDef, wf_field: Option<&str>) -> bool {
    if field.system || field.computed || field.secret || field.server_managed || field.ephemeral {
        return false;
    }
    if field.is_child_table() || !field.stores_column() {
        return false;
    }
    if is_secret_key(&field.name) || field.name.starts_with("password") {
        return false;
    }
    if PROTECTED
        .iter()
        .any(|name| field.name.eq_ignore_ascii_case(name))
    {
        return false;
    }
    if wf_field.is_some_and(|name| field.name == name) {
        return false;
    }
    true
}

pub fn field_info(entity: &EntityDef, wf_field: Option<&str>) -> Vec<ImportFieldInfo> {
    importable_fields(entity, wf_field)
        .into_iter()
        .map(|field| ImportFieldInfo {
            name: field.name.clone(),
            label: field.label.clone(),
            field_type: field.field_type.as_str().to_string(),
            required: field.required,
            unique: field.unique,
            relation: field.relation.as_ref().map(|r| r.target_entity.clone()),
        })
        .collect()
}

pub fn suggest_mapping(
    entity: &EntityDef,
    columns: &[String],
    wf_field: Option<&str>,
) -> Vec<ImportMapping> {
    let fields = importable_fields(entity, wf_field);
    let mut label_counts: HashMap<String, usize> = HashMap::new();
    for field in &fields {
        *label_counts
            .entry(field.label.to_ascii_lowercase())
            .or_default() += 1;
    }
    columns
        .iter()
        .map(|column| {
            let key = column.trim();
            let lower = key.to_ascii_lowercase();
            let by_name: Vec<_> = fields
                .iter()
                .filter(|f| f.name.eq_ignore_ascii_case(key))
                .collect();
            let by_label: Vec<_> = fields
                .iter()
                .filter(|f| f.label.eq_ignore_ascii_case(key))
                .collect();
            let mapped = if by_name.len() == 1 {
                if by_label.len() > 1
                    || (by_label.len() == 1 && by_label[0].name != by_name[0].name)
                {
                    None
                } else {
                    Some(by_name[0].name.clone())
                }
            } else if by_name.is_empty()
                && by_label.len() == 1
                && label_counts.get(&lower).copied().unwrap_or(0) == 1
            {
                Some(by_label[0].name.clone())
            } else {
                None
            };
            ImportMapping {
                column: column.clone(),
                field: mapped,
                default: None,
            }
        })
        .collect()
}

pub fn apply_mapping(
    entity: &EntityDef,
    row: &Map<String, Value>,
    mapping: &[ImportMapping],
) -> Value {
    let wf = None;
    let maps: Vec<ImportMapping> = if mapping.is_empty() {
        let columns: Vec<String> = row.keys().cloned().collect();
        suggest_mapping(entity, &columns, wf)
    } else {
        mapping.to_vec()
    };
    let mut out = Map::new();
    for map in &maps {
        let Some(field_name) = map.field.as_deref() else {
            continue;
        };
        if field_name.is_empty() || field_name.eq_ignore_ascii_case("ignore") {
            continue;
        }
        let Some(field) = entity.get_field(field_name) else {
            continue;
        };
        if !is_importable(field, wf) {
            continue;
        }
        let raw = row
            .get(&map.column)
            .cloned()
            .or_else(|| map.default.clone())
            .unwrap_or(Value::Null);
        if raw.as_str() == Some("") || raw.is_null() {
            continue;
        }
        match coerce_value(field, &raw) {
            Ok(value) => {
                out.insert(field.name.clone(), value);
            }
            Err(_) => {
                out.insert(field.name.clone(), raw);
            }
        }
    }
    let mut payload = Value::Object(out);
    strip_secrets(Some(entity), &mut payload);
    strip_protected(&mut payload);
    payload
}

fn strip_protected(data: &mut Value) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    obj.retain(|k, _| {
        !PROTECTED.iter().any(|p| k.eq_ignore_ascii_case(p))
            && !is_secret_key(k)
            && !k.starts_with("password")
    });
}

fn coerce_value(field: &FieldDef, value: &Value) -> QefroResult<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    if let Value::Object(_) | Value::Array(_) = value {
        if matches!(field.field_type, FieldType::Json) {
            return Ok(value.clone());
        }
        return Err(QefroError::bad_request(
            "Nested relation import is not supported.",
        ));
    }
    let text = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    };
    if text.is_empty() {
        return Ok(Value::Null);
    }
    match &field.field_type {
        FieldType::Integer => text
            .parse::<i64>()
            .map(|n| json!(n))
            .map_err(|_| type_err(field, "expected an integer")),
        FieldType::Decimal => {
            if value.is_number() {
                Ok(value.clone())
            } else {
                text.parse::<f64>()
                    .map(|_| json!(text))
                    .map_err(|_| type_err(field, "expected a number"))
            }
        }
        FieldType::Boolean => parse_bool(&text)
            .map(|b| json!(b))
            .ok_or_else(|| type_err(field, "expected true or false")),
        FieldType::Uuid | FieldType::Relation => Ok(json!(text)),
        FieldType::Json => serde_json::from_str(&text).or_else(|_| Ok(json!(text))),
        FieldType::ChildTable => Err(QefroError::bad_request(
            "Nested relation import is not supported.",
        )),
        _ => Ok(json!(text)),
    }
}

fn type_err(field: &FieldDef, message: &str) -> QefroError {
    QefroError::validation(vec![qefro_core::FieldError::new(
        &field.name,
        "invalid_type",
        message,
    )])
}

fn parse_bool(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "t" | "yes" | "y" | "1" => Some(true),
        "false" | "f" | "no" | "n" | "0" => Some(false),
        _ => None,
    }
}

fn unique_match_fields(entity: &EntityDef, wf_field: Option<&str>) -> Vec<String> {
    importable_fields(entity, wf_field)
        .into_iter()
        .filter(|f| f.unique)
        .map(|f| f.name.clone())
        .collect()
}

fn resolve_match_field(
    entity: &EntityDef,
    wf_field: Option<&str>,
    requested: Option<&str>,
    mode: ImportMode,
) -> QefroResult<Option<String>> {
    let uniques = unique_match_fields(entity, wf_field);
    if let Some(name) = requested.map(str::trim).filter(|s| !s.is_empty()) {
        if name.eq_ignore_ascii_case("id") {
            return Err(QefroError::bad_request(
                "matching on id is not allowed; use a unique business field",
            ));
        }
        let field = entity
            .get_field(name)
            .ok_or_else(|| QefroError::bad_request(format!("unknown match field '{name}'")))?;
        if !field.unique {
            return Err(QefroError::bad_request(format!(
                "match field '{name}' is not unique"
            )));
        }
        if !is_importable(field, wf_field) {
            return Err(QefroError::bad_request(format!(
                "match field '{name}' cannot be used for import matching"
            )));
        }
        return Ok(Some(field.name.clone()));
    }
    match mode {
        ImportMode::Create => Ok(uniques.first().cloned()),
        ImportMode::Update | ImportMode::Upsert => {
            if uniques.len() == 1 {
                Ok(Some(uniques[0].clone()))
            } else if uniques.is_empty() {
                Err(QefroError::bad_request(
                    "update/upsert requires an explicit unique match field",
                ))
            } else {
                Err(QefroError::bad_request(
                    "update/upsert requires an explicit unique match field",
                ))
            }
        }
    }
}

impl EntityService {
    fn import_wf_field(&self, entity: &EntityDef) -> Option<String> {
        self.workflows()
            .for_entity(&entity.name)
            .map(|wf| wf.field.clone())
    }

    fn ensure_importable(&self, ctx: &OpContext, entity: &EntityDef) -> QefroResult<()> {
        self.ensure_app(ctx, &entity)?;
        if entity.singleton || !entity.standalone {
            return Err(QefroError::bad_request(format!(
                "{} cannot be imported",
                entity.name
            )));
        }
        Ok(())
    }

    pub fn preview_import(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        csv: &str,
        mapping: &[ImportMapping],
    ) -> QefroResult<ImportPreview> {
        let opts = ImportOptions {
            mapping: mapping.to_vec(),
            format: ImportFormat::Csv,
            ..ImportOptions::default()
        };
        self.preview_import_source(ctx, entity_name, csv, &opts)
    }

    pub fn preview_import_source(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        source: &str,
        opts: &ImportOptions,
    ) -> QefroResult<ImportPreview> {
        let entity = self.registry().get(entity_name)?;
        self.ensure_importable(ctx, &entity)?;
        self.permissions()
            .check(ctx, &entity.name, Action::Create)?;
        if source.len() > max_import_bytes() {
            return Err(QefroError::payload_too_large(format!(
                "import exceeds {} bytes",
                max_import_bytes()
            )));
        }
        let wf = self.import_wf_field(&entity);
        let (columns, rows) = parse_source(source, opts.format)?;
        let mapping = if opts.mapping.is_empty() {
            suggest_mapping(&entity, &columns, wf.as_deref())
        } else {
            opts.mapping.clone()
        };
        let ignored: Vec<String> = mapping
            .iter()
            .filter(|m| {
                m.field
                    .as_deref()
                    .map(|f| f.is_empty() || f == "ignore")
                    .unwrap_or(true)
            })
            .map(|m| m.column.clone())
            .collect();
        let mut errors = Vec::new();
        let mut sample = Vec::new();
        let mut valid = 0;
        let mut warnings = 0;
        for (i, row) in rows.iter().enumerate() {
            if let Err(err) = reject_nested(row) {
                errors.push(row_error(i, None, &err, Some(row)));
                continue;
            }
            let payload = apply_mapping(&entity, row, &mapping);
            match qefro_core::validate_record(entity.business_fields(), &payload, false) {
                Ok(()) => {
                    valid += 1;
                    if sample.len() < PREVIEW_SAMPLE {
                        sample.push(payload);
                    }
                }
                Err(err) => {
                    errors.push(row_error(i, field_from_err(&err), &err, Some(row)));
                }
            }
        }
        if !ignored.is_empty() {
            warnings += ignored.len();
        }
        Ok(ImportPreview {
            rows: rows.len(),
            valid,
            invalid: errors.len(),
            warnings,
            columns,
            mapping,
            fields: field_info(&entity, wf.as_deref()),
            ignored,
            errors: errors.into_iter().take(200).collect(),
            sample,
            match_fields: unique_match_fields(&entity, wf.as_deref()),
        })
    }

    pub async fn run_import(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        csv: &str,
        mapping: &[ImportMapping],
        batch_size: usize,
    ) -> QefroResult<ImportResult> {
        let opts = ImportOptions {
            mapping: mapping.to_vec(),
            batch_size,
            format: ImportFormat::Csv,
            ..ImportOptions::default()
        };
        self.run_import_source(ctx, entity_name, csv, &opts, None, None, None)
            .await
    }

    pub async fn run_import_source(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        source: &str,
        opts: &ImportOptions,
        blobs: Option<&dyn BlobStore>,
        blob_meta: Option<&BlobMetaStore>,
        notifications: Option<&NotificationStore>,
    ) -> QefroResult<ImportResult> {
        let entity = self.registry().get(entity_name)?;
        self.ensure_importable(ctx, &entity)?;
        self.permissions()
            .check(ctx, &entity.name, Action::Create)?;
        if matches!(opts.mode, ImportMode::Update | ImportMode::Upsert)
            || opts.duplicate_policy == DuplicatePolicy::Update
        {
            self.permissions()
                .check(ctx, &entity.name, Action::Update)?;
        }
        if source.len() > max_import_bytes() {
            return Err(QefroError::payload_too_large(format!(
                "import exceeds {} bytes",
                max_import_bytes()
            )));
        }
        let wf = self.import_wf_field(&entity);
        let match_field = resolve_match_field(
            &entity,
            wf.as_deref(),
            opts.match_field.as_deref(),
            opts.mode,
        )?;
        let mut opts = opts.clone();
        opts.match_field = match_field;
        if opts.batch_size == 0 {
            opts.batch_size = DEFAULT_BATCH;
        }
        let async_needed = !opts.dry_run
            && (source.len() > SYNC_MAX_BYTES
                || source.lines().count().saturating_sub(1) > SYNC_MAX_ROWS);
        if async_needed {
            if blobs.is_none() {
                return Err(QefroError::bad_request(
                    "large imports require File Runtime storage",
                ));
            }
            let job = self
                .create_import_job(ctx, &entity.name, source, &opts, blobs, blob_meta)
                .await?;
            return Ok(ImportResult {
                imported: 0,
                created: 0,
                updated: 0,
                skipped: 0,
                failed: 0,
                warnings: 0,
                processed: 0,
                total: job.total_rows as usize,
                dry_run: opts.dry_run,
                async_job: true,
                job_id: Some(job.id),
                status: job.status,
                errors: Vec::new(),
                error_report_key: None,
            });
        }
        let (columns, rows) = parse_source(source, opts.format)?;
        let _ = columns;
        let mapping = if opts.mapping.is_empty() {
            let cols: Vec<String> = rows
                .first()
                .map(|r| r.keys().cloned().collect())
                .unwrap_or_default();
            suggest_mapping(&entity, &cols, wf.as_deref())
        } else {
            opts.mapping.clone()
        };
        let mut counts = ImportCounts::default();
        counts.total = rows.len();
        let mut errors = Vec::new();
        let mut originals: Vec<(usize, Map<String, Value>, String, Option<String>)> = Vec::new();
        let batch = opts.batch_size.clamp(1, 500);
        for (i, row) in rows.iter().enumerate() {
            match self
                .import_one_row(ctx, &entity, row, &mapping, &opts, wf.as_deref())
                .await
            {
                Ok(outcome) => counts.apply(outcome),
                Err(err) => {
                    counts.failed += 1;
                    let field = field_from_err(&err);
                    errors.push(row_error(i, field.clone(), &err, Some(row)));
                    originals.push((i + 2, row.clone(), err.to_string(), field));
                    if opts.strict {
                        return Err(err);
                    }
                }
            }
            if (i + 1) % batch == 0 {
                tokio::task::yield_now().await;
            }
        }
        let report_key = if !originals.is_empty() {
            if let Some(store) = blobs {
                write_error_report(ctx, store, blob_meta, Uuid::nil(), &entity.name, &originals)
                    .await
                    .ok()
            } else {
                None
            }
        } else {
            None
        };
        if !opts.dry_run {
            self.record_import_summary(ctx, &entity.name, Uuid::nil(), &counts, &opts)
                .await;
            if let Some(notes) = notifications {
                notify_import(notes, ctx, &entity.name, &counts).await;
            }
        }
        Ok(counts.into_result(opts.dry_run, false, None, errors, report_key))
    }

    pub async fn store_import_file(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        filename: &str,
        mime: &str,
        bytes: &[u8],
        blobs: &dyn BlobStore,
        blob_meta: &BlobMetaStore,
    ) -> QefroResult<(String, String, ImportFormat)> {
        let entity = self.registry().get(entity_name)?;
        self.ensure_importable(ctx, &entity)?;
        self.permissions()
            .check(ctx, &entity.name, Action::Create)?;
        let filename = sanitize_filename(filename)?;
        if bytes.len() > max_import_bytes() {
            return Err(QefroError::payload_too_large(format!(
                "import exceeds {} bytes",
                max_import_bytes()
            )));
        }
        let format = detect_format(&filename, mime, bytes)?;
        let key = format!("imports/{}/{filename}", Uuid::new_v4());
        blobs.put(ctx.tenant_id, &key, bytes)?;
        blob_meta
            .insert(
                ctx.tenant_id,
                ctx.user_id,
                &BlobMeta {
                    key: key.clone(),
                    filename: filename.clone(),
                    content_type: match format {
                        ImportFormat::Json => "application/json".into(),
                        ImportFormat::Csv => "text/csv".into(),
                    },
                    size: bytes.len() as i64,
                },
            )
            .await?;
        Ok((key, filename, format))
    }

    pub async fn submit_import_blob(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        blob_key: &str,
        opts: &ImportOptions,
        blobs: &dyn BlobStore,
        blob_meta: Option<&BlobMetaStore>,
        notifications: Option<&NotificationStore>,
    ) -> QefroResult<ImportResult> {
        let bytes = blobs.get(ctx.tenant_id, blob_key)?;
        let text = decode_text(&bytes)?;
        let mut opts = opts.clone();
        if opts.filename.is_none() {
            opts.filename = blob_key.rsplit('/').next().map(ToOwned::to_owned);
        }
        self.run_import_source(
            ctx,
            entity_name,
            &text,
            &opts,
            Some(blobs),
            blob_meta,
            notifications,
        )
        .await
    }

    async fn create_import_job(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        source: &str,
        opts: &ImportOptions,
        blobs: Option<&dyn BlobStore>,
        blob_meta: Option<&BlobMetaStore>,
    ) -> QefroResult<ImportJobRecord> {
        if let Some(key) = opts.idempotency_key.as_deref() {
            if let Some(existing) = self.find_import_by_idemp(ctx, key).await? {
                return Ok(existing);
            }
        }
        let (columns, rows) = parse_source(source, opts.format)?;
        let entity = self.registry().get(entity_name)?;
        let wf = self.import_wf_field(&entity);
        let mapping = if opts.mapping.is_empty() {
            suggest_mapping(&entity, &columns, wf.as_deref())
        } else {
            opts.mapping.clone()
        };
        let id = Uuid::new_v4();
        let blob_key = if let Some(store) = blobs {
            let filename = opts
                .filename
                .clone()
                .unwrap_or_else(|| format!("import.{}", opts.format.as_str()));
            let key = format!("imports/{id}/{filename}");
            store.put(ctx.tenant_id, &key, source.as_bytes())?;
            if let Some(meta) = blob_meta {
                let _ = meta
                    .insert(
                        ctx.tenant_id,
                        ctx.user_id,
                        &BlobMeta {
                            key: key.clone(),
                            filename,
                            content_type: match opts.format {
                                ImportFormat::Json => "application/json".into(),
                                ImportFormat::Csv => "text/csv".into(),
                            },
                            size: source.len() as i64,
                        },
                    )
                    .await;
            }
            Some(key)
        } else {
            None
        };
        let mapping_json = serde_json::to_value(&mapping).unwrap_or(json!([]));
        sqlx::query(
            r#"
            INSERT INTO qefro_import_jobs (
                id, tenant_id, user_id, entity, status, mode, duplicate_policy, match_field,
                mapping, format, dry_run, strict, filename, content_type, blob_key,
                total_rows, idempotency_key
            ) VALUES (
                $1,$2,$3,$4,'pending',$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16
            )
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .bind(ctx.user_id)
        .bind(entity_name)
        .bind(opts.mode.as_str())
        .bind(opts.duplicate_policy.as_str())
        .bind(opts.match_field.as_deref())
        .bind(&mapping_json)
        .bind(opts.format.as_str())
        .bind(opts.dry_run)
        .bind(opts.strict)
        .bind(opts.filename.as_deref())
        .bind(match opts.format {
            ImportFormat::Csv => "text/csv",
            ImportFormat::Json => "application/json",
        })
        .bind(blob_key.as_deref())
        .bind(rows.len() as i32)
        .bind(opts.idempotency_key.as_deref())
        .execute(self.pool())
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let payload = json!({
            "import_job_id": id,
            "user_id": ctx.user_id,
            "roles": ctx.roles,
            "actor_name": ctx.actor_name,
            "idempotency_key": format!("import:{id}"),
            "max_attempts": 8,
        });
        self.job_queue()
            .enqueue(ctx, IMPORT_RUN_JOB, payload)
            .await?;
        self.get_import_job(ctx, id).await
    }

    pub async fn list_import_jobs(
        &self,
        ctx: &OpContext,
        entity: Option<&str>,
    ) -> QefroResult<Vec<ImportJobRecord>> {
        if let Some(name) = entity {
            self.permissions().check(ctx, name, Action::List)?;
        }
        let rows = if let Some(name) = entity {
            sqlx::query_as::<_, ImportJobRecord>(
                r#"
                SELECT * FROM qefro_import_jobs
                WHERE tenant_id = $1 AND entity = $2
                  AND (user_id = $3 OR $4)
                ORDER BY created_at DESC
                LIMIT 100
                "#,
            )
            .bind(ctx.tenant_id)
            .bind(name)
            .bind(ctx.user_id)
            .bind(ctx.is_admin())
            .fetch_all(self.pool())
            .await
        } else {
            sqlx::query_as::<_, ImportJobRecord>(
                r#"
                SELECT * FROM qefro_import_jobs
                WHERE tenant_id = $1 AND (user_id = $2 OR $3)
                ORDER BY created_at DESC
                LIMIT 100
                "#,
            )
            .bind(ctx.tenant_id)
            .bind(ctx.user_id)
            .bind(ctx.is_admin())
            .fetch_all(self.pool())
            .await
        };
        rows.map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn get_import_job(&self, ctx: &OpContext, id: Uuid) -> QefroResult<ImportJobRecord> {
        let row = sqlx::query_as::<_, ImportJobRecord>(
            "SELECT * FROM qefro_import_jobs WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("import job not found"))?;
        if row.user_id != Some(ctx.user_id) && !ctx.is_admin() {
            return Err(QefroError::not_found("import job not found"));
        }
        self.permissions().check(ctx, &row.entity, Action::Read)?;
        Ok(row)
    }

    pub async fn cancel_import_job(
        &self,
        ctx: &OpContext,
        id: Uuid,
    ) -> QefroResult<ImportJobRecord> {
        let job = self.get_import_job(ctx, id).await?;
        if !matches!(job.status.as_str(), "pending" | "validating" | "running") {
            return Err(QefroError::bad_request("import is not running"));
        }
        sqlx::query(
            r#"
            UPDATE qefro_import_jobs
            SET cancel_requested = true,
                status = CASE WHEN status = 'pending' THEN 'cancelled' ELSE status END,
                updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .execute(self.pool())
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        self.get_import_job(ctx, id).await
    }

    pub async fn retry_import_job(
        &self,
        ctx: &OpContext,
        id: Uuid,
    ) -> QefroResult<ImportJobRecord> {
        let job = self.get_import_job(ctx, id).await?;
        if !matches!(job.status.as_str(), "failed") {
            return Err(QefroError::bad_request(
                "only failed imports can be retried",
            ));
        }
        sqlx::query(
            r#"
            UPDATE qefro_import_jobs
            SET status = 'pending', last_error = NULL, cancel_requested = false,
                retry_count = retry_count + 1, updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .execute(self.pool())
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let retry = job.retry_count + 1;
        let payload = json!({
            "import_job_id": id,
            "user_id": ctx.user_id,
            "roles": ctx.roles,
            "actor_name": ctx.actor_name,
            "idempotency_key": format!("import:{id}:r{retry}"),
            "max_attempts": 8,
        });
        self.job_queue()
            .enqueue(ctx, IMPORT_RUN_JOB, payload)
            .await?;
        self.get_import_job(ctx, id).await
    }

    pub async fn import_error_report(
        &self,
        ctx: &OpContext,
        id: Uuid,
        blobs: &dyn BlobStore,
    ) -> QefroResult<(String, Vec<u8>)> {
        let job = self.get_import_job(ctx, id).await?;
        let key = job
            .error_report_key
            .ok_or_else(|| QefroError::not_found("error report not found"))?;
        let bytes = blobs.get(ctx.tenant_id, &key)?;
        Ok((
            format!("{}-import-errors.csv", job.entity.to_ascii_lowercase()),
            bytes,
        ))
    }

    pub async fn process_import_job(
        &self,
        ctx: &OpContext,
        job_id: Uuid,
        blobs: &dyn BlobStore,
        blob_meta: Option<&BlobMetaStore>,
        notifications: Option<&NotificationStore>,
    ) -> QefroResult<()> {
        let job = sqlx::query_as::<_, ImportJobRecord>(
            "SELECT * FROM qefro_import_jobs WHERE id = $1 AND tenant_id = $2",
        )
        .bind(job_id)
        .bind(ctx.tenant_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("import job not found"))?;
        if job.cancel_requested {
            self.set_import_status(job_id, ctx.tenant_id, "cancelled", None)
                .await?;
            return Ok(());
        }
        if matches!(
            job.status.as_str(),
            "completed" | "completed_with_errors" | "cancelled"
        ) {
            return Ok(());
        }
        self.set_import_status(job_id, ctx.tenant_id, "validating", None)
            .await?;
        let entity = self.registry().get(&job.entity)?;
        let key = job
            .blob_key
            .as_deref()
            .ok_or_else(|| QefroError::internal("import file is missing"))?;
        let bytes = blobs.get(ctx.tenant_id, key)?;
        let text = decode_text(&bytes)?;
        let format = ImportFormat::parse(Some(&job.format));
        let mapping: Vec<ImportMapping> =
            serde_json::from_value(job.mapping.clone()).unwrap_or_default();
        let opts = ImportOptions {
            mapping,
            mode: ImportMode::parse(Some(&job.mode))?,
            duplicate_policy: DuplicatePolicy::parse(Some(&job.duplicate_policy))?,
            match_field: job.match_field.clone(),
            dry_run: job.dry_run,
            batch_size: DEFAULT_BATCH,
            strict: job.strict,
            format,
            filename: job.filename.clone(),
            idempotency_key: None,
        };
        let (_, rows) = parse_source(&text, format)?;
        self.set_import_status(job_id, ctx.tenant_id, "running", None)
            .await?;
        let wf = self.import_wf_field(&entity);
        let mut counts = ImportCounts {
            total: rows.len(),
            created: job.created_count as usize,
            updated: job.updated_count as usize,
            skipped: job.skipped_count as usize,
            failed: job.failed_count as usize,
            warnings: job.warning_count as usize,
            processed: job.checkpoint as usize,
        };
        let mut errors: Vec<(usize, Map<String, Value>, String, Option<String>)> = Vec::new();
        let start = job.checkpoint as usize;
        let batch = opts.batch_size.clamp(1, 500);
        for (i, row) in rows.iter().enumerate() {
            if i < start {
                continue;
            }
            if self.import_cancel_requested(job_id, ctx.tenant_id).await? {
                self.persist_progress(job_id, ctx.tenant_id, &counts, "cancelled")
                    .await?;
                return Ok(());
            }
            match self
                .import_one_row(ctx, &entity, row, &opts.mapping, &opts, wf.as_deref())
                .await
            {
                Ok(outcome) => counts.apply(outcome),
                Err(err) => {
                    counts.failed += 1;
                    let field = field_from_err(&err);
                    errors.push((i + 2, row.clone(), err.to_string(), field));
                    if opts.strict {
                        self.persist_progress(job_id, ctx.tenant_id, &counts, "failed")
                            .await?;
                        return Err(err);
                    }
                }
            }
            counts.processed = i + 1;
            if (i + 1) % batch == 0 {
                self.persist_progress(job_id, ctx.tenant_id, &counts, "running")
                    .await?;
            }
        }
        let report_key = if !errors.is_empty() {
            write_error_report(ctx, blobs, blob_meta, job_id, &entity.name, &errors)
                .await
                .ok()
        } else {
            None
        };
        let status = if job.cancel_requested {
            "cancelled"
        } else if counts.failed > 0 && counts.created + counts.updated == 0 && !opts.dry_run {
            "failed"
        } else if counts.failed > 0 {
            "completed_with_errors"
        } else {
            "completed"
        };
        sqlx::query(
            r#"
            UPDATE qefro_import_jobs SET
                processed = $3, created_count = $4, updated_count = $5, skipped_count = $6,
                failed_count = $7, warning_count = $8, checkpoint = $3, status = $9,
                error_report_key = COALESCE($10, error_report_key),
                last_error = $11, updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(job_id)
        .bind(ctx.tenant_id)
        .bind(counts.processed as i32)
        .bind(counts.created as i32)
        .bind(counts.updated as i32)
        .bind(counts.skipped as i32)
        .bind(counts.failed as i32)
        .bind(counts.warnings as i32)
        .bind(status)
        .bind(report_key.as_deref())
        .bind(errors.first().map(|e| e.2.clone()))
        .execute(self.pool())
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        if !opts.dry_run {
            self.record_import_summary(ctx, &entity.name, job_id, &counts, &opts)
                .await;
            if let Some(notes) = notifications {
                notify_import(notes, ctx, &entity.name, &counts).await;
            }
        }
        Ok(())
    }

    async fn import_one_row(
        &self,
        ctx: &OpContext,
        entity: &EntityDef,
        row: &Map<String, Value>,
        mapping: &[ImportMapping],
        opts: &ImportOptions,
        wf_field: Option<&str>,
    ) -> QefroResult<RowOutcome> {
        reject_nested(row)?;
        let mut payload = apply_mapping(entity, row, mapping);
        if let Some(wf) = wf_field {
            if let Some(obj) = payload.as_object_mut() {
                obj.remove(wf);
            }
        }
        strip_protected(&mut payload);
        strip_secrets(Some(entity), &mut payload);
        self.resolve_relations(ctx, entity, &mut payload).await?;
        let existing = self
            .lookup_existing(ctx, entity, &payload, opts.match_field.as_deref())
            .await?;
        match (opts.mode, existing) {
            (ImportMode::Create, Some(id)) => match opts.duplicate_policy {
                DuplicatePolicy::Fail => Err(QefroError::conflict("duplicate row")),
                DuplicatePolicy::Skip => Ok(RowOutcome::Skipped),
                DuplicatePolicy::Update => self.import_update(ctx, entity, id, payload, opts).await,
            },
            (ImportMode::Create, None) => self.import_create(ctx, entity, payload, opts).await,
            (ImportMode::Update, Some(id)) => {
                self.import_update(ctx, entity, id, payload, opts).await
            }
            (ImportMode::Update, None) => Err(QefroError::not_found("matching record not found")),
            (ImportMode::Upsert, Some(id)) => {
                self.import_update(ctx, entity, id, payload, opts).await
            }
            (ImportMode::Upsert, None) => self.import_create(ctx, entity, payload, opts).await,
        }
    }

    async fn import_create(
        &self,
        ctx: &OpContext,
        entity: &EntityDef,
        payload: Value,
        opts: &ImportOptions,
    ) -> QefroResult<RowOutcome> {
        if opts.dry_run {
            qefro_core::validate_record(entity.business_fields(), &payload, false)?;
            self.check_uniques(ctx, entity, &payload, None).await?;
            return Ok(RowOutcome::Created);
        }
        self.create(ctx, &entity.name, payload).await?;
        Ok(RowOutcome::Created)
    }

    async fn import_update(
        &self,
        ctx: &OpContext,
        entity: &EntityDef,
        id: Uuid,
        payload: Value,
        opts: &ImportOptions,
    ) -> QefroResult<RowOutcome> {
        self.permissions()
            .check(ctx, &entity.name, Action::Update)?;
        if opts.dry_run {
            qefro_core::validate_record(entity.business_fields(), &payload, true)?;
            self.check_uniques(ctx, entity, &payload, Some(id)).await?;
            return Ok(RowOutcome::Updated);
        }
        self.update(ctx, &entity.name, id, payload).await?;
        Ok(RowOutcome::Updated)
    }

    async fn lookup_existing(
        &self,
        ctx: &OpContext,
        entity: &EntityDef,
        payload: &Value,
        match_field: Option<&str>,
    ) -> QefroResult<Option<Uuid>> {
        let Some(field) = match_field else {
            return self.lookup_any_unique(ctx, entity, payload).await;
        };
        let Some(value) = payload
            .get(field)
            .filter(|v| !v.is_null() && v.as_str() != Some(""))
        else {
            return Ok(None);
        };
        self.lookup_field(ctx, entity, field, value).await
    }

    async fn lookup_any_unique(
        &self,
        ctx: &OpContext,
        entity: &EntityDef,
        payload: &Value,
    ) -> QefroResult<Option<Uuid>> {
        for field in importable_fields(entity, None)
            .into_iter()
            .filter(|f| f.unique)
        {
            if let Some(value) = payload
                .get(&field.name)
                .filter(|v| !v.is_null() && v.as_str() != Some(""))
            {
                if let Some(id) = self.lookup_field(ctx, entity, &field.name, value).await? {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }

    async fn lookup_field(
        &self,
        ctx: &OpContext,
        entity: &EntityDef,
        field: &str,
        value: &Value,
    ) -> QefroResult<Option<Uuid>> {
        let ids = self
            .repo
            .find_ids_by_field(entity, ctx, field, value, 3)
            .await?;
        match ids.as_slice() {
            [] => Ok(None),
            [id] => Ok(Some(*id)),
            _ => Err(QefroError::conflict(format!(
                "ambiguous relation: multiple {entity} records match {field}",
                entity = entity.name
            ))),
        }
    }

    async fn resolve_relations(
        &self,
        ctx: &OpContext,
        entity: &EntityDef,
        payload: &mut Value,
    ) -> QefroResult<()> {
        let Some(obj) = payload.as_object_mut() else {
            return Ok(());
        };
        let fields: Vec<FieldDef> = entity
            .business_fields()
            .iter()
            .filter(|f| f.relation.is_some())
            .cloned()
            .collect();
        for field in fields {
            let Some(raw) = obj.get(&field.name).cloned() else {
                continue;
            };
            if raw.is_null() || raw.as_str() == Some("") {
                continue;
            }
            if let Value::Object(_) | Value::Array(_) = &raw {
                return Err(QefroError::bad_request(
                    "Nested relation import is not supported.",
                ));
            }
            let text = match &raw {
                Value::String(s) => s.trim().to_string(),
                other => other.to_string().trim_matches('"').to_string(),
            };
            if Uuid::parse_str(&text).is_ok() {
                obj.insert(field.name.clone(), json!(text));
                continue;
            }
            let target_name = field
                .relation
                .as_ref()
                .map(|r| r.target_entity.as_str())
                .unwrap_or("");
            let target = self.registry().get(target_name)?;
            let resolved = resolve_relation_value(self, ctx, &target, &text).await?;
            obj.insert(field.name.clone(), json!(resolved));
        }
        Ok(())
    }

    async fn import_cancel_requested(&self, id: Uuid, tenant: Uuid) -> QefroResult<bool> {
        sqlx::query_scalar::<_, bool>(
            "SELECT cancel_requested FROM qefro_import_jobs WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant)
        .fetch_optional(self.pool())
        .await
        .map(|v| v.unwrap_or(false))
        .map_err(|e| QefroError::database(e.to_string()))
    }

    async fn set_import_status(
        &self,
        id: Uuid,
        tenant: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> QefroResult<()> {
        sqlx::query(
            "UPDATE qefro_import_jobs SET status = $3, last_error = $4, updated_at = now() WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant)
        .bind(status)
        .bind(error)
        .execute(self.pool())
        .await
        .map(|_| ())
        .map_err(|e| QefroError::database(e.to_string()))
    }

    async fn persist_progress(
        &self,
        id: Uuid,
        tenant: Uuid,
        counts: &ImportCounts,
        status: &str,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            UPDATE qefro_import_jobs SET
                processed = $3, created_count = $4, updated_count = $5, skipped_count = $6,
                failed_count = $7, warning_count = $8, checkpoint = $3, status = $9, updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant)
        .bind(counts.processed as i32)
        .bind(counts.created as i32)
        .bind(counts.updated as i32)
        .bind(counts.skipped as i32)
        .bind(counts.failed as i32)
        .bind(counts.warnings as i32)
        .bind(status)
        .execute(self.pool())
        .await
        .map(|_| ())
        .map_err(|e| QefroError::database(e.to_string()))
    }

    async fn find_import_by_idemp(
        &self,
        ctx: &OpContext,
        key: &str,
    ) -> QefroResult<Option<ImportJobRecord>> {
        sqlx::query_as::<_, ImportJobRecord>(
            "SELECT * FROM qefro_import_jobs WHERE tenant_id = $1 AND idempotency_key = $2",
        )
        .bind(ctx.tenant_id)
        .bind(key)
        .fetch_optional(self.pool())
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    async fn record_import_summary(
        &self,
        ctx: &OpContext,
        entity: &str,
        job_id: Uuid,
        counts: &ImportCounts,
        opts: &ImportOptions,
    ) {
        let message = format!(
            "Qefro Import: {} records imported, {} warnings, {} failed",
            counts.created + counts.updated,
            counts.warnings,
            counts.failed
        );
        let mut summary_ctx = ctx.clone();
        summary_ctx.actor_name = Some("Qefro Import".into());
        let metadata = json!({
            "processed": counts.processed.max(counts.total),
            "created": counts.created,
            "updated": counts.updated,
            "skipped": counts.skipped,
            "failed": counts.failed,
            "dry_run": opts.dry_run,
        });
        let _ = self
            .activity
            .record(
                &summary_ctx,
                entity,
                if job_id.is_nil() {
                    ctx.request_id
                } else {
                    job_id
                },
                TYPE_SYSTEM,
                &message,
                metadata.clone(),
            )
            .await;
        let _ = self
            .audit()
            .record(
                ctx,
                entity,
                if job_id.is_nil() { None } else { Some(job_id) },
                "import",
                None,
                Some(&json!({
                    "message": format!("Imported {} {} records", counts.created + counts.updated, entity),
                    "created": counts.created,
                    "updated": counts.updated,
                    "skipped": counts.skipped,
                    "failed": counts.failed,
                })),
            )
            .await;
    }
}

async fn resolve_relation_value(
    service: &EntityService,
    ctx: &OpContext,
    target: &EntityDef,
    text: &str,
) -> QefroResult<String> {
    let mut candidates: Vec<&FieldDef> = target
        .business_fields()
        .iter()
        .filter(|f| {
            f.unique
                && !f.system
                && (f.validation.email || f.name == "email" || f.name == "external_id" || f.unique)
        })
        .collect();
    candidates.sort_by(|a, b| a.name.cmp(&b.name));
    candidates.dedup_by(|a, b| a.name == b.name);
    let mut matches = Vec::new();
    for field in candidates {
        let ids = service
            .repo
            .find_ids_by_field(target, ctx, &field.name, &json!(text), 3)
            .await?;
        matches.extend(ids);
    }
    matches.sort();
    matches.dedup();
    match matches.as_slice() {
        [] => Err(QefroError::bad_request(format!(
            "related {} not found",
            target.name
        ))),
        [id] => Ok(id.to_string()),
        _ => Err(QefroError::conflict(format!(
            "ambiguous relation: multiple {} records match",
            target.name
        ))),
    }
}

#[derive(Default)]
struct ImportCounts {
    total: usize,
    processed: usize,
    created: usize,
    updated: usize,
    skipped: usize,
    failed: usize,
    warnings: usize,
}

impl ImportCounts {
    fn apply(&mut self, outcome: RowOutcome) {
        self.processed += 1;
        match outcome {
            RowOutcome::Created => self.created += 1,
            RowOutcome::Updated => self.updated += 1,
            RowOutcome::Skipped => self.skipped += 1,
        }
    }

    fn into_result(
        self,
        dry_run: bool,
        async_job: bool,
        job_id: Option<Uuid>,
        errors: Vec<ImportRowError>,
        error_report_key: Option<String>,
    ) -> ImportResult {
        let status = if dry_run {
            "validated".into()
        } else if self.failed > 0 && self.created + self.updated > 0 {
            "completed_with_errors".into()
        } else if self.failed > 0 {
            "failed".into()
        } else {
            "completed".into()
        };
        ImportResult {
            imported: self.created + self.updated,
            created: self.created,
            updated: self.updated,
            skipped: self.skipped,
            failed: self.failed,
            warnings: self.warnings,
            processed: self.processed.max(self.total),
            total: self.total,
            dry_run,
            async_job,
            job_id,
            status,
            errors,
            error_report_key,
        }
    }
}

enum RowOutcome {
    Created,
    Updated,
    Skipped,
}

fn row_error(
    index: usize,
    field: Option<String>,
    err: &QefroError,
    values: Option<&Map<String, Value>>,
) -> ImportRowError {
    ImportRowError {
        row: index + 2,
        field,
        message: err.to_string(),
        reason: Some(err.to_string()),
        values: values.map(|v| Value::Object(v.clone())),
    }
}

fn field_from_err(err: &QefroError) -> Option<String> {
    match err {
        QefroError::Validation { fields, .. } => fields.first().map(|e| e.field.clone()),
        _ => None,
    }
}

async fn write_error_report(
    ctx: &OpContext,
    blobs: &dyn BlobStore,
    blob_meta: Option<&BlobMetaStore>,
    job_id: Uuid,
    entity: &str,
    errors: &[(usize, Map<String, Value>, String, Option<String>)],
) -> QefroResult<String> {
    let mut columns: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for (_, row, _, _) in errors {
        for key in row.keys() {
            if seen.insert(key.clone()) {
                columns.push(key.clone());
            }
        }
    }
    let mut csv = String::from("row");
    for col in &columns {
        csv.push(',');
        csv.push_str(&csv_escape(col));
    }
    csv.push_str(",field,error,reason\n");
    for (row_no, values, message, field) in errors {
        csv.push_str(&row_no.to_string());
        for col in &columns {
            csv.push(',');
            let cell = values.get(col).and_then(|v| v.as_str()).unwrap_or("");
            csv.push_str(&csv_escape(cell));
        }
        csv.push(',');
        csv.push_str(&csv_escape(field.as_deref().unwrap_or("")));
        csv.push(',');
        csv.push_str(&csv_escape(message));
        csv.push(',');
        csv.push_str(&csv_escape(message));
        csv.push('\n');
    }
    let key = format!(
        "imports/{}/{}-errors.csv",
        if job_id.is_nil() {
            ctx.request_id
        } else {
            job_id
        },
        entity.to_ascii_lowercase()
    );
    blobs.put(ctx.tenant_id, &key, csv.as_bytes())?;
    if let Some(meta) = blob_meta {
        let _ = meta
            .insert(
                ctx.tenant_id,
                ctx.user_id,
                &BlobMeta {
                    key: key.clone(),
                    filename: format!("{entity}-import-errors.csv"),
                    content_type: "text/csv".into(),
                    size: csv.len() as i64,
                },
            )
            .await;
    }
    Ok(key)
}

async fn notify_import(
    store: &NotificationStore,
    ctx: &OpContext,
    entity: &str,
    counts: &ImportCounts,
) {
    let row = InAppNotification {
        id: Uuid::new_v4(),
        tenant_id: ctx.tenant_id,
        user_id: ctx.user_id,
        title: "Import completed".into(),
        body: format!(
            "{entity} import completed — {} processed",
            counts.processed.max(counts.total)
        ),
        entity: Some(entity.into()),
        record_id: None,
        read_at: None,
        created_at: Utc::now(),
    };
    let _ = store.insert(&row).await;
}

fn detect_format(filename: &str, mime: &str, bytes: &[u8]) -> QefroResult<ImportFormat> {
    let lower = filename.to_ascii_lowercase();
    let mime = mime.to_ascii_lowercase();
    if lower.ends_with(".json") || mime.contains("json") {
        return Ok(ImportFormat::Json);
    }
    if lower.ends_with(".csv") || mime.contains("csv") {
        return Ok(ImportFormat::Csv);
    }
    let trimmed = bytes
        .iter()
        .copied()
        .skip_while(|b| b.is_ascii_whitespace());
    let first = trimmed.take(1).next();
    match first {
        Some(b'[' | b'{') => Ok(ImportFormat::Json),
        Some(_) => Ok(ImportFormat::Csv),
        None => Err(QefroError::bad_request("import file is empty")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qefro_core::FieldDef;

    fn customer() -> EntityDef {
        EntityDef::new("Customer")
            .field(FieldDef::string("name").required().label("Customer Name"))
            .field(FieldDef::string("email").required().email().unique())
            .field(FieldDef::string("phone").nullable())
            .field(FieldDef::text("notes").nullable())
            .build()
    }

    #[test]
    fn parses_csv_and_json() {
        let (cols, rows) = parse_csv("name,email\nAda,ada@ex.com\n").unwrap();
        assert_eq!(cols, vec!["name", "email"]);
        assert_eq!(rows[0]["name"], json!("Ada"));
        let (jcols, jrows) = parse_json(r#"[{"name":"Ada","email":"ada@ex.com"}]"#).unwrap();
        assert!(jcols.contains(&"name".to_string()));
        assert_eq!(jrows.len(), 1);
    }

    #[test]
    fn rejects_malformed_and_nested() {
        assert!(parse_csv("").is_err());
        assert!(parse_csv("name,name\na,b\n").is_err());
        assert!(parse_json("{not json").is_err());
        assert!(parse_json(r#"{"name":"Ada"}"#).is_err());
        assert!(parse_json(r#"[{"name":{"nested":true}}]"#).is_err());
        assert!(decode_text(&[0xFF, 0xFE]).is_err());
    }

    #[test]
    fn auto_map_is_conservative() {
        let entity = customer();
        let mapping = suggest_mapping(
            &entity,
            &["Customer Name".into(), "email".into(), "Unknown".into()],
            None,
        );
        assert_eq!(mapping[0].field.as_deref(), Some("name"));
        assert_eq!(mapping[1].field.as_deref(), Some("email"));
        assert!(mapping[2].field.is_none());
    }

    #[test]
    fn does_not_map_ambiguous_labels() {
        let entity = EntityDef::new("Thing")
            .field(FieldDef::string("a").label("Title"))
            .field(FieldDef::string("b").label("Title"))
            .build();
        let mapping = suggest_mapping(&entity, &["Title".into()], None);
        assert!(mapping[0].field.is_none());
    }

    #[test]
    fn strips_protected_and_secrets() {
        let entity = customer();
        let mut row = Map::new();
        row.insert("email".into(), json!("a@b.c"));
        row.insert("name".into(), json!("Ada"));
        row.insert("tenant_id".into(), json!("nope"));
        row.insert("password_hash".into(), json!("x"));
        let mapping = vec![
            ImportMapping {
                column: "email".into(),
                field: Some("email".into()),
                default: None,
            },
            ImportMapping {
                column: "name".into(),
                field: Some("name".into()),
                default: None,
            },
            ImportMapping {
                column: "tenant_id".into(),
                field: Some("tenant_id".into()),
                default: None,
            },
            ImportMapping {
                column: "password_hash".into(),
                field: Some("password_hash".into()),
                default: None,
            },
        ];
        let payload = apply_mapping(&entity, &row, &mapping);
        assert!(payload.get("tenant_id").is_none());
        assert!(payload.get("password_hash").is_none());
        assert_eq!(payload["email"], json!("a@b.c"));
    }

    #[test]
    fn formula_cells_are_escaped() {
        assert!(csv_escape("=1+1").starts_with('\''));
        assert!(csv_escape("+cmd").starts_with('\''));
        assert!(csv_escape("@SUM(A1)").starts_with('\''));
        assert_eq!(csv_escape("-12.5"), "-12.5");
    }

    #[test]
    fn ten_thousand_rows_parse_with_bounded_headers() {
        let mut csv = String::from("name,email\n");
        for i in 0..10_000 {
            csv.push_str(&format!("n{i},n{i}@ex.com\n"));
        }
        let (cols, rows) = parse_csv(&csv).unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(rows.len(), 10_000);
    }
}
