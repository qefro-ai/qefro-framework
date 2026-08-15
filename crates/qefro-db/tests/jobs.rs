use async_trait::async_trait;
use qefro_core::{OpContext, QefroError, QefroResult};
use qefro_db::{apply_schema, connect, JobHandler, JobQueue, JobRegistry};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

struct AlwaysFail;

#[async_trait]
impl JobHandler for AlwaysFail {
    async fn run(&self, _ctx: &OpContext, _payload: &Value) -> QefroResult<()> {
        Err(QefroError::internal("boom"))
    }
}

struct Succeed;

#[async_trait]
impl JobHandler for Succeed {
    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        assert!(!ctx.tenant_id.is_nil());
        assert_eq!(payload["entity"], "Reservation");
        Ok(())
    }
}

#[tokio::test]
async fn jobs_execute_retry_fail_and_preserve_tenant() {
    let Some(url) = db_url() else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let pool = connect(&url).await.unwrap();
    apply_schema(&pool, &qefro_core::EntityRegistry::new())
        .await
        .unwrap();

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug, created_at) VALUES ($1,$2,$3,now())")
        .bind(tenant_id)
        .bind("Jobs")
        .bind(format!("jobs-{}", &tenant_id.to_string()[..8]))
        .execute(&pool)
        .await
        .unwrap();

    let ctx = OpContext::new(tenant_id, user_id, vec!["System".into()]);
    let queue = JobQueue::new(pool.clone());
    let mut registry = JobRegistry::new();
    registry.register("succeed_job", std::sync::Arc::new(Succeed));
    registry.register("fail_job", std::sync::Arc::new(AlwaysFail));

    let ok_id = queue
        .enqueue(
            &ctx,
            "succeed_job",
            json!({ "entity": "Reservation", "entity_id": Uuid::new_v4() }),
        )
        .await
        .unwrap();
    process_until(&queue, &registry, &pool, tenant_id, ok_id, "succeeded").await;
    let ok = queue.get(tenant_id, ok_id).await.unwrap();
    assert_eq!(ok.status, "succeeded");
    assert_eq!(ok.tenant_id, tenant_id);

    let fail_id = queue
        .enqueue(&ctx, "fail_job", json!({ "entity": "Reservation" }))
        .await
        .unwrap();
    sqlx::query("UPDATE jobs SET max_attempts = 2, run_at = now() WHERE id = $1")
        .bind(fail_id)
        .execute(&pool)
        .await
        .unwrap();
    process_until(&queue, &registry, &pool, tenant_id, fail_id, "pending").await;
    let retrying = queue.get(tenant_id, fail_id).await.unwrap();
    assert_eq!(retrying.status, "pending");
    assert_eq!(retrying.attempts, 1);
    sqlx::query("UPDATE jobs SET run_at = now() WHERE id = $1")
        .bind(fail_id)
        .execute(&pool)
        .await
        .unwrap();
    process_until(&queue, &registry, &pool, tenant_id, fail_id, "failed").await;
    let failed = queue.get(tenant_id, fail_id).await.unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.attempts, 2);
    assert!(failed.last_error.as_deref().unwrap().contains("boom"));
    assert_eq!(failed.tenant_id, tenant_id);
}

async fn process_until(
    queue: &JobQueue,
    registry: &JobRegistry,
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    want: &str,
) {
    for _ in 0..30 {
        sqlx::query("UPDATE jobs SET run_at = now() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .ok();
        let _ = queue.process_one(registry).await.unwrap();
        let job = queue.get(tenant_id, id).await.unwrap();
        if job.status == want && (want != "pending" || job.attempts > 0) {
            return;
        }
        if job.status == "failed" || job.status == "succeeded" {
            assert_eq!(job.status, want, "job ended in {}", job.status);
            return;
        }
    }
    let job = queue.get(tenant_id, id).await.unwrap();
    panic!("job {} stuck in {} attempts={}", id, job.status, job.attempts);
}
