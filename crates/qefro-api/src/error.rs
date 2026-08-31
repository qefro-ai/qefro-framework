use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use qefro_core::QefroError;
use serde_json::json;

pub fn error_response(err: QefroError) -> Response {
    let status =
        StatusCode::from_u16(err.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if status.is_server_error() {
        tracing::error!(error = %err, code = err.error_code(), "internal request failure");
    }
    let retry_after = match &err {
        QefroError::RateLimited { retry_after, .. } => *retry_after,
        _ => None,
    };
    let mut body = json!({
        "error": err.error_code(),
        "message": err.public_message(),
        "details": err.public_details(),
    });
    if let QefroError::Validation { fields, .. } | QefroError::Locked { fields, .. } = &err {
        body["fields"] = json!(fields);
        body["nested"] = nest_field_errors(fields);
    }
    let mut res = (status, Json(body)).into_response();
    if let Some(secs) = retry_after {
        if let Ok(value) = HeaderValue::from_str(&secs.to_string()) {
            res.headers_mut().insert(header::RETRY_AFTER, value);
        }
        res.headers_mut().insert(
            header::HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("0"),
        );
    }
    res
}

fn nest_field_errors(fields: &[qefro_core::FieldError]) -> serde_json::Value {
    let mut root = serde_json::Map::new();
    for err in fields {
        let mut parts = err.field.split('.').peekable();
        let mut cursor = &mut root;
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                cursor.insert(part.to_string(), json!(err.message.clone()));
                break;
            }
            let next = cursor.entry(part.to_string()).or_insert_with(|| json!({}));
            cursor = next.as_object_mut().unwrap();
        }
    }
    serde_json::Value::Object(root)
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        error_response(self.0)
    }
}

#[derive(Debug)]
pub struct ApiError(pub QefroError);

impl From<QefroError> for ApiError {
    fn from(value: QefroError) -> Self {
        Self(value)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
