use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use qefro_core::{OpContext, QefroError, QefroResult, USER_ENTITY};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::OnceLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: User,
    pub tenant_id: Uuid,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub tid: Uuid,
    pub sid: Uuid,
    pub roles: Vec<String>,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Clone)]
pub struct AuthService {
    pool: PgPool,
    jwt_secret: String,
    token_ttl_hours: i64,
}

impl AuthService {
    pub fn new(pool: PgPool, jwt_secret: impl Into<String>) -> Self {
        Self {
            pool,
            jwt_secret: jwt_secret.into(),
            token_ttl_hours: 12,
        }
    }

    pub async fn register(
        &self,
        name: &str,
        email: &str,
        password: &str,
        tenant_name: &str,
        tenant_slug: &str,
    ) -> QefroResult<AuthToken> {
        if password.len() < 8 {
            return Err(QefroError::bad_request(
                "password must be at least 8 characters",
            ));
        }
        let email = email.trim().to_ascii_lowercase();
        let hash = hash_password(password)?;
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;

        sqlx::query("INSERT INTO tenants (id, name, slug, created_at) VALUES ($1, $2, $3, now())")
            .bind(tenant_id)
            .bind(tenant_name)
            .bind(tenant_slug.to_ascii_lowercase())
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;

        sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, name, created_at, updated_at)
            VALUES ($1, $2, $3, $4, now(), now())
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind(&hash)
        .bind(name)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;

        sqlx::query("INSERT INTO user_tenants (user_id, tenant_id, roles) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind(tenant_id)
            .bind(&["Admin".to_string()][..])
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;

        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;

        self.issue_token(user_id, tenant_id, vec!["Admin".into()])
            .await
    }

    pub async fn login(
        &self,
        email: &str,
        password: &str,
        tenant_slug: Option<&str>,
    ) -> QefroResult<AuthToken> {
        let email = email.trim().to_ascii_lowercase();
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, name, password_hash, enabled, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(&email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;

        // Always verify against a hash so missing users cost Argon2 like real ones.
        let hash = row
            .as_ref()
            .map(|r| r.password_hash.as_str())
            .unwrap_or_else(|| dummy_password_hash());
        let password_ok = verify_password(password, hash)?;
        let Some(row) = row else {
            return Err(QefroError::unauthorized("invalid credentials"));
        };
        if !row.enabled || !password_ok {
            return Err(QefroError::unauthorized("invalid credentials"));
        }

        let memberships = sqlx::query_as::<_, Membership>(
            r#"
            SELECT t.id as tenant_id, t.slug, ut.roles, ut.enabled
            FROM user_tenants ut
            JOIN tenants t ON t.id = ut.tenant_id
            WHERE ut.user_id = $1
            "#,
        )
        .bind(row.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;

        let membership = if let Some(slug) = tenant_slug {
            memberships.iter().find(|m| m.slug == slug)
        } else {
            memberships.first()
        };
        let Some(membership) = membership.filter(|m| m.enabled) else {
            return Err(QefroError::unauthorized("invalid credentials"));
        };

        self.issue_token(row.id, membership.tenant_id, membership.roles.clone())
            .await
    }

    pub async fn switch_tenant(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        current_session: Option<Uuid>,
    ) -> QefroResult<AuthToken> {
        let membership = sqlx::query_as::<_, Membership>(
            r#"
            SELECT t.id as tenant_id, t.slug, ut.roles, ut.enabled
            FROM user_tenants ut
            JOIN tenants t ON t.id = ut.tenant_id
            WHERE ut.user_id = $1 AND ut.tenant_id = $2
            "#,
        )
        .bind(user_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::forbidden("not a member of that tenant"))?;
        if !membership.enabled {
            return Err(QefroError::forbidden("user is disabled in this tenant"));
        }
        if let Some(sid) = current_session {
            self.logout(sid).await?;
        }
        self.issue_token(user_id, membership.tenant_id, membership.roles)
            .await
    }

    pub async fn logout(&self, session_id: Uuid) -> QefroResult<()> {
        sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn authenticate(&self, token: &str) -> QefroResult<OpContext> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.algorithms = vec![Algorithm::HS256];
        validation.validate_exp = true;
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| QefroError::unauthorized("invalid token"))?
        .claims;

        let session = sqlx::query_as::<_, SessionRow>(
            "SELECT id, user_id, tenant_id, expires_at, revoked_at FROM sessions WHERE id = $1",
        )
        .bind(claims.sid)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::unauthorized("session not found"))?;

        if session.revoked_at.is_some() || session.expires_at < Utc::now() {
            return Err(QefroError::unauthorized("session expired"));
        }
        if session.user_id != claims.sub || session.tenant_id != claims.tid {
            return Err(QefroError::unauthorized("token mismatch"));
        }

        let membership = sqlx::query_as::<_, Membership>(
            r#"
            SELECT t.id as tenant_id, t.slug, ut.roles, ut.enabled
            FROM user_tenants ut
            JOIN tenants t ON t.id = ut.tenant_id
            WHERE ut.user_id = $1 AND ut.tenant_id = $2
            "#,
        )
        .bind(claims.sub)
        .bind(claims.tid)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::unauthorized("session expired"))?;
        if !membership.enabled {
            return Err(QefroError::unauthorized("session expired"));
        }
        let enabled: bool = sqlx::query_scalar("SELECT enabled FROM users WHERE id = $1")
            .bind(claims.sub)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?
            .unwrap_or(false);
        if !enabled {
            return Err(QefroError::unauthorized("session expired"));
        }
        let actor_name: Option<String> = sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
            .bind(claims.sub)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;

        let mut ctx = OpContext::new(claims.tid, claims.sub, membership.roles);
        ctx.session_id = Some(claims.sid);
        ctx.actor_name = actor_name;
        Ok(ctx)
    }

    pub async fn get_user(&self, id: Uuid) -> QefroResult<User> {
        sqlx::query_as::<_, User>(
            "SELECT id, email, name, enabled, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("user not found"))
    }

    pub async fn add_membership(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        roles: Vec<String>,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            INSERT INTO user_tenants (user_id, tenant_id, roles)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id, tenant_id) DO UPDATE SET roles = EXCLUDED.roles
            "#,
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(&roles)
        .execute(&self.pool)
        .await
        .map_err(map_db)?;
        Ok(())
    }

    pub async fn create_user_in_tenant(
        &self,
        tenant_id: Uuid,
        name: &str,
        email: &str,
        password: &str,
        roles: Vec<String>,
    ) -> QefroResult<User> {
        let email = email.trim().to_ascii_lowercase();
        let hash = hash_password(password)?;
        let user_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, name, created_at, updated_at)
            VALUES ($1, $2, $3, $4, now(), now())
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind(&hash)
        .bind(name)
        .execute(&self.pool)
        .await
        .map_err(map_db)?;
        self.add_membership(user_id, tenant_id, roles).await?;
        self.get_user(user_id).await
    }

    /// Tenant-scoped User record for EntityService. Never includes password_hash.
    pub async fn get_tenant_user(&self, tenant_id: Uuid, id: Uuid) -> QefroResult<Value> {
        let row = sqlx::query_as::<_, TenantUserRow>(
            r#"
            SELECT u.id, u.email, u.name, u.created_at, u.updated_at,
                   (u.enabled AND ut.enabled) AS enabled, ut.roles
            FROM user_tenants ut
            JOIN users u ON u.id = ut.user_id
            WHERE ut.tenant_id = $1 AND u.id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("user not found"))?;
        Ok(tenant_user_json(&row))
    }

    pub async fn list_tenant_users(
        &self,
        tenant_id: Uuid,
        search: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> QefroResult<(Vec<Value>, i64)> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 200);
        let offset = (page - 1) * page_size;
        let like = search
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{s}%"));
        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM user_tenants ut
            JOIN users u ON u.id = ut.user_id
            WHERE ut.tenant_id = $1
              AND ($2::text IS NULL OR u.name ILIKE $2 OR u.email ILIKE $2)
            "#,
        )
        .bind(tenant_id)
        .bind(like.as_deref())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let rows = sqlx::query_as::<_, TenantUserRow>(
            r#"
            SELECT u.id, u.email, u.name, u.created_at, u.updated_at,
                   (u.enabled AND ut.enabled) AS enabled, ut.roles
            FROM user_tenants ut
            JOIN users u ON u.id = ut.user_id
            WHERE ut.tenant_id = $1
              AND ($2::text IS NULL OR u.name ILIKE $2 OR u.email ILIKE $2)
            ORDER BY u.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(tenant_id)
        .bind(like.as_deref())
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok((rows.iter().map(tenant_user_json).collect(), total))
    }

    pub async fn create_tenant_user(&self, ctx: &OpContext, data: &Value) -> QefroResult<Value> {
        let name = data
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| QefroError::bad_request("name is required"))?;
        let email = data
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("email is required"))?;
        let password = data
            .get("password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("password is required"))?;
        if password.len() < 8 {
            return Err(QefroError::bad_request(
                "password must be at least 8 characters",
            ));
        }
        let roles = parse_roles(data.get("roles"));
        if roles.iter().any(|r| r.eq_ignore_ascii_case("Admin")) && !ctx.is_admin() {
            return Err(QefroError::forbidden(
                "only Admin can assign the Admin role",
            ));
        }
        let user = self
            .create_user_in_tenant(ctx.tenant_id, name, email, password, roles)
            .await?;
        if let Some(enabled) = data.get("enabled").and_then(|v| v.as_bool()) {
            if !enabled {
                self.set_membership_enabled(ctx, user.id, false).await?;
            }
        }
        self.get_tenant_user(ctx.tenant_id, user.id).await
    }

    pub async fn update_tenant_user(
        &self,
        ctx: &OpContext,
        id: Uuid,
        patch: &Value,
    ) -> QefroResult<Value> {
        let _ = self.get_tenant_user(ctx.tenant_id, id).await?;
        if let Some(name) = patch.get("name").and_then(|v| v.as_str()) {
            sqlx::query("UPDATE users SET name = $1, updated_at = now() WHERE id = $2")
                .bind(name)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
        }
        if let Some(email) = patch.get("email").and_then(|v| v.as_str()) {
            let email = email.trim().to_ascii_lowercase();
            sqlx::query("UPDATE users SET email = $1, updated_at = now() WHERE id = $2")
                .bind(email)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(map_db)?;
        }
        if let Some(password) = patch.get("password").and_then(|v| v.as_str()) {
            if !password.is_empty() {
                if password.len() < 8 {
                    return Err(QefroError::bad_request(
                        "password must be at least 8 characters",
                    ));
                }
                let hash = hash_password(password)?;
                sqlx::query(
                    "UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2",
                )
                .bind(hash)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            }
        }
        if patch.get("roles").is_some() {
            if !ctx.is_admin() {
                return Err(QefroError::forbidden("only Admin can assign roles"));
            }
            let roles = parse_roles(patch.get("roles"));
            sqlx::query("UPDATE user_tenants SET roles = $1 WHERE user_id = $2 AND tenant_id = $3")
                .bind(&roles)
                .bind(id)
                .bind(ctx.tenant_id)
                .execute(&self.pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
        }
        if let Some(enabled) = patch.get("enabled").and_then(|v| v.as_bool()) {
            self.set_membership_enabled(ctx, id, enabled).await?;
        }
        self.get_tenant_user(ctx.tenant_id, id).await
    }

    pub async fn set_membership_enabled(
        &self,
        ctx: &OpContext,
        id: Uuid,
        enabled: bool,
    ) -> QefroResult<()> {
        if id == ctx.user_id && !enabled {
            return Err(QefroError::bad_request("cannot disable your own account"));
        }
        let result = sqlx::query(
            "UPDATE user_tenants SET enabled = $1 WHERE user_id = $2 AND tenant_id = $3",
        )
        .bind(enabled)
        .bind(id)
        .bind(ctx.tenant_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        if result.rows_affected() == 0 {
            return Err(QefroError::not_found("user not found"));
        }
        if !enabled {
            sqlx::query(
                "UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND tenant_id = $2 AND revoked_at IS NULL",
            )
            .bind(id)
            .bind(ctx.tenant_id)
            .execute(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn remove_tenant_membership(&self, ctx: &OpContext, id: Uuid) -> QefroResult<Value> {
        if id == ctx.user_id {
            return Err(QefroError::bad_request(
                "cannot remove your own tenant membership",
            ));
        }
        let current = self.get_tenant_user(ctx.tenant_id, id).await?;
        sqlx::query("DELETE FROM user_tenants WHERE user_id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(ctx.tenant_id)
            .execute(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        sqlx::query(
            "UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND tenant_id = $2 AND revoked_at IS NULL",
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(current)
    }

    async fn issue_token(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        roles: Vec<String>,
    ) -> QefroResult<AuthToken> {
        let session_id = Uuid::new_v4();
        let raw = Uuid::new_v4().to_string();
        let token_hash = sha256_hex(&raw);
        let expires_at = Utc::now() + Duration::hours(self.token_ttl_hours);
        sqlx::query(
            r#"
            INSERT INTO sessions (id, user_id, tenant_id, token_hash, expires_at, created_at)
            VALUES ($1, $2, $3, $4, $5, now())
            "#,
        )
        .bind(session_id)
        .bind(user_id)
        .bind(tenant_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;

        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id,
            tid: tenant_id,
            sid: session_id,
            roles: roles.clone(),
            iat: now,
            exp: expires_at.timestamp(),
        };
        let access_token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| QefroError::internal(e.to_string()))?;

        let user = self.get_user(user_id).await?;
        Ok(AuthToken {
            access_token,
            token_type: "Bearer".into(),
            expires_in: self.token_ttl_hours * 3600,
            user,
            tenant_id,
            roles,
        })
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    #[allow(dead_code)]
    email: String,
    #[allow(dead_code)]
    name: String,
    password_hash: String,
    enabled: bool,
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
    #[allow(dead_code)]
    updated_at: DateTime<Utc>,
}

#[derive(Clone, sqlx::FromRow)]
struct Membership {
    tenant_id: Uuid,
    slug: String,
    roles: Vec<String>,
    enabled: bool,
}

#[derive(sqlx::FromRow)]
struct TenantUserRow {
    id: Uuid,
    email: String,
    name: String,
    enabled: bool,
    roles: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn tenant_user_json(row: &TenantUserRow) -> Value {
    json!({
        "id": row.id,
        "email": row.email,
        "name": row.name,
        "enabled": row.enabled,
        "roles": row.roles,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    })
}

fn parse_roles(value: Option<&Value>) -> Vec<String> {
    let mut roles: Vec<String> = match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) => s
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    };
    let mut unique = Vec::new();
    for role in roles.drain(..) {
        if unique
            .iter()
            .all(|existing: &String| !existing.eq_ignore_ascii_case(&role))
        {
            unique.push(role);
        }
    }
    if unique.is_empty() {
        vec!["Staff".into()]
    } else {
        unique
    }
}

/// Extension point for inviting a person to create a login. V1 does not send
/// mail or persist invitation rows — apps supply an implementation.
#[async_trait::async_trait]
pub trait InvitationSender: Send + Sync {
    async fn send(&self, ctx: &OpContext, email: &str, roles: &[String]) -> QefroResult<()>;
}

/// Documented no-op. Replace with SMTP / queue in the application.
pub struct NoopInvitationSender;

#[async_trait::async_trait]
impl InvitationSender for NoopInvitationSender {
    async fn send(&self, _ctx: &OpContext, _email: &str, _roles: &[String]) -> QefroResult<()> {
        Err(QefroError::bad_request(
            "invitations are not configured; create a User through EntityService (POST /api/v1/users)",
        ))
    }
}

#[allow(dead_code)]
fn _identity_entity_name() -> &'static str {
    USER_ENTITY
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    #[allow(dead_code)]
    id: Uuid,
    user_id: Uuid,
    tenant_id: Uuid,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

fn dummy_password_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        hash_password("qefro-timing-dummy").unwrap_or_else(|_| {
            "$argon2id$v=19$m=19456,t=2,p=1$cWVmcm90aW1pbmdzYWx0$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into()
        })
    })
}

fn hash_password(password: &str) -> QefroResult<String> {
    use argon2::password_hash::{rand_core::OsRng, SaltString};
    use argon2::{Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| QefroError::internal(e.to_string()))
}

fn verify_password(password: &str, hash: &str) -> QefroResult<bool> {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let parsed = PasswordHash::new(hash).map_err(|e| QefroError::internal(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn map_db(err: sqlx::Error) -> QefroError {
    if let sqlx::Error::Database(db) = &err {
        if db.code().as_deref() == Some("23505") {
            return QefroError::conflict("could not create account");
        }
    }
    QefroError::database(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let hash = hash_password("s3cret-pass").unwrap();
        assert!(verify_password("s3cret-pass", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }
}
