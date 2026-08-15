//! Process-level metrics. Tenant identifiers and field values are never included.

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use uuid::Uuid;

static HTTP_REQUESTS: AtomicU64 = AtomicU64::new(0);
static HTTP_ERRORS: AtomicU64 = AtomicU64::new(0);
static HTTP_LATENCY_MS_TOTAL: AtomicU64 = AtomicU64::new(0);

pub fn http_snapshot() -> (u64, u64, u64) {
    (
        HTTP_REQUESTS.load(Ordering::Relaxed),
        HTTP_ERRORS.load(Ordering::Relaxed),
        HTTP_LATENCY_MS_TOTAL.load(Ordering::Relaxed),
    )
}

pub async fn track(mut req: Request, next: Next) -> Response {
    HTTP_REQUESTS.fetch_add(1, Ordering::Relaxed);
    if req.headers().get("x-request-id").is_none() {
        if let Ok(value) = HeaderValue::from_str(&Uuid::new_v4().to_string()) {
            req.headers_mut().insert("x-request-id", value);
        }
    }
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    let start = Instant::now();
    let mut res = next.run(req).await;
    let elapsed = start.elapsed().as_millis() as u64;
    HTTP_LATENCY_MS_TOTAL.fetch_add(elapsed, Ordering::Relaxed);
    let status = res.status().as_u16();
    if status >= 400 {
        HTTP_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
    if res.headers().get("x-request-id").is_none() {
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            res.headers_mut().insert("x-request-id", value);
        }
    }
    tracing::info!(
        request_id,
        method,
        path,
        status,
        duration_ms = elapsed,
        "http"
    );
    res
}
