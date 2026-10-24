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
    repository::{FileRepository, RepositoryError, SqlFileRepository},
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
                Some(started),
            )
            .await
            {
                Ok((true, _)) if started.elapsed() < RECONCILIATION_BUDGET => {}
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

async fn database_phase<T, F, Fut>(
    turn: &TenantTurn<'_>,
    timeout: Duration,
    phase: F,
) -> Result<T, ()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, ()>>,
{
    tokio::time::timeout(timeout, turn.run(phase))
        .await
        .map_err(|_| ())?
        .ok_or(())?
}

/// A bounded batch; only owned operation data crosses the provider-I/O boundary.
async fn reconcile_tenant_batch(
    access: &FileDatabase<'_>,
    platform: &Arc<FilePlatform>,
    tenant: &str,
    worker: &str,
    turn: &TenantTurn<'_>,
    started: Option<Instant>,
) -> Result<(bool, Instant), ()> {
    let config = platform.runtime_config();
    let phase_timeout = (config.reconciliation_lease / 4).min(Duration::from_secs(5));
    let (tenant_id, expired, started) = database_phase(turn, phase_timeout, || async {
        let started = started.unwrap_or_else(Instant::now);
        let (id, pool) = access.pool(tenant, None).await?;
        let repository = SqlFileRepository::new(pool.as_ref().clone());
        Ok::<_, ()>((
            id,
            request_expired_file_deletions(&repository).await,
            started,
        ))
    })
    .await?;
    let mut leased = 0;
    for _ in 0..config.reconciliation_batch_size {
        // Lease immediately before this item's external work, rather than aging
        // later items in a sequential batch while the first provider is slow.
        let operation = database_phase(turn, phase_timeout, || async {
            let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
            let repository = SqlFileRepository::new(pool.as_ref().clone());
            repository
                .lease_due_operations(worker, config.reconciliation_lease, 1)
                .await
                .map(|mut work| work.pop())
                .map_err(|_| ())
        })
        .await?;
        let Some(operation) = operation else {
            break;
        };
        leased += 1;
        let result = perform_leased_operation(platform, &operation).await;
        database_phase(turn, phase_timeout, || async {
            let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
            let repository = SqlFileRepository::new(pool.as_ref().clone());
            match repository.finish_leased_operation(&operation, result).await {
                Ok(()) | Err(RepositoryError::LeaseLost) => Ok(()),
                Err(error) => {
                    warn!(
                        error_code = error.log_safe_code(),
                        "File reconciliation completion fenced or deferred"
                    );
                    Err(())
                }
            }
        })
        .await?;
        // Finish the current external operation, then yield before another claim.
        if started.elapsed() >= RECONCILIATION_BUDGET {
            return Ok((true, started));
        }
    }
    database_phase(turn, phase_timeout, || async {
        let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
        purge_service::reconcile_pending_purges(&pool)
            .await
            .map_err(|_| ())?;
        Ok::<_, ()>(())
    })
    .await?;
    Ok((
        expired == EXPIRY_BATCH_SIZE as usize
            || leased == config.reconciliation_batch_size as usize,
        started,
    ))
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
        .map(|school| (school.subdomain, None, 0))
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
                    reconcile_tenant_batch(&access, platform, &tenant, worker, &turn, started)
                        .await;
                drop(turn);
                match result {
                    Ok((true, started))
                        if batches + 1 < MAX_RECONCILIATION_BATCHES
                            && started.elapsed() < RECONCILIATION_BUDGET =>
                    {
                        Some((tenant, Some(started), batches + 1))
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

#[cfg(test)]
mod phase_tests {
    use super::*;
    use async_trait::async_trait;
    use bytes::Bytes;
    use school_file_platform::{
        malware_scanner::{MalwareScanner, ScanOutcome},
        platform_types::DownloadGrant,
        runtime_config::FilePlatformRuntimeConfig,
        storage_provider::{ObjectMetadata, StorageError, StorageProvider, StoredObject},
    };
    use url::Url;
    struct Provider {
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
        block: bool,
    }
    #[async_trait]
    impl StorageProvider for Provider {
        async fn check_readiness(&self) -> Result<(), StorageError> {
            panic!("unexpected readiness")
        }
        async fn put(&self, _: &StoredObject, _: Bytes) -> Result<(), StorageError> {
            panic!("unexpected PUT")
        }
        async fn get(&self, _: &StoredObject, _: u64) -> Result<Bytes, StorageError> {
            panic!("unexpected GET")
        }
        async fn head(&self, _: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError> {
            panic!("unexpected HEAD")
        }
        async fn delete(&self, _: &StoredObject) -> Result<(), StorageError> {
            if self.block {
                self.started.notify_one();
                self.release.notified().await;
            }
            Ok(())
        }
        async fn private_download_grant(
            &self,
            _: &StoredObject,
            _: &str,
            _: Duration,
        ) -> Result<DownloadGrant, StorageError> {
            panic!("unexpected grant")
        }
        fn public_location(&self, _: &StoredObject) -> Result<Url, StorageError> {
            panic!("unexpected public location")
        }
    }
    struct Scanner;
    #[async_trait]
    impl MalwareScanner for Scanner {
        async fn scan(&self, _: &[u8]) -> ScanOutcome {
            panic!("unexpected scan")
        }
    }
    async fn fixture(name: &str, max_connections: u32) -> PgPool {
        let pool =
            school_test_db::create_named_test_pool_with_max_connections(name, max_connections)
                .await;
        school_test_db::run_test_migrations(&pool).await;
        let actor = Uuid::new_v4();
        let file = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) VALUES($1,'synthetic','Synthetic','Cleaner','staff','active')")
            .bind(actor).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO files(id,owner_user_id,purpose_code,display_filename,visibility,lifecycle_status,retention_class,created_by,expires_at,inspection_metadata) VALUES($1,$2,'attendance_evidence','fixture.jpg','private','ready','temporary',$2,now()-interval '1 minute','{\"kind\":\"image\",\"width_px\":640,\"height_px\":480}')")
            .bind(file).bind(actor).execute(&pool).await.unwrap();
        let version: Uuid = sqlx::query_scalar("INSERT INTO file_versions(file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status,created_by) VALUES($1,1,'r2','private','stored',$2,'image/jpeg','jpg',50000,repeat('a',64),'clean',$3) RETURNING id")
            .bind(file).bind(format!("tenants/{}/attendance/evidence/{file}/v1/original.jpg", Uuid::new_v4())).bind(actor).fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
            .bind(file)
            .bind(version)
            .execute(&pool)
            .await
            .unwrap();
        pool
    }
    fn platform(block: bool) -> (Arc<Provider>, Arc<FilePlatform>) {
        let provider = Arc::new(Provider {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            block,
        });
        let config = FilePlatformRuntimeConfig {
            reconciliation_lease: Duration::from_secs(4),
            reconciliation_batch_size: 1,
            ..FilePlatformRuntimeConfig::default()
        };
        let platform = Arc::new(FilePlatform::with_config(
            provider.clone(),
            Arc::new(Scanner),
            config,
        ));
        (provider, platform)
    }
    #[tokio::test]
    async fn slow_provider_item_allows_other_tenant_database_progress() {
        let slow = fixture("cleaner_slow_provider", 1).await;
        let healthy = fixture("cleaner_healthy_provider", 1).await;
        let turns = Arc::new(school_tenancy::background::TenantTurnExecutor::for_tests(1));
        let (provider, slow_platform) = platform(true);
        let (_, healthy_platform) = platform(false);
        let worker_turns = turns.clone();
        let worker_pool = slow.clone();
        let worker = tokio::spawn(async move {
            let turn = worker_turns
                .begin("slow", TenantJobKind::FileReconciliation)
                .unwrap();
            reconcile_tenant_batch(
                &FileDatabase::Unmanaged(&worker_pool),
                &slow_platform,
                "slow",
                "slow-worker",
                &turn,
                None,
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), provider.started.notified())
            .await
            .unwrap();
        assert_eq!(slow.options().get_max_connections(), 1);
        let mut free_connection = tokio::time::timeout(Duration::from_millis(200), slow.acquire())
            .await
            .unwrap()
            .unwrap();
        let probe: i32 = sqlx::query_scalar("SELECT 1")
            .fetch_one(&mut *free_connection)
            .await
            .unwrap();
        assert_eq!(
            probe, 1,
            "the slow tenant's sole physical connection must be released during provider I/O"
        );
        drop(free_connection);
        let healthy_turn = turns
            .begin("healthy", TenantJobKind::FileReconciliation)
            .unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            reconcile_tenant_batch(
                &FileDatabase::Unmanaged(&healthy),
                &healthy_platform,
                "healthy",
                "healthy-worker",
                &healthy_turn,
                None,
            ),
        )
        .await
        .unwrap();
        assert!(result.is_ok());
        let deleted: i64 =
            sqlx::query_scalar("SELECT count(*) FROM files WHERE lifecycle_status='deleted'")
                .fetch_one(&healthy)
                .await
                .unwrap();
        assert_eq!(deleted, 1);
        assert!(
            !worker.is_finished(),
            "healthy tenant must finish while slow external work remains blocked"
        );
        provider.release.notify_one();
        worker.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn blocked_database_phase_times_out_and_releases_shared_admission() {
        let pool = fixture("cleaner_blocked_database", 3).await;
        let turns = Arc::new(school_tenancy::background::TenantTurnExecutor::for_tests(1));
        let (_, platform) = platform(false);
        let mut blocker = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE files IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
        let worker_turns = turns.clone();
        let worker_pool = pool.clone();
        let worker = tokio::spawn(async move {
            let turn = worker_turns
                .begin("blocked", TenantJobKind::FileReconciliation)
                .unwrap();
            reconcile_tenant_batch(
                &FileDatabase::Unmanaged(&worker_pool),
                &platform,
                "blocked",
                "worker",
                &turn,
                None,
            )
            .await
        });
        let mut observer = pool.acquire().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                sqlx::query("SELECT pg_stat_clear_snapshot()").execute(&mut *observer).await.unwrap();
                let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))")
                    .bind(blocker_pid).fetch_one(&mut *observer).await.unwrap();
                if waiting { break; } tokio::task::yield_now().await;
            }
        }).await.unwrap();
        assert!(tokio::time::timeout(Duration::from_secs(2), worker)
            .await
            .unwrap()
            .unwrap()
            .is_err());
        assert_eq!(
            tokio::time::timeout(
                Duration::from_millis(200),
                turns.run("healthy", TenantJobKind::AuthRetention, || async { 1 })
            )
            .await
            .unwrap(),
            Some(1)
        );
        blocker.rollback().await.unwrap();
    }
}
