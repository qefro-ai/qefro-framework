use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use qefro_core::{OpContext, QefroError, RateLimiter};

pub struct Auth(pub OpContext);

impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| QefroError::unauthorized("missing authorization header"))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| QefroError::unauthorized("expected Bearer token"))?;
        let mut ctx = state.auth.authenticate(token).await?;
        if let Some(rid) = parts
            .headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
        {
            ctx.request_id = rid;
        }
        ctx.ip = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
        ctx.user_agent = parts
            .headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // X-Tenant-ID and query tenant_id are never trusted.
        let _ignored_header = parts.headers.get("x-tenant-id");

        let config = state
            .tenants
            .get_config(ctx.tenant_id)
            .await
            .unwrap_or_default();
        ctx.apply_tenant_config(&config);
        ctx.enabled_apps = state.entitlements.resolve_apps(
            &state.installed_apps,
            &config.enabled_apps,
            config.plan.as_deref(),
        );

        let path = parts.uri.path();
        let key = format!("{}:{}:{}", ctx.tenant_id, ctx.user_id, path);
        let decision = state.rate_limiter.check(&key);
        if !decision.allowed {
            return Err(QefroError::rate_limited_retry(
                "rate limit exceeded",
                decision.retry_after_secs(),
            )
            .into());
        }

        tracing::info!(
            request_id = %ctx.request_id,
            tenant_id = %ctx.tenant_id,
            user_id = %ctx.user_id,
            path,
            "authenticated request"
        );
        qefro_core::MeteringEvent::new(ctx.tenant_id, "api.request", path, ctx.request_id)
            .with_user(ctx.user_id)
            .emit();

        Ok(Auth(ctx))
    }
}
