use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use qefro_core::QefroError;
use serde_json::json;

pub fn error_response(err: QefroError) -> Response {
    let status =
        StatusCode::from_u16(err.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = json!({
        "error": err.error_code(),
        "message": err.public_message(),
        "details": err.public_details(),
    });
    (status, Json(body)).into_response()
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
