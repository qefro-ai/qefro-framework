use qefro_core::{
    quote_ident, EntityDef, EntityRegistry, FieldType, QefroError, QefroResult, RelationKind,
};
use sqlx::PgPool;

const SYSTEM_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS tenants (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS user_tenants (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    roles TEXT[] NOT NULL DEFAULT ARRAY['Staff']::TEXT[],
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, tenant_id)
);

CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS sessions_user_idx ON sessions(user_id);

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID,
    entity TEXT NOT NULL,
    entity_id UUID,
    action TEXT NOT NULL,
    old_values JSONB,
    new_values JSONB,
    request_id UUID,
    ip TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS audit_logs_tenant_idx ON audit_logs(tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS audit_logs_entity_idx ON audit_logs(tenant_id, entity, entity_id);
CREATE INDEX IF NOT EXISTS audit_logs_actor_idx ON audit_logs(tenant_id, user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS tenant_settings (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    branding JSONB NOT NULL DEFAULT '{}'::jsonb,
    ui_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    enabled_apps TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    business_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS jobs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID,
    name TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    run_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS jobs_poll_idx ON jobs (status, run_at);
CREATE INDEX IF NOT EXISTS jobs_tenant_idx ON jobs (tenant_id, created_at DESC);

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS idempotency_key TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS jobs_idemp_uidx
    ON jobs (tenant_id, name, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

ALTER TABLE tenant_settings ADD COLUMN IF NOT EXISTS feature_flags JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE tenant_settings ADD COLUMN IF NOT EXISTS plan TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE user_tenants ADD COLUMN IF NOT EXISTS enabled BOOLEAN NOT NULL DEFAULT true;

CREATE TABLE IF NOT EXISTS saved_filters (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    entity TEXT NOT NULL,
    name TEXT NOT NULL,
    query JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS saved_filters_scope_idx ON saved_filters (tenant_id, user_id, entity);

CREATE TABLE IF NOT EXISTS document_sequences (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    entity TEXT NOT NULL,
    period TEXT NOT NULL,
    last_value BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, entity, period)
);

CREATE TABLE IF NOT EXISTS blobs (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size BIGINT NOT NULL,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, key)
);

CREATE TABLE IF NOT EXISTS qefro_apps (
    name TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT 'catalog',
    status TEXT NOT NULL DEFAULT 'installed',
    framework_version TEXT,
    api_version TEXT NOT NULL DEFAULT '1',
    dependencies JSONB NOT NULL DEFAULT '{}'::jsonb,
    package_sha256 TEXT,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS qefro_app_versions (
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'catalog',
    package_sha256 TEXT,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (name, version)
);

CREATE TABLE IF NOT EXISTS qefro_app_migrations (
    app TEXT NOT NULL,
    version TEXT NOT NULL,
    name TEXT NOT NULL,
    destructive BOOLEAN NOT NULL DEFAULT false,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app, version, name)
);

CREATE TABLE IF NOT EXISTS qefro_app_events (
    id UUID PRIMARY KEY,
    tenant_id UUID REFERENCES tenants(id) ON DELETE SET NULL,
    user_id UUID,
    app TEXT NOT NULL,
    version TEXT,
    event TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS qefro_app_events_app_idx ON qefro_app_events (app, created_at DESC);

CREATE TABLE IF NOT EXISTS qefro_app_seeds (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    app TEXT NOT NULL,
    kind TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, app, kind)
);

CREATE TABLE IF NOT EXISTS qefro_studio_drafts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    target TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'draft',
    summary TEXT NOT NULL DEFAULT '',
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS qefro_studio_drafts_tenant_idx
    ON qefro_studio_drafts (tenant_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS qefro_studio_versions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    target TEXT NOT NULL,
    version INT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    summary TEXT NOT NULL DEFAULT '',
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, kind, target, version)
);

CREATE INDEX IF NOT EXISTS qefro_studio_versions_target_idx
    ON qefro_studio_versions (tenant_id, kind, target, version DESC);

CREATE TABLE IF NOT EXISTS qefro_attachments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    entity TEXT NOT NULL,
    record_id UUID NOT NULL,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size BIGINT NOT NULL,
    storage_key TEXT NOT NULL,
    uploaded_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    description TEXT
);
CREATE INDEX IF NOT EXISTS qefro_attachments_record_idx
    ON qefro_attachments (tenant_id, entity, record_id);
CREATE INDEX IF NOT EXISTS qefro_attachments_created_idx
    ON qefro_attachments (tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS qefro_attachments_uploader_idx
    ON qefro_attachments (tenant_id, uploaded_by);
ALTER TABLE qefro_attachments ADD COLUMN IF NOT EXISTS description TEXT;
CREATE INDEX IF NOT EXISTS qefro_attachments_filename_idx
    ON qefro_attachments (tenant_id, filename);

CREATE TABLE IF NOT EXISTS qefro_operation_runs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID,
    entity TEXT NOT NULL,
    entity_id UUID NOT NULL,
    operation TEXT NOT NULL,
    status TEXT NOT NULL,
    request_id UUID,
    idempotency_key TEXT,
    progress INT NOT NULL DEFAULT 0,
    result JSONB,
    error TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS qefro_operation_runs_idemp_uidx
    ON qefro_operation_runs (tenant_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS qefro_operation_runs_record_idx
    ON qefro_operation_runs (tenant_id, entity, entity_id, created_at DESC);
CREATE INDEX IF NOT EXISTS qefro_operation_runs_status_idx
    ON qefro_operation_runs (tenant_id, status, created_at DESC);

CREATE TABLE IF NOT EXISTS qefro_notifications (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    entity TEXT,
    record_id UUID,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS qefro_notifications_user_idx
    ON qefro_notifications (tenant_id, user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS qefro_communications (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    entity TEXT NOT NULL,
    entity_id UUID NOT NULL,
    template TEXT NOT NULL,
    channel TEXT NOT NULL,
    purpose TEXT NOT NULL DEFAULT 'transactional',
    status TEXT NOT NULL DEFAULT 'queued',
    recipient TEXT,
    recipient_user_id UUID,
    event_id UUID,
    attempts INT NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS qefro_communications_idemp
    ON qefro_communications (tenant_id, template, entity_id, event_id, channel);
CREATE INDEX IF NOT EXISTS qefro_communications_record_idx
    ON qefro_communications (tenant_id, entity, entity_id, created_at DESC);
CREATE INDEX IF NOT EXISTS qefro_communications_status_idx
    ON qefro_communications (tenant_id, status, created_at DESC);

CREATE TABLE IF NOT EXISTS qefro_webhook_deliveries (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    webhook TEXT NOT NULL,
    event TEXT NOT NULL,
    event_id UUID NOT NULL,
    target TEXT NOT NULL,
    status_code INT,
    success BOOLEAN NOT NULL DEFAULT false,
    attempts INT NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS qefro_webhook_deliveries_idx
    ON qefro_webhook_deliveries (tenant_id, webhook, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS qefro_webhook_deliveries_idemp
    ON qefro_webhook_deliveries (tenant_id, webhook, event_id);

ALTER TABLE qefro_app_migrations ADD COLUMN IF NOT EXISTS checksum TEXT;
ALTER TABLE qefro_app_migrations ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'applied';
ALTER TABLE qefro_app_migrations ADD COLUMN IF NOT EXISTS error TEXT;

CREATE TABLE IF NOT EXISTS qefro_outbox (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    event_name TEXT NOT NULL,
    entity TEXT NOT NULL,
    entity_id UUID NOT NULL,
    user_id UUID,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS qefro_outbox_pending_idx
    ON qefro_outbox (created_at)
    WHERE published_at IS NULL;

CREATE TABLE IF NOT EXISTS qefro_automation_executions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    automation_id TEXT NOT NULL,
    event_id UUID NOT NULL,
    execution_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'running',
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, automation_id, event_id)
);
CREATE INDEX IF NOT EXISTS qefro_automation_exec_tenant_idx
    ON qefro_automation_executions (tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS qefro_automation_exec_status_idx
    ON qefro_automation_executions (tenant_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS qefro_automation_exec_event_idx
    ON qefro_automation_executions (event_id);
ALTER TABLE qefro_automation_executions ADD COLUMN IF NOT EXISTS step_index INT NOT NULL DEFAULT 0;
ALTER TABLE qefro_automation_executions ADD COLUMN IF NOT EXISTS cursor JSONB NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE qefro_automation_executions ADD COLUMN IF NOT EXISTS steps_log JSONB NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE qefro_automation_executions ADD COLUMN IF NOT EXISTS def_snapshot JSONB;
ALTER TABLE qefro_automation_executions ADD COLUMN IF NOT EXISTS entity TEXT;
ALTER TABLE qefro_automation_executions ADD COLUMN IF NOT EXISTS record_id UUID;
ALTER TABLE qefro_automation_executions ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();


CREATE TABLE IF NOT EXISTS qefro_activity (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    actor_id UUID,
    actor_name TEXT,
    activity_type TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS qefro_activity_record_idx
    ON qefro_activity (tenant_id, entity_type, entity_id, created_at DESC);
CREATE INDEX IF NOT EXISTS qefro_activity_actor_idx
    ON qefro_activity (tenant_id, actor_id, created_at DESC);
CREATE INDEX IF NOT EXISTS qefro_activity_created_idx
    ON qefro_activity (tenant_id, created_at DESC);
"#;

pub fn entity_ddl(entity: &EntityDef) -> QefroResult<String> {
    entity.validate_idents()?;
    let table = quote_ident(&entity.table)?;
    let mut cols = vec!["\"id\" UUID PRIMARY KEY".to_string()];
    if entity.tenant_owned {
        cols.push("\"tenant_id\" UUID NOT NULL REFERENCES tenants(id)".to_string());
    }
    cols.push("\"created_at\" TIMESTAMPTZ NOT NULL DEFAULT now()".to_string());
    cols.push("\"updated_at\" TIMESTAMPTZ NOT NULL DEFAULT now()".to_string());
    cols.push("\"created_by\" UUID".to_string());
    cols.push("\"updated_by\" UUID".to_string());
    if entity.soft_delete {
        cols.push("\"deleted_at\" TIMESTAMPTZ".to_string());
    }
    if entity.archives() {
        cols.push("\"archived_at\" TIMESTAMPTZ".to_string());
    }

    for field in entity.stored_fields() {
        let col = quote_ident(&field.column_name())?;
        let mut sql_ty = field.field_type.sql_type().to_string();
        if !field.nullable && !field.computed {
            sql_ty.push_str(" NOT NULL");
        }
        if let FieldType::Enum { values } = &field.field_type {
            let list = values
                .iter()
                .map(|v| format!("'{}'", v.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ");
            sql_ty.push_str(&format!(" CHECK ({col} IN ({list}))"));
        }
        cols.push(format!("{col} {sql_ty}"));
    }

    let mut ddl = format!(
        "CREATE TABLE IF NOT EXISTS {table} (\n    {}\n);",
        cols.join(",\n    ")
    );

    if entity.tenant_owned {
        ddl.push_str(&format!(
            "\nCREATE INDEX IF NOT EXISTS {}_tenant_idx ON {table} (\"tenant_id\");",
            entity.table
        ));
        ddl.push_str(&format!(
            "\nCREATE INDEX IF NOT EXISTS {}_created_idx ON {table} (\"tenant_id\", \"created_at\" DESC);",
            entity.table
        ));
        if entity.singleton {
            if entity.soft_delete {
                ddl.push_str(&format!(
                    "\nCREATE UNIQUE INDEX IF NOT EXISTS {}_singleton_uidx ON {table} (\"tenant_id\") WHERE \"deleted_at\" IS NULL;",
                    entity.table
                ));
            } else {
                ddl.push_str(&format!(
                    "\nCREATE UNIQUE INDEX IF NOT EXISTS {}_singleton_uidx ON {table} (\"tenant_id\");",
                    entity.table
                ));
            }
        }
    }
    if let Some(child_of) = &entity.child_of {
        if let Some(fk) = entity.parent_fk(&child_of.parent_entity) {
            let fk_col = quote_ident(&fk.column_name())?;
            ddl.push_str(&format!(
                "\nCREATE INDEX IF NOT EXISTS {}_{}_parent_idx ON {table} (\"tenant_id\", {fk_col});",
                entity.table,
                fk.column_name(),
            ));
        }
    }
    if entity.soft_delete {
        ddl.push_str(&format!(
            "\nCREATE INDEX IF NOT EXISTS {}_deleted_idx ON {table} (\"deleted_at\");",
            entity.table
        ));
    }
    if entity.archives() {
        ddl.push_str(&format!(
            "\nCREATE INDEX IF NOT EXISTS {}_archived_idx ON {table} (\"archived_at\");",
            entity.table
        ));
    }
    for field in entity.stored_fields() {
        if field.indexed || field.unique {
            let col = quote_ident(&field.column_name())?;
            if field.unique && entity.tenant_owned {
                if entity.soft_delete {
                    ddl.push_str(&format!(
                        "\nCREATE UNIQUE INDEX IF NOT EXISTS {}_{}_uidx ON {table} (\"tenant_id\", {col}) WHERE \"deleted_at\" IS NULL;",
                        entity.table,
                        field.column_name()
                    ));
                } else {
                    ddl.push_str(&format!(
                        "\nCREATE UNIQUE INDEX IF NOT EXISTS {}_{}_uidx ON {table} (\"tenant_id\", {col});",
                        entity.table,
                        field.column_name()
                    ));
                }
            } else if field.unique {
                ddl.push_str(&format!(
                    "\nCREATE UNIQUE INDEX IF NOT EXISTS {}_{}_uidx ON {table} ({col});",
                    entity.table,
                    field.column_name()
                ));
            } else {
                ddl.push_str(&format!(
                    "\nCREATE INDEX IF NOT EXISTS {}_{}_idx ON {table} ({col});",
                    entity.table,
                    field.column_name()
                ));
            }
        }
        if field.searchable || field.name == "status" {
            if !(field.indexed || field.unique) {
                let col = quote_ident(&field.column_name())?;
                if entity.tenant_owned {
                    ddl.push_str(&format!(
                        "\nCREATE INDEX IF NOT EXISTS {}_{}_search_idx ON {table} (\"tenant_id\", {col});",
                        entity.table,
                        field.column_name()
                    ));
                } else {
                    ddl.push_str(&format!(
                        "\nCREATE INDEX IF NOT EXISTS {}_{}_search_idx ON {table} ({col});",
                        entity.table,
                        field.column_name()
                    ));
                }
            }
        }
        if let Some(rel) = &field.relation {
            if rel.kind == RelationKind::ManyToOne {
                // FK is added after all tables exist.
            }
        }
    }
    Ok(ddl)
}

pub async fn apply_schema(pool: &PgPool, registry: &EntityRegistry) -> QefroResult<()> {
    for stmt in split_sql(SYSTEM_DDL) {
        sqlx::query(stmt)
            .execute(pool)
            .await
            .map_err(|e| QefroError::database(format!("system schema: {e}")))?;
    }

    for entity in registry.list() {
        if entity.skip_ddl {
            continue;
        }
        let ddl = entity_ddl(&entity)?;
        let stmts = split_sql(&ddl);
        if let Some(create) = stmts.first() {
            sqlx::query(create)
                .execute(pool)
                .await
                .map_err(|e| QefroError::database(format!("schema {}: {e}", entity.name)))?;
        }
    }
    apply_missing_columns(pool, registry).await?;
    apply_column_nullability(pool, registry).await?;
    for entity in registry.list() {
        if entity.skip_ddl {
            continue;
        }
        let ddl = entity_ddl(&entity)?;
        for stmt in split_sql(&ddl).into_iter().skip(1) {
            sqlx::query(stmt)
                .execute(pool)
                .await
                .map_err(|e| QefroError::database(format!("schema {}: {e}", entity.name)))?;
        }
    }

    apply_foreign_keys(pool, registry).await?;
    apply_junction_tables(pool, registry).await?;
    apply_enum_checks(pool, registry).await?;
    Ok(())
}

async fn apply_missing_columns(pool: &PgPool, registry: &EntityRegistry) -> QefroResult<()> {
    for entity in registry.list() {
        if entity.skip_ddl {
            continue;
        }
        let table = quote_ident(&entity.table)?;
        for field in entity.stored_fields() {
            let col = quote_ident(&field.column_name())?;
            let sql = format!(
                "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS {col} {}",
                field.field_type.sql_type()
            );
            sqlx::query(&sql).execute(pool).await.map_err(|e| {
                QefroError::database(format!("add column {}.{}: {e}", entity.name, field.name))
            })?;
        }
        if entity.archives() {
            let sql =
                format!("ALTER TABLE {table} ADD COLUMN IF NOT EXISTS \"archived_at\" TIMESTAMPTZ");
            sqlx::query(&sql).execute(pool).await.map_err(|e| {
                QefroError::database(format!("add column {}.archived_at: {e}", entity.name))
            })?;
        }
    }
    Ok(())
}

const SYSTEM_COLUMNS: &[&str] = &[
    "id",
    "tenant_id",
    "created_at",
    "updated_at",
    "created_by",
    "updated_by",
    "deleted_at",
    "archived_at",
];

/// `CREATE TABLE IF NOT EXISTS` never drops leftover columns. A previous
/// metadata version may have left `parent_id NOT NULL` after the child FK was
/// renamed to `invoice_id` / `order_id`. Relax unmanaged and computed columns
/// so inserts are not blocked by schema that the entity no longer owns.
async fn apply_column_nullability(pool: &PgPool, registry: &EntityRegistry) -> QefroResult<()> {
    use std::collections::HashSet;
    for entity in registry.list() {
        if entity.skip_ddl {
            continue;
        }
        ident_check(&entity.table)?;
        let table = quote_ident(&entity.table)?;
        let rows: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT column_name::text, is_nullable::text
            FROM information_schema.columns
            WHERE table_schema = 'public' AND table_name = $1
            "#,
        )
        .bind(&entity.table)
        .fetch_all(pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let managed: HashSet<String> = entity
            .stored_fields()
            .iter()
            .map(|f| f.column_name())
            .chain(SYSTEM_COLUMNS.iter().map(|s| (*s).to_string()))
            .collect();
        for (name, nullable) in rows {
            if nullable != "NO" {
                continue;
            }
            ident_check(&name)?;
            let unmanaged = !managed.contains(&name);
            let computed = entity
                .stored_fields()
                .iter()
                .any(|f| f.column_name() == name && f.computed);
            if !unmanaged && !computed {
                continue;
            }
            let col = quote_ident(&name)?;
            let sql = format!("ALTER TABLE {table} ALTER COLUMN {col} DROP NOT NULL");
            sqlx::query(&sql).execute(pool).await.map_err(|e| {
                QefroError::database(format!("relax {}.{}: {e}", entity.table, name))
            })?;
        }
    }
    Ok(())
}

/// CREATE TABLE IF NOT EXISTS does not refresh CHECK constraints. Re-apply
/// enum CHECKs so status values can grow without leftover rejections.
async fn apply_enum_checks(pool: &PgPool, registry: &EntityRegistry) -> QefroResult<()> {
    for entity in registry.list() {
        if entity.skip_ddl {
            continue;
        }
        ident_check(&entity.table)?;
        for field in entity.stored_fields() {
            let FieldType::Enum { values } = &field.field_type else {
                continue;
            };
            ident_check(&field.column_name())?;
            let names: Vec<String> = sqlx::query_scalar(
                r#"
                SELECT con.conname
                FROM pg_constraint con
                JOIN pg_attribute att
                  ON att.attrelid = con.conrelid AND att.attnum = ANY (con.conkey)
                WHERE con.contype = 'c'
                  AND con.conrelid = $1::regclass
                  AND att.attname = $2
                "#,
            )
            .bind(&entity.table)
            .bind(field.column_name())
            .fetch_all(pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
            let table = quote_ident(&entity.table)?;
            for name in &names {
                ident_check(name)?;
                let drop = format!(
                    "ALTER TABLE {table} DROP CONSTRAINT IF EXISTS {}",
                    quote_ident(name)?
                );
                sqlx::query(&drop)
                    .execute(pool)
                    .await
                    .map_err(|e| QefroError::database(e.to_string()))?;
            }
            let col = quote_ident(&field.column_name())?;
            let constraint = format!("{}_{}_check", entity.table, field.column_name());
            ident_check(&constraint)?;
            let list = values
                .iter()
                .map(|v| format!("'{}'", v.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ");
            let add = format!(
                "ALTER TABLE {table} ADD CONSTRAINT {} CHECK ({col} IN ({list}))",
                quote_ident(&constraint)?
            );
            sqlx::query(&add).execute(pool).await.or_else(|e| {
                let msg = e.to_string();
                if msg.contains("already exists") {
                    Ok(Default::default())
                } else {
                    Err(QefroError::database(e.to_string()))
                }
            })?;
        }
    }
    Ok(())
}

async fn apply_foreign_keys(pool: &PgPool, registry: &EntityRegistry) -> QefroResult<()> {
    for entity in registry.list() {
        if entity.skip_ddl {
            continue;
        }
        for field in entity.stored_fields() {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ManyToOne {
                continue;
            }
            let target = match registry.try_get(&rel.target_entity) {
                Some(t) => t,
                None => continue,
            };
            let table = quote_ident(&entity.table)?;
            let col = quote_ident(&field.column_name())?;
            let target_table = quote_ident(&target.table)?;
            let constraint = format!("fk_{}_{}", entity.table, field.column_name());
            crate::schema::ident_check(&constraint)?;
            let sql = format!(
                "ALTER TABLE {table} DROP CONSTRAINT IF EXISTS \"{constraint}\"; \
                 ALTER TABLE {table} ADD CONSTRAINT \"{constraint}\" FOREIGN KEY ({col}) REFERENCES {target_table} (\"id\");"
            );
            // DROP + ADD in one round-trip can fail on first run; do separately.
            let drop = format!("ALTER TABLE {table} DROP CONSTRAINT IF EXISTS \"{constraint}\"");
            let on_delete = if entity.child_of.as_ref().map(|c| c.parent_entity.as_str())
                == Some(rel.target_entity.as_str())
            {
                " ON DELETE CASCADE"
            } else {
                rel.on_delete.sql_clause()
            };
            let add = format!(
                "ALTER TABLE {table} ADD CONSTRAINT \"{constraint}\" FOREIGN KEY ({col}) REFERENCES {target_table} (\"id\"){on_delete}"
            );
            sqlx::query(&drop)
                .execute(pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            if let Err(e) = sqlx::query(&add).execute(pool).await {
                tracing::debug!(error = %e, table = %entity.table, "fk already present or skipped");
                let _ = sql;
            }
        }
    }
    Ok(())
}

async fn apply_junction_tables(pool: &PgPool, registry: &EntityRegistry) -> QefroResult<()> {
    for entity in registry.list() {
        if entity.skip_ddl {
            continue;
        }
        for field in &entity.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ManyToMany {
                continue;
            }
            let table_name = junction_table_name(&entity.table, &field.column_name());
            ident_check(&table_name)?;
            let table = quote_ident(&table_name)?;
            let ddl = format!(
                r#"CREATE TABLE IF NOT EXISTS {table} (
                    "tenant_id" UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                    "left_id" UUID NOT NULL,
                    "right_id" UUID NOT NULL,
                    PRIMARY KEY ("tenant_id", "left_id", "right_id")
                );
                CREATE INDEX IF NOT EXISTS {table_name}_right_idx ON {table} ("tenant_id", "right_id");"#
            );
            for stmt in split_sql(&ddl) {
                sqlx::query(stmt)
                    .execute(pool)
                    .await
                    .map_err(|e| QefroError::database(e.to_string()))?;
            }
        }
    }
    Ok(())
}

pub fn junction_table_name(left_table: &str, field: &str) -> String {
    format!("{left_table}_{field}")
}

fn split_sql(ddl: &str) -> Vec<&str> {
    ddl.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

pub(crate) fn ident_check(name: &str) -> QefroResult<()> {
    qefro_core::ident::assert_safe_ident(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qefro_core::{EntityDef, FieldDef};

    #[test]
    fn ddl_uses_quoted_idents_and_binds_no_user_data() {
        let def = EntityDef::new("Customer")
            .field(FieldDef::string("name").required())
            .field(FieldDef::string("email").unique().email())
            .build();
        let ddl = entity_ddl(&def).unwrap();
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS \"customers\""));
        assert!(ddl.contains("\"tenant_id\""));
        assert!(ddl.contains("\"email\""));
        assert!(!ddl.contains("DROP TABLE tenants"));
    }

    #[test]
    fn computed_columns_are_nullable() {
        let def = EntityDef::new("Invoice")
            .field(FieldDef::currency("subtotal").computed("SUM(items.amount)"))
            .build();
        let ddl = entity_ddl(&def).unwrap();
        assert!(ddl.contains("\"subtotal\" NUMERIC(18,6)"));
        assert!(!ddl.contains("\"subtotal\" NUMERIC(18,6) NOT NULL"));
    }
}
