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
    Internal {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldError {
    pub field: String,
    pub code: String,
    pub message: String,
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
        }
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

    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound { .. } => 404,
            Self::Validation { .. } => 422,
            Self::Forbidden { .. } => 403,
            Self::Unauthorized { .. } => 401,
            Self::Conflict { .. } => 409,
            Self::BadRequest { .. } => 400,
            Self::Workflow { .. } => 409,
            Self::Business { .. } => 409,
            Self::Database { .. } | Self::Internal { .. } => 500,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not_found",
            Self::Validation { .. } => "validation_failed",
            Self::Forbidden { .. } => "forbidden",
            Self::Unauthorized { .. } => "unauthorized",
            Self::Conflict { .. } => "conflict",
            Self::BadRequest { .. } => "bad_request",
            Self::Workflow { .. } => "workflow_error",
            Self::Business { .. } => "business_rule_failed",
            Self::Database { .. } => "database_error",
            Self::Internal { .. } => "internal_error",
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
            | Self::Database { message }
            | Self::Internal { message } => write!(f, "{message}"),
            Self::Validation { message, .. } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for QefroError {}

pub type QefroResult<T> = Result<T, QefroError>;
