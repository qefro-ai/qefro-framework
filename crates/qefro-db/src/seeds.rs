use qefro_core::{OpContext, QefroResult, SeedBatch};
use qefro_search::{Filter, Query};
use serde_json::Value;

use crate::service::EntityService;

/// Insert seed records. Existing rows matching `unique_by` are left untouched.
pub async fn apply_seed_batch(
    service: &EntityService,
    ctx: &OpContext,
    batch: &SeedBatch,
) -> QefroResult<u32> {
    let def = service.registry().get(&batch.entity)?;
    if batch.kind == "system" && def.tenant_owned {
        return Ok(0);
    }
    let mut created = 0u32;
    for record in &batch.records {
        if record_exists(service, ctx, batch, record).await? {
            continue;
        }
        service.create(ctx, &batch.entity, record.clone()).await?;
        created += 1;
    }
    Ok(created)
}

async fn record_exists(
    service: &EntityService,
    ctx: &OpContext,
    batch: &SeedBatch,
    record: &Value,
) -> QefroResult<bool> {
    if batch.unique_by.is_empty() {
        return Ok(false);
    }
    let mut filters = Vec::new();
    for field in &batch.unique_by {
        let Some(value) = record.get(field).cloned() else {
            return Ok(false);
        };
        filters.push(Filter::Eq {
            field: field.clone(),
            value,
        });
    }
    let page = service
        .list(
            ctx,
            &batch.entity,
            Query {
                filters,
                page_size: 1,
                ..Query::default()
            },
        )
        .await?;
    Ok(!page.items.is_empty())
}
