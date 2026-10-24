use futures::{stream::FuturesUnordered, StreamExt};
use school_tenancy::{
    background::{TenantJobKind, TenantTurn},
    AdminClient, PoolManager, TenantPool,
};
#[cfg(test)]
use sqlx::PgPool;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

const MAX_RECONCILIATION_BATCHES: usize = 100;
const RECONCILIATION_BUDGET: Duration = Duration::from_secs(120);
const EXPIRY_BATCH_SIZE: i64 = 50;
use uuid::Uuid;

use school_certificates::services::purge_service;
use school_file_platform::{
    platform_service::FilePlatform,
    reconciler::perform_leased_operation,
    repository::{FileRepository, SqlFileRepository},
};

/// Fixture adapter uses the production phase algorithm with an unmanaged database.
#[cfg(test)]
pub struct FileCleaner {
    pool: PgPool,
    platform: Arc<FilePlatform>,
}
#[cfg(test)]
impl FileCleaner {
    pub fn new(pool: PgPool, platform: Arc<FilePlatform>) -> Self {
        Self { pool, platform }
    }
    pub async fn reconcile_file_operations(&self) {
        let pools = PoolManager::new();
        let turns = pools.background_turns();
        let turn = turns
            .begin("fixture", TenantJobKind::FileReconciliation)
            .unwrap();
        let access = FileDatabase::Unmanaged(&self.pool);
        let started = Instant::now();
        for _ in 0..MAX_RECONCILIATION_BATCHES {
            match reconcile_tenant_batch(
                &access,
                &self.platform,
                "fixture",
                "fixture-worker",
                &turn,
            )
            .await
            {
                Ok(true) if started.elapsed() < RECONCILIATION_BUDGET => {}
                Ok(_) => break,
                Err(()) => panic!("file cleaner fixture failed"),
            }
        }
    }
}

async fn request_expired_file_deletions(repository: &SqlFileRepository) -> usize {
    let file_ids = match sqlx::query_scalar::<_, Uuid>(
        r#"
SELECT id
FROM files
WHERE retention_class = 'temporary'
  AND expires_at <= now()
  AND deleted_at IS NULL
  AND lifecycle_status NOT IN ('deleted', 'delete_requested')
ORDER BY expires_at, id
LIMIT $1
"#,
    )
    .bind(EXPIRY_BATCH_SIZE)
    .fetch_all(repository.pool())
    .await
    {
        Ok(file_ids) => file_ids,
        Err(_) => {
            warn!("Expired File Platform rows could not be listed");
            return 0;
        }
    };

    let selected = file_ids.len();
    for file_id in file_ids {
        match repository.request_delete(file_id).await {
            Ok(_) => info!("Expired file deletion durably queued"),
            Err(error) => warn!(
                error_code = error.log_safe_code(),
                "Expired file deletion request failed safely"
            ),
        }
    }
    selected
}

enum FileDatabase<'a> {
    Managed {
        admin: &'a AdminClient,
        pools: &'a PoolManager,
    },
    #[cfg(test)]
    Unmanaged(&'a PgPool),
}
impl FileDatabase<'_> {
    async fn pool(&self, tenant: &str, expected: Option<Uuid>) -> Result<(Uuid, TenantPool), ()> {
        match self {
            Self::Managed { admin, pools } => resolve_pool(admin, pools, tenant, expected).await,
            #[cfg(test)]
            Self::Unmanaged(pool) => Ok((Uuid::nil(), (*pool).clone().into())),
        }
    }
}

async fn resolve_pool(
    admin: &AdminClient,
    pools: &PoolManager,
    tenant: &str,
    expected: Option<Uuid>,
) -> Result<(Uuid, TenantPool), ()> {
    let school = admin
        .get_school_database_info(tenant)
        .await
        .map_err(|_| ())?;
    if expected.is_some_and(|id| id != school.tenant_id) {
        return Err(());
    }
    let pool = pools
        .get_pool_without_migrations(&school.database_url, tenant)
        .await
        .map_err(|_| ())?;
    Ok((school.tenant_id, pool))
}

/// A bounded batch; only owned operation data crosses the provider-I/O boundary.
async fn reconcile_tenant_batch(
    access: &FileDatabase<'_>,
    platform: &Arc<FilePlatform>,
    tenant: &str,
    worker: &str,
    turn: &TenantTurn<'_>,
) -> Result<bool, ()> {
    let (tenant_id, expired) = turn
        .run(|| async {
            let (id, pool) = access.pool(tenant, None).await?;
            let repository = SqlFileRepository::new(pool.as_ref().clone());
            Ok::<_, ()>((id, request_expired_file_deletions(&repository).await))
        })
        .await
        .ok_or(())??;
    let config = platform.runtime_config();
    let mut leased = 0;
    for _ in 0..config.reconciliation_batch_size {
        // Lease immediately before this item's external work, rather than aging
        // later items in a sequential batch while the first provider is slow.
        let operation = turn
            .run(|| async {
                let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
                let repository = SqlFileRepository::new(pool.as_ref().clone());
                repository
                    .lease_due_operations(worker, config.reconciliation_lease, 1)
                    .await
                    .map(|mut work| work.pop())
                    .map_err(|_| ())
            })
            .await
            .ok_or(())??;
        let Some(operation) = operation else {
            break;
        };
        leased += 1;
        let result = perform_leased_operation(platform, &operation).await;
        turn.run(|| async {
            let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
            let repository = SqlFileRepository::new(pool.as_ref().clone());
            match repository.finish_leased_operation(&operation, result).await {
                Ok(()) => Ok(()),
                Err(error) => {
                    warn!(
                        error_code = error.log_safe_code(),
                        "File reconciliation completion fenced or deferred"
                    );
                    Err(())
                }
            }
        })
        .await
        .ok_or(())??;
    }
    turn.run(|| async {
        let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
        purge_service::reconcile_pending_purges(&pool)
            .await
            .map_err(|_| ())?;
        Ok::<_, ()>(())
    })
    .await
    .ok_or(())??;
    Ok(
        expired == EXPIRY_BATCH_SIZE as usize
            || leased == config.reconciliation_batch_size as usize,
    )
}

pub async fn reconcile_all_tenants(
    admin: Arc<AdminClient>,
    pools: Arc<PoolManager>,
    platform: Arc<FilePlatform>,
) {
    let schools = match admin.list_active_schools().await {
        Ok(schools) => schools,
        Err(_) => {
            warn!(
                error_code = "file_directory_unavailable",
                "File reconciliation directory will retry"
            );
            return;
        }
    };
    let turns = pools.background_turns();
    let worker = format!("file-reconciler-{}", Uuid::new_v4());
    let mut pending: VecDeque<_> = schools
        .into_iter()
        .map(|school| (school.subdomain, Instant::now(), 0))
        .collect();
    let mut running = FuturesUnordered::new();
    loop {
        while running.len() < turns.limit() {
            let Some((tenant, started, batches)) = pending.pop_front() else {
                break;
            };
            let turns = &turns;
            let admin = &admin;
            let pools = &pools;
            let platform = &platform;
            let worker = &worker;
            running.push(async move {
                let turn = turns.begin(&tenant, TenantJobKind::FileReconciliation)?;
                let access = FileDatabase::Managed { admin, pools };
                let result =
                    reconcile_tenant_batch(&access, platform, &tenant, worker, &turn).await;
                drop(turn);
                match result {
                    Ok(true)
                        if batches + 1 < MAX_RECONCILIATION_BATCHES
                            && started.elapsed() < RECONCILIATION_BUDGET =>
                    {
                        Some((tenant, started, batches + 1))
                    }
                    Ok(_) => None,
                    Err(()) => {
                        warn!(
                            error_code = "file_reconciliation_deferred",
                            "File reconciliation tenant will retry"
                        );
                        None
                    }
                }
            });
        }
        let Some(remaining) = running.next().await else {
            break;
        };
        if let Some(remaining) = remaining {
            pending.push_back(remaining);
        }
    }
}
