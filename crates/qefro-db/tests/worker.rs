use async_trait::async_trait;
use qefro_core::{OpContext, QefroError, QefroResult, ROLE_WORKER};
use qefro_db::{apply_schema, connect, JobHandler, JobQueue, JobRegistry};
use serde_json::{json, Value};
use uuid::Uuid;

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

struct SafeNotify;

#[async_trait]
impl JobHandler for SafeNotify {
    fn worker_safe(&self) -> bool {
        true
    }
    async fn run(&self, ctx: &OpContext, _payload: &Value) -> QefroResult<()> {
        assert!(ctx.is_worker());
        assert!(!ctx.is_admin());
        assert_eq!(ctx.roles, vec![ROLE_WORKER]);
        Ok(())
    }
}

struct UnsafeMutation;

#[async_trait]
impl JobHandler for UnsafeMutation {
    fn worker_safe(&self) -> bool {
        false
    }
    async fn run(&self, _ctx: &OpContext, _payload: &Value) -> QefroResult<()> {
        panic!("unsafe job handler must not run");
    }
}

#[tokio::test]
async fn worker_policy_allows_safe_and_rejects_unsafe() {
    let url = db_url();
    let pool = connect(&url).await.unwrap();
    apply_schema(&pool, &qefro_core::EntityRegistry::new())
        .await
        .unwrap();
    let tenant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug, created_at) VALUES ($1,$2,$3,now())")
        .bind(tenant_id)
        .bind("W")
        .bind(format!("w-{}", &tenant_id.to_string()[..8]))
        .execute(&pool)
        .await
        .unwrap();
    let ctx = OpContext::new(tenant_id, Uuid::new_v4(), vec!["Admin".into()]);
    let queue = JobQueue::new(pool.clone());
    let mut registry = JobRegistry::new();
    registry.register("send_notification", std::sync::Arc::new(SafeNotify));
    registry.register("confirm_reservation", std::sync::Arc::new(UnsafeMutation));

    let ok = queue
        .enqueue(
            &ctx,
            "send_notification",
            json!({ "entity": "Reservation" }),
        )
        .await
        .unwrap();
    process_until(&queue, &registry, &pool, tenant_id, ok, "succeeded").await;
    let rec = queue.get(tenant_id, ok).await.unwrap();
    assert_eq!(rec.status, "succeeded");
    assert_eq!(rec.tenant_id, tenant_id);

    let bad = queue
        .enqueue(&ctx, "confirm_reservation", json!({}))
        .await
        .unwrap();
    process_until(&queue, &registry, &pool, tenant_id, bad, "pending").await;
    let rec = queue.get(tenant_id, bad).await.unwrap();
    assert_eq!(rec.status, "pending");
    assert!(rec
        .last_error
        .as_deref()
        .unwrap_or("")
        .contains("worker-safe"));
}

#[test]
fn worker_context_is_not_admin() {
    let ctx = OpContext::worker(Uuid::new_v4(), Uuid::new_v4());
    assert!(ctx.is_worker());
    assert!(!ctx.is_admin());
    let err = QefroError::forbidden("workers cannot perform generic entity mutations");
    assert_eq!(err.error_code(), "forbidden");
}

#[tokio::test]
async fn worker_cannot_run_user_mutations_or_manager_ops() {
    let url = db_url();
    let mut registry = qefro_core::EntityRegistry::new();
    registry
        .register(
            qefro_core::EntityDef::new("WorkerNote")
                .table_name("worker_notes")
                .slug_name("worker-notes")
                .field(qefro_core::FieldDef::string("title").required())
                .build(),
        )
        .unwrap();
    let pool = connect(&url).await.unwrap();
    apply_schema(&pool, &registry).await.unwrap();
    let tenant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug, created_at) VALUES ($1,$2,$3,now())")
        .bind(tenant_id)
        .bind("W2")
        .bind(format!("w2-{}", &tenant_id.to_string()[..8]))
        .execute(&pool)
        .await
        .unwrap();

    let mut perms = qefro_permissions::PermissionRegistry::new();
    perms.grant(qefro_permissions::PermissionGrant::crud(
        "Admin",
        "WorkerNote",
    ));
    perms.grant(qefro_permissions::PermissionGrant::crud(
        "Manager",
        "WorkerNote",
    ));

    let mut operations = qefro_db::OperationRegistry::new();
    operations.register(
        qefro_core::OperationDef::new("generate_report", "WorkerNote").worker_safe(),
        std::sync::Arc::new(qefro_db::NoopOperationHandler),
    );
    operations.register(
        qefro_core::OperationDef::new("confirm", "WorkerNote").roles(&["Manager"]),
        std::sync::Arc::new(qefro_db::NoopOperationHandler),
    );

    let service = qefro_db::EntityService::new(
        pool,
        std::sync::Arc::new(registry),
        std::sync::Arc::new(perms),
        std::sync::Arc::new(qefro_workflow::WorkflowRegistry::new()),
        std::sync::Arc::new(qefro_core::HookRegistry::new()),
        qefro_events::InProcessEventBus::new(),
    )
    .with_operations(std::sync::Arc::new(operations));

    let admin = OpContext::new(tenant_id, Uuid::new_v4(), vec!["Admin".into()]);
    let created = service
        .create(&admin, "WorkerNote", json!({ "title": "n1" }))
        .await
        .unwrap();
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    let worker = OpContext::worker(tenant_id, admin.user_id);
    let create_err = service
        .create(&worker, "WorkerNote", json!({ "title": "nope" }))
        .await
        .unwrap_err();
    assert_eq!(create_err.error_code(), "forbidden");

    let manager_err = service
        .execute(&worker, "WorkerNote", id, "confirm", json!({}))
        .await
        .unwrap_err();
    assert_eq!(manager_err.error_code(), "forbidden");
    assert!(manager_err.to_string().contains("worker-safe"));

    let ok = service
        .execute(&worker, "WorkerNote", id, "generate_report", json!({}))
        .await
        .unwrap();
    assert_eq!(ok["id"], created["id"]);
}

async fn process_until(
    queue: &JobQueue,
    registry: &JobRegistry,
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    id: Uuid,
    want: &str,
) {
    for _ in 0..40 {
        sqlx::query("UPDATE jobs SET run_at = now() + interval '1 hour' WHERE status = 'pending' AND id <> $1")
            .bind(id)
            .execute(pool)
            .await
            .ok();
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
    panic!(
        "job {} stuck in {} attempts={}",
        id, job.status, job.attempts
    );
}
