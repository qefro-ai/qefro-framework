use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use qefro_core::{OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
            "SELECT id, email, name, password_hash, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(&email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::unauthorized("invalid credentials"))?;

        if !verify_password(password, &row.password_hash)? {
            return Err(QefroError::unauthorized("invalid credentials"));
        }

        let memberships = sqlx::query_as::<_, Membership>(
            r#"
            SELECT t.id as tenant_id, t.slug, ut.roles
            FROM user_tenants ut
            JOIN tenants t ON t.id = ut.tenant_id
            WHERE ut.user_id = $1
            "#,
        )
        .bind(row.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;

        if memberships.is_empty() {
            return Err(QefroError::forbidden("user has no tenant membership"));
        }

        let membership = if let Some(slug) = tenant_slug {
            memberships
                .iter()
                .find(|m| m.slug == slug)
                .ok_or_else(|| QefroError::forbidden("not a member of that tenant"))?
                .clone()
        } else {
            memberships[0].clone()
        };

        self.issue_token(row.id, membership.tenant_id, membership.roles)
            .await
    }

    pub async fn switch_tenant(&self, user_id: Uuid, tenant_id: Uuid) -> QefroResult<AuthToken> {
        let membership = sqlx::query_as::<_, Membership>(
            r#"
            SELECT t.id as tenant_id, t.slug, ut.roles
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
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
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

        Ok(OpContext {
            tenant_id: claims.tid,
            user_id: claims.sub,
            roles: claims.roles,
            request_id: Uuid::new_v4(),
            session_id: Some(claims.sid),
            ip: None,
            user_agent: None,
        })
    }

    pub async fn get_user(&self, id: Uuid) -> QefroResult<User> {
        sqlx::query_as::<_, User>(
            "SELECT id, email, name, created_at, updated_at FROM users WHERE id = $1",
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
            &Header::default(),
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
            return QefroError::conflict("email or tenant slug already exists");
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
