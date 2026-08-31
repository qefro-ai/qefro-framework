use serde::{Deserialize, Serialize};
use std::fmt;

/// Framework-wide error type. HTTP and agent layers map this to a consistent
/// JSON error envelope. Database crates convert driver errors into `Database`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum QefroError {
    NotFound {
        message: String,
    },
    Validation {
        message: String,
        fields: Vec<FieldError>,
    },
    Forbidden {
        message: String,
    },
    Unauthorized {
        message: String,
    },
    Conflict {
        message: String,
    },
    BadRequest {
        message: String,
    },
    Workflow {
        message: String,
    },
    Business {
        code: String,
        message: String,
    },
    Database {
        message: String,
    },
    RateLimited {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retry_after: Option<u64>,
    },
    Locked {
        message: String,
        fields: Vec<FieldError>,
    },
    AppNotEnabled {
        message: String,
    },
    MigrationRequired {
        message: String,
    },
    PayloadTooLarge {
        message: String,
    },
    Internal {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldError {
    pub field: String,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

impl FieldError {
    pub fn new(
        field: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
            message: message.into(),
            entity: None,
            record: None,
            rule: None,
        }
    }

    pub fn with_entity(mut self, entity: impl Into<String>) -> Self {
        self.entity = Some(entity.into());
        self
    }

    pub fn with_record(mut self, record: impl Into<String>) -> Self {
        self.record = Some(record.into());
        self
    }

    pub fn with_rule(mut self, rule: impl Into<String>) -> Self {
        self.rule = Some(rule.into());
        self
    }
}

impl QefroError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
        }
    }

    pub fn validation(fields: Vec<FieldError>) -> Self {
        Self::Validation {
            message: "validation failed".into(),
            fields,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden {
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized {
            message: message.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
        }
    }

    pub fn workflow(message: impl Into<String>) -> Self {
        Self::Workflow {
            message: message.into(),
        }
    }

    pub fn business(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Business {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::Database {
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::RateLimited {
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn rate_limited_retry(message: impl Into<String>, retry_after_secs: u64) -> Self {
        Self::RateLimited {
            message: message.into(),
            retry_after: Some(retry_after_secs.max(1)),
        }
    }

    pub fn locked(fields: Vec<FieldError>) -> Self {
        Self::Locked {
            message: "document is locked".into(),
            fields,
        }
    }

    pub fn app_not_enabled(message: impl Into<String>) -> Self {
        Self::AppNotEnabled {
            message: message.into(),
        }
    }

    pub fn migration_required(message: impl Into<String>) -> Self {
        Self::MigrationRequired {
            message: message.into(),
        }
    }

    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::PayloadTooLarge {
            message: message.into(),
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound { .. } | Self::AppNotEnabled { .. } => 404,
            Self::Validation { .. } | Self::Locked { .. } => 422,
            Self::Forbidden { .. } => 403,
            Self::Unauthorized { .. } => 401,
            Self::Conflict { .. } | Self::Workflow { .. } | Self::Business { .. } => 409,
            Self::MigrationRequired { .. } => 409,
            Self::BadRequest { .. } => 400,
            Self::RateLimited { .. } => 429,
            Self::PayloadTooLarge { .. } => 413,
            Self::Database { .. } | Self::Internal { .. } => 500,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not_found",
            Self::Validation { .. } => "validation_failed",
            Self::Forbidden { .. } => "forbidden",
            Self::Unauthorized { .. } => "unauthenticated",
            Self::Conflict { .. } => "conflict",
            Self::BadRequest { .. } => "bad_request",
            Self::Workflow { .. } => "invalid_transition",
            Self::Business { code, .. } if code == "automation_failed" => "automation_failed",
            Self::Business { code, .. } if code == "job_failed" => "job_failed",
            Self::Business { code, .. } if code == "scheduling_conflict" => "scheduling_conflict",
            Self::Business { code, .. } if code == "scheduling_capacity" => "scheduling_capacity",
            Self::Business { .. } => "business_rule_failed",
            Self::RateLimited { .. } => "rate_limited",
            Self::Locked { .. } => "locked",
            Self::AppNotEnabled { .. } => "app_not_enabled",
            Self::MigrationRequired { .. } => "migration_required",
            Self::PayloadTooLarge { .. } => "payload_too_large",
            Self::Database { .. } => "dependency_failed",
            Self::Internal { .. } => "internal_error",
        }
    }

    /// Client-facing message. Database and internal errors never include driver text.
    pub fn public_message(&self) -> String {
        match self {
            Self::Database { .. } | Self::Internal { .. } => "an internal error occurred".into(),
            _ => self.to_string(),
        }
    }

    /// Client-facing details. Never includes SQL, credentials, or stack traces.
    pub fn public_details(&self) -> serde_json::Value {
        match self {
            Self::Database { .. } | Self::Internal { .. } => serde_json::json!({}),
            Self::Business { code, message } => {
                serde_json::json!({ "code": code, "message": message })
            }
            Self::Locked { fields, .. } | Self::Validation { fields, .. } => {
                serde_json::json!({ "fields": fields })
            }
            other => serde_json::to_value(other).unwrap_or(serde_json::json!({})),
        }
    }
}

impl fmt::Display for QefroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { message }
            | Self::Forbidden { message }
            | Self::Unauthorized { message }
            | Self::Conflict { message }
            | Self::BadRequest { message }
            | Self::Workflow { message }
            | Self::Business { message, .. }
            | Self::RateLimited { message, .. }
            | Self::AppNotEnabled { message }
            | Self::MigrationRequired { message }
            | Self::PayloadTooLarge { message }
            | Self::Database { message }
            | Self::Internal { message } => write!(f, "{message}"),
            Self::Validation { message, .. } | Self::Locked { message, .. } => {
                write!(f, "{message}")
            }
        }
    }
}

impl std::error::Error for QefroError {}

pub type QefroResult<T> = Result<T, QefroError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_errors_are_not_leaked_to_clients() {
        let err = QefroError::database("SELECT * FROM users WHERE password = 'secret'");
        assert_eq!(err.public_message(), "an internal error occurred");
        let details = err.public_details().to_string();
        assert!(!details.contains("SELECT"));
        assert!(!details.contains("password"));
        assert!(!details.contains("secret"));
    }
}
