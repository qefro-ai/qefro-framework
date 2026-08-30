//! Transactional outbox. Events are inserted in the same SQL transaction as
//! the business mutation, then published after COMMIT. Delivery is at-least-once.

use qefro_core::{QefroError, QefroResult};
use qefro_events::{DomainEvent, EventBus};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct Outbox {
    pool: PgPool,
}

impl Outbox {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn enqueue_tx(
        tx: &mut Transaction<'_, Postgres>,
        event: &DomainEvent,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            INSERT INTO qefro_outbox (
                id, tenant_id, event_name, entity, entity_id, user_id, payload, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(event.id)
        .bind(event.tenant_id)
        .bind(&event.name)
        .bind(&event.entity)
        .bind(event.entity_id)
        .bind(event.user_id)
        .bind(&event.payload)
        .bind(event.timestamp)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn enqueue_many_tx(
        tx: &mut Transaction<'_, Postgres>,
        events: &[DomainEvent],
    ) -> QefroResult<()> {
        for event in events {
            Self::enqueue_tx(tx, event).await?;
        }
        Ok(())
    }

    pub async fn dispatch_pending(&self, bus: &dyn EventBus, limit: i64) -> QefroResult<usize> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let rows: Vec<(
            Uuid,
            Uuid,
            String,
            String,
            Uuid,
            Option<Uuid>,
            Value,
            chrono::DateTime<chrono::Utc>,
        )> = sqlx::query_as(
            r#"
                SELECT id, tenant_id, event_name, entity, entity_id, user_id, payload, created_at
                FROM qefro_outbox
                WHERE published_at IS NULL
                ORDER BY created_at ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
                "#,
        )
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let mut n = 0;
        for (id, tenant_id, name, entity, entity_id, user_id, payload, created_at) in rows {
            let event = DomainEvent {
                id,
                name,
                entity,
                entity_id,
                tenant_id,
                timestamp: created_at,
                payload,
                user_id,
            };
            bus.publish(event).await?;
            sqlx::query("UPDATE qefro_outbox SET published_at = now() WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            n += 1;
        }
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(n)
    }

    pub async fn enqueue(&self, event: &DomainEvent) -> QefroResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Self::enqueue_tx(&mut tx, event).await?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn pending_count(&self) -> QefroResult<i64> {
        sqlx::query_scalar("SELECT COUNT(*) FROM qefro_outbox WHERE published_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))
    }
}
