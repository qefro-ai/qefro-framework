use qefro_core::{EntityDef, OpContext, QefroError, QefroResult};
use qefro_permissions::Action;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::service::EntityService;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMapping {
    pub column: String,
    pub field: Option<String>,
    #[serde(default)]
    pub default: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub rows: usize,
    pub valid: usize,
    pub invalid: usize,
    pub columns: Vec<String>,
    pub errors: Vec<ImportRowError>,
    pub sample: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowError {
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub failed: usize,
    pub errors: Vec<ImportRowError>,
}

pub fn parse_csv(text: &str) -> QefroResult<(Vec<String>, Vec<Map<String, Value>>)> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| QefroError::bad_request(format!("csv: {e}")))?
        .iter()
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for rec in reader.records() {
        let rec = rec.map_err(|e| QefroError::bad_request(format!("csv: {e}")))?;
        let mut map = Map::new();
        for (i, col) in headers.iter().enumerate() {
            let val = rec.get(i).unwrap_or("").trim();
            map.insert(col.clone(), json!(val));
        }
        rows.push(map);
    }
    Ok((headers, rows))
}

pub fn apply_mapping(
    entity: &EntityDef,
    row: &Map<String, Value>,
    mapping: &[ImportMapping],
) -> Value {
    let mut out = Map::new();
    if mapping.is_empty() {
        for field in entity.business_fields() {
            if field.system || field.computed || field.is_child_table() {
                continue;
            }
            if let Some(v) = row.get(&field.name).or_else(|| row.get(&field.label)) {
                if !v.as_str().map(|s| s.is_empty()).unwrap_or(false) {
                    out.insert(field.name.clone(), v.clone());
                }
            }
        }
    } else {
        for map in mapping {
            if map.field.as_deref() == Some("ignore") || map.field.is_none() {
                continue;
            }
            let field = map.field.as_deref().unwrap();
            let value = row
                .get(&map.column)
                .cloned()
                .or_else(|| map.default.clone())
                .unwrap_or(Value::Null);
            if value.as_str() == Some("") {
                continue;
            }
            out.insert(field.to_string(), value);
        }
    }
    Value::Object(out)
}

impl EntityService {
    pub fn preview_import(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        csv: &str,
        mapping: &[ImportMapping],
    ) -> QefroResult<ImportPreview> {
        let entity = self.registry().get(entity_name)?;
        self.permissions().check(ctx, &entity.name, Action::Create)?;
        if csv.len() > 2 * 1024 * 1024 {
            return Err(QefroError::payload_too_large("CSV exceeds 2 MiB"));
        }
        let (columns, rows) = parse_csv(csv)?;
        let mut errors = Vec::new();
        let mut sample = Vec::new();
        let mut valid = 0;
        for (i, row) in rows.iter().enumerate() {
            let payload = apply_mapping(&entity, row, mapping);
            match qefro_core::validate_record(entity.business_fields(), &payload, false) {
                Ok(()) => {
                    valid += 1;
                    if sample.len() < 5 {
                        sample.push(payload);
                    }
                }
                Err(err) => {
                    errors.push(ImportRowError {
                        row: i + 2,
                        message: err.to_string(),
                    });
                }
            }
        }
        Ok(ImportPreview {
            rows: rows.len(),
            valid,
            invalid: errors.len(),
            columns,
            errors,
            sample,
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
        let preview = self.preview_import(ctx, entity_name, csv, mapping)?;
        let entity = self.registry().get(entity_name)?;
        let (_, rows) = parse_csv(csv)?;
        let batch = batch_size.clamp(1, 500);
        let mut imported = 0;
        let mut failed = 0;
        let mut errors = preview.errors.clone();
        for (i, row) in rows.iter().enumerate() {
            let payload = apply_mapping(&entity, row, mapping);
            if qefro_core::validate_record(entity.business_fields(), &payload, false).is_err() {
                failed += 1;
                continue;
            }
            match self.create(ctx, &entity.name, payload).await {
                Ok(_) => imported += 1,
                Err(err) => {
                    failed += 1;
                    errors.push(ImportRowError {
                        row: i + 2,
                        message: err.to_string(),
                    });
                }
            }
            let _ = batch;
        }
        Ok(ImportResult {
            imported,
            failed,
            errors,
        })
    }
}
