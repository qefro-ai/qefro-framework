use qefro_core::{EntityDef, FieldType, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Filter {
    Eq {
        field: String,
        value: Value,
    },
    Neq {
        field: String,
        value: Value,
    },
    Contains {
        field: String,
        value: String,
    },
    StartsWith {
        field: String,
        value: String,
    },
    Gt {
        field: String,
        value: Value,
    },
    Gte {
        field: String,
        value: Value,
    },
    Lt {
        field: String,
        value: Value,
    },
    Lte {
        field: String,
        value: Value,
    },
    Between {
        field: String,
        from: Value,
        to: Value,
    },
    In {
        field: String,
        values: Vec<Value>,
    },
    NotIn {
        field: String,
        values: Vec<Value>,
    },
    Empty {
        field: String,
    },
    NotEmpty {
        field: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sort {
    pub field: String,
    pub dir: SortDir,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Query {
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sort: Vec<Sort>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub fields: Option<Vec<String>>,
    /// When the entity supports archive, include archived rows.
    #[serde(default)]
    pub include_archived: bool,
}

fn default_page() -> u32 {
    1
}
fn default_page_size() -> u32 {
    25
}

impl Query {
    pub fn offset(&self) -> u32 {
        self.page.saturating_sub(1).saturating_mul(self.page_size)
    }

    pub fn sanitize(mut self, entity: &EntityDef) -> QefroResult<Self> {
        if self.page == 0 {
            self.page = 1;
        }
        self.page_size = self.page_size.clamp(1, 200);
        if self.filters.len() > 20 {
            return Err(QefroError::bad_request("too many filters (max 20)"));
        }
        if self.sort.len() > 3 {
            self.sort.truncate(3);
        }
        if let Some(search) = &self.search {
            if search.chars().count() > 200 {
                return Err(QefroError::bad_request("search query too long"));
            }
        }
        for filter in &self.filters {
            let name = filter.field_name();
            if !entity.is_filterable_field(name) {
                return Err(QefroError::bad_request(format!(
                    "cannot filter on unknown field '{name}'"
                )));
            }
        }
        if self.sort.is_empty() {
            self.sort.push(Sort {
                field: "created_at".into(),
                dir: SortDir::Desc,
            });
        }
        for sort in &self.sort {
            if !entity.is_sortable_field(&sort.field) {
                return Err(QefroError::bad_request(format!(
                    "cannot sort on unknown field '{}'",
                    sort.field
                )));
            }
        }
        if let Some(fields) = &self.fields {
            for f in fields {
                if !entity.has_column(f) {
                    return Err(QefroError::bad_request(format!("unknown field '{f}'")));
                }
            }
        }
        Ok(self)
    }
}

impl Filter {
    pub fn field_name(&self) -> &str {
        match self {
            Self::Eq { field, .. }
            | Self::Neq { field, .. }
            | Self::Contains { field, .. }
            | Self::StartsWith { field, .. }
            | Self::Gt { field, .. }
            | Self::Gte { field, .. }
            | Self::Lt { field, .. }
            | Self::Lte { field, .. }
            | Self::Between { field, .. }
            | Self::In { field, .. }
            | Self::NotIn { field, .. }
            | Self::Empty { field }
            | Self::NotEmpty { field } => field,
        }
    }
}

/// Parse a simple query string:
/// `search=ahmed&status=pending&sort=-created_at&page=2&page_size=25&qty.gt=1&id.in=a,b`
pub fn parse_query(entity: &EntityDef, raw: &[(String, String)]) -> QefroResult<Query> {
    let mut query = Query::default();
    for (key, value) in raw {
        match key.as_str() {
            "search" => query.search = Some(value.clone()),
            "page" => {
                query.page = value.parse().unwrap_or(1);
            }
            "page_size" | "per_page" | "limit" => {
                query.page_size = value.parse().unwrap_or(25);
            }
            "sort" => {
                query.sort = parse_sort(value);
            }
            "fields" => {
                query.fields = Some(
                    value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            "q" => query.search = Some(value.clone()),
            "include_archived" => {
                query.include_archived = matches!(value.as_str(), "1" | "true" | "yes");
            }
            _ => {
                if let Some(filter) = parse_filter(entity, key, value)? {
                    query.filters.push(filter);
                }
            }
        }
    }
    query.sanitize(entity)
}

fn parse_sort(value: &str) -> Vec<Sort> {
    value
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|part| {
            if let Some(field) = part.strip_prefix('-') {
                Sort {
                    field: field.to_string(),
                    dir: SortDir::Desc,
                }
            } else if let Some(field) = part.strip_prefix('+') {
                Sort {
                    field: field.to_string(),
                    dir: SortDir::Asc,
                }
            } else {
                Sort {
                    field: part.to_string(),
                    dir: SortDir::Asc,
                }
            }
        })
        .collect()
}

fn parse_filter(entity: &EntityDef, key: &str, value: &str) -> QefroResult<Option<Filter>> {
    let (field, op) = if let Some((f, op)) = key.split_once('.') {
        (f, op)
    } else if let Some((f, op)) = key.split_once("__") {
        (f, op)
    } else {
        (key, "eq")
    };

    if field == "tenant_id" {
        return Err(QefroError::bad_request("tenant_id is not a client filter"));
    }
    if !entity.is_filterable_field(field) {
        // Ignore unknown keys so clients can pass extra UI params.
        return Ok(None);
    }

    let json_value = coerce_value(entity, field, value)?;
    let filter = match op {
        "eq" | "equals" => Filter::Eq {
            field: field.into(),
            value: json_value,
        },
        "neq" | "ne" | "not_equals" | "not-equals" => Filter::Neq {
            field: field.into(),
            value: json_value,
        },
        "contains" | "like" | "ilike" => Filter::Contains {
            field: field.into(),
            value: value.to_string(),
        },
        "starts_with" | "startswith" | "starts" => Filter::StartsWith {
            field: field.into(),
            value: value.to_string(),
        },
        "gt" => Filter::Gt {
            field: field.into(),
            value: json_value,
        },
        "gte" => Filter::Gte {
            field: field.into(),
            value: json_value,
        },
        "lt" => Filter::Lt {
            field: field.into(),
            value: json_value,
        },
        "lte" => Filter::Lte {
            field: field.into(),
            value: json_value,
        },
        "between" => {
            let mut parts = value.splitn(2, ',');
            let from = coerce_value(entity, field, parts.next().unwrap_or(""))?;
            let to = coerce_value(entity, field, parts.next().unwrap_or(""))?;
            Filter::Between {
                field: field.into(),
                from,
                to,
            }
        }
        "in" => Filter::In {
            field: field.into(),
            values: value
                .split(',')
                .map(|s| coerce_value(entity, field, s.trim()))
                .collect::<QefroResult<Vec<_>>>()?,
        },
        "nin" | "not_in" | "notin" => Filter::NotIn {
            field: field.into(),
            values: value
                .split(',')
                .map(|s| coerce_value(entity, field, s.trim()))
                .collect::<QefroResult<Vec<_>>>()?,
        },
        "empty" | "is_empty" => Filter::Empty {
            field: field.into(),
        },
        "nempty" | "not_empty" | "is_not_empty" => Filter::NotEmpty {
            field: field.into(),
        },
        _ => {
            return Err(QefroError::bad_request(format!(
                "unknown filter operator '{op}'"
            )));
        }
    };
    Ok(Some(filter))
}

fn coerce_value(entity: &EntityDef, field: &str, raw: &str) -> QefroResult<Value> {
    let def = entity.get_field(field);
    let ty = def.map(|f| &f.field_type);
    let value = match ty {
        Some(FieldType::Integer) => {
            let n: i64 = raw
                .parse()
                .map_err(|_| QefroError::bad_request(format!("'{raw}' is not an integer")))?;
            Value::from(n)
        }
        Some(FieldType::Decimal) => {
            let n: f64 = raw
                .parse()
                .map_err(|_| QefroError::bad_request(format!("'{raw}' is not a number")))?;
            serde_json::Number::from_f64(n)
                .map(Value::Number)
                .unwrap_or(Value::String(raw.into()))
        }
        Some(FieldType::Boolean) => Value::Bool(matches!(raw, "true" | "1" | "yes")),
        Some(FieldType::Time) | Some(FieldType::Date) | Some(FieldType::DateTime) => {
            Value::String(raw.to_string())
        }
        _ => Value::String(raw.to_string()),
    };
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qefro_core::{EntityDef, FieldDef};

    fn customer() -> EntityDef {
        EntityDef::new("Customer")
            .field(FieldDef::string("name").searchable())
            .field(FieldDef::string("email"))
            .field(FieldDef::enum_values("status", vec!["pending", "active"]))
            .build()
    }

    #[test]
    fn parse_common_params() {
        let entity = customer();
        let raw = vec![
            ("search".into(), "ahmed".into()),
            ("status".into(), "pending".into()),
            ("sort".into(), "-created_at".into()),
            ("page".into(), "2".into()),
            ("page_size".into(), "25".into()),
        ];
        let q = parse_query(&entity, &raw).unwrap();
        assert_eq!(q.search.as_deref(), Some("ahmed"));
        assert_eq!(q.page, 2);
        assert_eq!(q.page_size, 25);
        assert!(matches!(&q.sort[0], Sort { field, dir: SortDir::Desc } if field == "created_at"));
        assert!(matches!(&q.filters[0], Filter::Eq { field, .. } if field == "status"));
    }

    #[test]
    fn rejects_unknown_sort() {
        let entity = customer();
        let raw = vec![("sort".into(), "password_hash".into())];
        assert!(parse_query(&entity, &raw).is_err());
    }

    #[test]
    fn extra_filter_operators_are_parameterized() {
        let entity = customer();
        let raw = vec![
            ("name.starts_with".into(), "Ah".into()),
            ("status.neq".into(), "pending".into()),
            ("email.empty".into(), "1".into()),
        ];
        let q = parse_query(&entity, &raw).unwrap();
        assert_eq!(q.filters.len(), 3);
        assert!(matches!(&q.filters[0], Filter::StartsWith { .. }));
        assert!(matches!(&q.filters[1], Filter::Neq { .. }));
        assert!(matches!(&q.filters[2], Filter::Empty { .. }));
    }

    #[test]
    fn tenant_id_not_filterable() {
        let entity = customer();
        let raw = vec![("tenant_id".into(), "x".into())];
        assert!(parse_query(&entity, &raw).is_err());
    }
}
