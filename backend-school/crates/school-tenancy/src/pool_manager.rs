use crate::{tenant_pool::Lease, TenantPool};
use dashmap::DashMap;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, PgPool,
};
use std::{
    collections::HashMap,
    future::Future,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OwnedSemaphorePermit, RwLock, Semaphore};

#[derive(Clone, Debug)]
pub struct PoolManagerConfig {
    runtime_connections: u32,
    per_tenant: u32,
    admission_timeout: Duration,
}
impl PoolManagerConfig {
    pub(crate) fn background_turn_limit(&self, requested: usize) -> usize {
        let resident_slots = self.runtime_connections / self.per_tenant;
        requested.min(resident_slots.saturating_sub(1).max(1) as usize)
    }

    pub fn from_env() -> Result<Self, String> {
        let config = Self {
            runtime_connections: setting("TENANT_RUNTIME_CONNECTIONS", 40, 1, 10_000)?,
            per_tenant: setting("TENANT_POOL_MAX_CONNECTIONS", 10, 1, 100)?,
            admission_timeout: Duration::from_millis(setting(
                "TENANT_POOL_ADMISSION_TIMEOUT_MS",
                5000,
                10,
                30_000,
            )? as u64),
        };
        let server = setting("POSTGRES_MAX_CONNECTIONS", 64, 16, 100_000)?;
        let reserved = setting("ADMIN_POOL_MAX_CONNECTIONS", 5, 1, 1000)?
            + setting("PG_PROVISIONING_MAX_CONNECTIONS", 2, 1, 100)?
            + setting("SCHOOL_PRIVILEGED_CONNECTIONS", 5, 5, 100)?
            + setting("POSTGRES_CONNECTION_HEADROOM", 12, 1, 10_000)?;
        if config.runtime_connections + reserved > server {
            return Err("Aggregate connection allocations exceed POSTGRES_MAX_CONNECTIONS".into());
        }
        if config.per_tenant > config.runtime_connections {
            return Err("Tenant pool maximum exceeds aggregate runtime budget".into());
        }
        Ok(config)
    }
}
fn setting(name: &str, default: u32, min: u32, max: u32) -> Result<u32, String> {
    parse_setting(name, std::env::var(name).ok().as_deref(), default, min, max)
}
fn parse_setting(
    name: &str,
    raw: Option<&str>,
    default: u32,
    min: u32,
    max: u32,
) -> Result<u32, String> {
    let value = raw.map_or(Ok(default), |raw| {
        raw.parse::<u32>()
            .map_err(|_| format!("{name} must be an integer"))
    })?;
    if !(min..=max).contains(&value) {
        return Err(format!("{name} is outside the supported range"));
    }
    Ok(value)
}
impl Default for PoolManagerConfig {
    fn default() -> Self {
        Self {
            runtime_connections: 40,
            per_tenant: 10,
            admission_timeout: Duration::from_secs(5),
        }
    }
}

struct PoolEntry {
    pool: PgPool,
    last_used: Instant,
    lease: Weak<Lease>,
    reservation: Option<Arc<Reservation>>,
}
impl PoolEntry {
    fn is_fresh_at(&self, now: Instant, ttl: Duration) -> bool {
        now.saturating_duration_since(self.last_used) < ttl
    }
    fn touch(&mut self, now: Instant) {
        self.last_used = now;
    }
    fn leased(&mut self) -> Result<TenantPool, String> {
        let lease = self.lease.upgrade().unwrap_or_else(|| Arc::new(Lease));
        self.lease = Arc::downgrade(&lease);
        TenantPool::managed(self.pool.clone(), lease)
    }
}

impl Drop for PoolEntry {
    fn drop(&mut self) {
        if self.reservation.is_some() {
            drop(PendingPool {
                pool: Some(self.pool.clone()),
                reservation: self.reservation.take(),
            });
        }
    }
}

/// Cancellation-safe ownership: a removed pool keeps its entire reservation until drainage.
struct Reservation {
    _permit: OwnedSemaphorePermit,
}

struct PendingPool {
    pool: Option<PgPool>,
    reservation: Option<Arc<Reservation>>,
}
impl Drop for PendingPool {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            drop(pool.close());
            TenantPool::unregister(&pool);
            let reservation = self.reservation.take();
            tokio::spawn(async move {
                pool.close().await;
                drop(reservation);
            });
        }
    }
}

pub struct PoolManager {
    pools: Arc<RwLock<HashMap<String, PoolEntry>>>,
    creation_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
    admission: Arc<Mutex<()>>,
    budget: Arc<Semaphore>,
    max_connections_per_school: u32,
    admission_timeout: Duration,
    pool_ttl: Duration,
    lifecycle_timeout: Duration,
}
impl PoolManager {
    pub fn new() -> Self {
        Self::with_config(PoolManagerConfig::default())
    }
    pub fn with_config(config: PoolManagerConfig) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            creation_locks: Arc::new(DashMap::new()),
            admission: Arc::new(Mutex::new(())),
            budget: Arc::new(Semaphore::new(config.runtime_connections as usize)),
            max_connections_per_school: config.per_tenant,
            admission_timeout: config.admission_timeout,
            pool_ttl: Duration::from_secs(1800),
            lifecycle_timeout: Duration::from_secs(30),
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    pub async fn insert_test_pool(&self, database_url: &str, pool: PgPool) {
        self.pools.write().await.insert(
            database_url.to_string(),
            PoolEntry {
                pool,
                last_used: Instant::now(),
                lease: Weak::new(),
                reservation: None,
            },
        );
    }
    async fn cached_pool_at(&self, key: &str, now: Instant) -> Result<Option<TenantPool>, String> {
        let mut pools = self.pools.write().await;
        if let Some(entry) = pools.get_mut(key) {
            if !entry.pool.is_closed() {
                // A live logical operation protects the pool even after its cache TTL.
                if entry.is_fresh_at(now, self.pool_ttl) || entry.lease.strong_count() > 0 {
                    entry.touch(now);
                    return entry.leased().map(Some);
                }
            }
        }
        if let Some(mut entry) = pools.remove(key) {
            drop(PendingPool {
                pool: Some(entry.pool.clone()),
                reservation: entry.reservation.take(),
            });
        }
        Ok(None)
    }
    async fn reserve(&self) -> Result<OwnedSemaphorePermit, String> {
        let deadline = tokio::time::Instant::now() + self.admission_timeout;
        tokio::time::timeout_at(deadline, async {
            let _admission = self.admission.lock().await;
            loop {
                if let Ok(permit) = self
                    .budget
                    .clone()
                    .try_acquire_many_owned(self.max_connections_per_school)
                {
                    return Ok(permit);
                }
                let mut pools = self.pools.write().await;
                let victim = pools
                    .iter()
                    .filter(|(_, e)| {
                        e.lease.strong_count() == 0 && e.pool.size() as usize == e.pool.num_idle()
                    })
                    .min_by_key(|(_, e)| e.last_used)
                    .map(|(key, _)| key.clone());
                if let Some(key) = victim {
                    if let Some(mut entry) = pools.remove(&key) {
                        drop(PendingPool {
                            pool: Some(entry.pool.clone()),
                            reservation: entry.reservation.take(),
                        });
                    }
                }
                drop(pools);
                // Poll lease release as well as physical drainage; no request is replayed.
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| "Tenant connection budget admission timed out".to_string())?
    }
    async fn get_or_create_pool_with<F, Fut>(
        &self,
        database_url: &str,
        subdomain: &str,
        create_pool: F,
    ) -> Result<TenantPool, String>
    where
        F: FnOnce(Arc<Reservation>) -> Fut,
        Fut: Future<Output = Result<PgPool, String>>,
    {
        if let Some(pool) = self.cached_pool_at(database_url, Instant::now()).await? {
            return Ok(pool);
        }
        let started = Instant::now();
        let mut stage = "creation_gate";
        let result = tokio::time::timeout(self.lifecycle_timeout, async {
            let gate = self
                .creation_locks
                .entry(subdomain.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone();
            let _guard = gate.lock().await;
            if let Some(pool) = self.cached_pool_at(database_url, Instant::now()).await? {
                return Ok(pool);
            }
            stage = "connection_budget";
            let reservation = Arc::new(Reservation {
                _permit: self.reserve().await?,
            });
            // Factory has no established cached pool until it completes. Production uses a lazy
            // pool and its own guard so cancellation during initial schema I/O also drains safely.
            stage = "connect_and_verify";
            let pool = create_pool(reservation.clone()).await?;
            let mut pending = PendingPool {
                pool: Some(pool),
                reservation: Some(reservation),
            };
            let mut pools = self.pools.write().await;
            let pool = pending
                .pool
                .take()
                .ok_or("Tenant pool initialization unavailable")?;
            let mut entry = PoolEntry {
                pool,
                last_used: Instant::now(),
                lease: Weak::new(),
                reservation: pending.reservation.take(),
            };
            let leased = match entry.leased() {
                Ok(leased) => leased,
                Err(error) => {
                    drop(PendingPool {
                        pool: Some(entry.pool.clone()),
                        reservation: entry.reservation.take(),
                    });
                    return Err(error);
                }
            };
            pools.insert(database_url.to_owned(), entry);
            Ok(leased)
        })
        .await
        .map_err(|_| "Tenant database initialization timed out".to_string())
        .and_then(|result| result);
        tracing::info!(
            subdomain,
            operation = "tenant_pool_creation",
            stage,
            duration_ms = started.elapsed().as_secs_f64() * 1000.0,
            succeeded = result.is_ok()
        );
        if result.is_err() {
            tracing::warn!(
                subdomain,
                operation = "tenant_pool_creation",
                stage,
                error_code = "tenant_pool_creation_failed"
            );
        }
        result
    }
    pub async fn get_pool(
        &self,
        database_url: &str,
        subdomain: &str,
    ) -> Result<TenantPool, String> {
        self.get_pool_without_migrations(database_url, subdomain)
            .await
    }
    pub async fn get_pool_without_migrations(
        &self,
        database_url: &str,
        subdomain: &str,
    ) -> Result<TenantPool, String> {
        self.get_or_create_pool_with(database_url, subdomain, |reservation| async {
            let options = PgConnectOptions::from_str(database_url)
                .map_err(|_| format!("Invalid database configuration for {subdomain}"))?
                .statement_cache_capacity(100)
                .disable_statement_logging();
            let verified = Arc::new(AtomicBool::new(false));
            let tenant = subdomain.to_owned();
            let pool = PgPoolOptions::new()
                .after_connect(move |connection, _| {
                    let verified = verified.clone();
                    let tenant = tenant.clone();
                    Box::pin(async move {
                        if !verified.load(Ordering::Acquire) {
                            tracing::info!(
                                subdomain = tenant,
                                operation = "tenant_pool_creation",
                                stage = "schema_history_started"
                            );
                            let version = school_migrations::verify_tenant_schema(connection)
                                .await
                                .map_err(sqlx::Error::Protocol)?;
                            tracing::info!(
                                subdomain = tenant,
                                operation = "tenant_pool_creation",
                                stage = "schema_history_verified",
                                version
                            );
                            verified.store(true, Ordering::Release);
                        }
                        Ok(())
                    })
                })
                .min_connections(0)
                .max_connections(self.max_connections_per_school)
                .acquire_time_level(log::LevelFilter::Debug)
                .acquire_slow_level(log::LevelFilter::Warn)
                .acquire_slow_threshold(Duration::from_millis(250))
                .acquire_timeout(Duration::from_secs(20))
                .idle_timeout(Duration::from_secs(300))
                .test_before_acquire(true)
                .connect_lazy_with(options);
            let mut pending = PendingPool {
                pool: Some(pool.clone()),
                reservation: Some(reservation),
            };
            let mut connection = pool
                .acquire()
                .await
                .map_err(|_| format!("Failed to connect to database for {subdomain}"))?;
            connection.return_to_pool().await;
            pending.pool = None;
            Ok(pool)
        })
        .await
    }
    pub async fn get_pool_for_read_only_status(
        &self,
        database_url: &str,
        subdomain: &str,
    ) -> Result<TenantPool, String> {
        self.get_pool_without_migrations(database_url, subdomain)
            .await
    }
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        let mut pools = self.pools.write().await;
        let expired: Vec<_> = pools
            .iter()
            .filter(|(_, e)| e.lease.strong_count() == 0 && !e.is_fresh_at(now, self.pool_ttl))
            .map(|(k, _)| k.clone())
            .collect();
        for key in expired {
            if let Some(mut e) = pools.remove(&key) {
                drop(PendingPool {
                    pool: Some(e.pool.clone()),
                    reservation: e.reservation.take(),
                });
            }
        }
        self.creation_locks
            .retain(|_, lock| Arc::strong_count(lock) > 1);
    }
    pub async fn retire_pool(&self, database_url: &str, subdomain: &str) -> Result<(), String> {
        let gate = self
            .creation_locks
            .entry(subdomain.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let entry = tokio::time::timeout(self.lifecycle_timeout, async {
            let _guard = gate.lock().await;
            let entry = self.pools.write().await.remove(database_url);
            if let Some(e) = &entry {
                drop(e.pool.close());
                TenantPool::unregister(&e.pool);
            }
            entry
        })
        .await
        .map_err(|_| "Tenant pool retirement timed out".to_string())?;
        if let Some(mut entry) = entry {
            let pending = PendingPool {
                pool: Some(entry.pool.clone()),
                reservation: entry.reservation.take(),
            };
            let result = close_pool_with_timeout(
                &entry.pool,
                "tenant_pool_retirement",
                self.lifecycle_timeout,
            )
            .await;
            drop(pending);
            result?;
        }
        Ok(())
    }
    pub async fn pool_count(&self) -> usize {
        self.pools.read().await.len()
    }
}
pub async fn close_pool(pool: &PgPool, operation: &'static str) -> Result<(), String> {
    close_pool_with_timeout(pool, operation, Duration::from_secs(30)).await
}
async fn close_pool_with_timeout(
    pool: &PgPool,
    operation: &'static str,
    timeout: Duration,
) -> Result<(), String> {
    tokio::time::timeout(timeout, pool.close())
        .await
        .map_err(|_| {
            tracing::warn!(
                operation,
                error_code = "database_pool_drain_timeout",
                pool_connections = pool.size(),
                pool_idle_connections = pool.num_idle(),
                "Database pool has outstanding borrowers"
            );
            "Database pool drainage timed out".into()
        })
}
impl Default for PoolManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn background_turn_admission_clamps_to_resident_pool_capacity() {
        use super::PoolManagerConfig;
        let one = PoolManagerConfig {
            runtime_connections: 10,
            per_tenant: 10,
            ..PoolManagerConfig::default()
        };
        assert_eq!(one.background_turn_limit(64), 1);
        let two = PoolManagerConfig {
            runtime_connections: 20,
            ..one
        };
        assert_eq!(two.background_turn_limit(64), 1);
        let four = PoolManagerConfig::default();
        assert_eq!(four.background_turn_limit(64), 3);
        assert_eq!(four.background_turn_limit(2), 2);
    }

    #[test]
    fn privileged_capacity_reserves_provisioning_transaction_and_followup_reads() {
        for raw in ["0", "1", "2", "3", "4", "101", "bad"] {
            assert!(
                super::parse_setting("SCHOOL_PRIVILEGED_CONNECTIONS", Some(raw), 5, 5, 100)
                    .is_err()
            );
        }
        assert_eq!(
            super::parse_setting("SCHOOL_PRIVILEGED_CONNECTIONS", Some("5"), 5, 5, 100).unwrap(),
            5
        );
        assert_eq!(
            super::parse_setting("SCHOOL_PRIVILEGED_CONNECTIONS", None, 5, 5, 100).unwrap(),
            5
        );
    }

    #[tokio::test]
    async fn raw_privileged_verification_cancellation_closes_socket_before_unlock() {
        use tokio::io::AsyncWriteExt;
        let fixture = school_test_db::create_named_test_pool("raw_privileged_cancel").await;
        school_test_db::run_test_migrations(&fixture).await;
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&fixture)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        let upstream = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap_or(5432));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        url.set_host(Some("127.0.0.1")).unwrap();
        url.set_port(Some(listener.local_addr().unwrap().port()))
            .unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
        let proxy = tokio::spawn(async move {
            let (client, _) = listener.accept().await.unwrap();
            let server = tokio::net::TcpStream::connect(upstream).await.unwrap();
            let (mut client_read, mut client_write) = client.into_split();
            let (mut server_read, mut server_write) = server.into_split();
            let forward = async {
                let _ = tokio::io::copy(&mut client_read, &mut server_write).await;
                let _ = closed_tx.send(());
                let _ = server_write.shutdown().await;
            };
            let reverse = async {
                let _ = tokio::io::copy(&mut server_read, &mut client_write).await;
            };
            tokio::join!(forward, reverse);
        });
        let mut lock = fixture.begin().await.unwrap();
        sqlx::query("LOCK TABLE _sqlx_migrations IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        let result = tokio::time::timeout(Duration::from_millis(150), async {
            let mut connection = super::connect_privileged_connection(url.as_str())
                .await
                .unwrap();
            school_migrations::verify_tenant_schema(&mut *connection).await
        })
        .await;
        assert!(result.is_err());
        let closed = tokio::time::timeout(Duration::from_millis(300), closed_rx).await;
        lock.rollback().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), proxy)
            .await
            .unwrap()
            .unwrap();
        assert!(closed.is_ok(),"raw verification cancellation must close its socket before the blocked database query completes");
    }

    fn small_manager() -> super::PoolManager {
        super::PoolManager::with_config(super::PoolManagerConfig {
            runtime_connections: 2,
            per_tenant: 1,
            admission_timeout: Duration::from_millis(100),
        })
    }

    #[tokio::test]
    async fn logical_leases_and_detached_tasks_prevent_eviction_between_queries() {
        let manager = small_manager();
        let first = manager
            .get_or_create_pool_with("first", "first", |_| async { Ok(lazy_pool("first")) })
            .await
            .unwrap();
        let retention = crate::TenantPool::retain_for_pool(&first).unwrap().unwrap();
        let second = manager
            .get_or_create_pool_with("second", "second", |_| async { Ok(lazy_pool("second")) })
            .await
            .unwrap();
        drop(first);
        assert!(manager
            .get_or_create_pool_with("third", "third", |_| async { Ok(lazy_pool("third")) })
            .await
            .is_err());
        assert!(!second.is_closed());
        drop(retention);
        let third = manager
            .get_or_create_pool_with("third", "third", |_| async { Ok(lazy_pool("third")) })
            .await
            .unwrap();
        assert!(!third.is_closed());
        assert!(!second.is_closed());
        assert_eq!(manager.pool_count().await, 2);
    }

    #[tokio::test]
    async fn background_turn_keeps_managed_lease_between_queries_until_cancellation() {
        use crate::background::{TenantJobKind, TenantTurnExecutor};
        let fixture = school_test_db::create_named_test_pool("turn_managed_lease").await;
        school_test_db::run_test_migrations(&fixture).await;
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&fixture)
            .await
            .unwrap();
        let mut base = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        base.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let tenant_url = |name: &str| {
            let mut url = base.clone();
            url.query_pairs_mut().append_pair("application_name", name);
            url
        };
        let first_url = tenant_url("turn-lease-one");
        let second_url = tenant_url("turn-lease-two");
        let third_url = tenant_url("turn-lease-three");
        let manager = Arc::new(small_manager());
        let turns = Arc::new(TenantTurnExecutor::for_tests(2));
        let worker_manager = manager.clone();
        let worker_turns = turns.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let turn = tokio::spawn(async move {
            worker_turns
                .run("one", TenantJobKind::AuthRetention, || async {
                    let pool = worker_manager
                        .get_pool(first_url.as_str(), "one")
                        .await
                        .unwrap();
                    assert_eq!(
                        sqlx::query_scalar::<_, i32>("SELECT 1")
                            .fetch_one(&*pool)
                            .await
                            .unwrap(),
                        1
                    );
                    started_tx.send(()).unwrap();
                    // No physical query is borrowed here, but the entire domain
                    // turn still owns its logical lease across this await.
                    std::future::pending::<()>().await;
                    sqlx::query_scalar::<_, i32>("SELECT 2")
                        .fetch_one(&*pool)
                        .await
                        .unwrap()
                })
                .await
        });
        started_rx.await.unwrap();
        let foreground = manager.get_pool(second_url.as_str(), "two").await.unwrap();
        assert!(
            manager.get_pool(third_url.as_str(), "three").await.is_err(),
            "a turn between queries must remain ineligible for eviction"
        );
        assert!(!foreground.is_closed());
        turn.abort();
        assert!(turn.await.unwrap_err().is_cancelled());
        let admitted = manager.get_pool(third_url.as_str(), "three").await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT 3")
                .fetch_one(&*admitted)
                .await
                .unwrap(),
            3
        );
        assert!(!foreground.is_closed());
        assert_eq!(
            turns
                .run("one", TenantJobKind::AuthRetention, || async { 4 })
                .await,
            Some(4)
        );
    }

    #[tokio::test]
    async fn cancelled_budget_waiter_recovers_without_losing_capacity() {
        let manager = Arc::new(small_manager());
        let first = manager
            .get_or_create_pool_with("one", "one", |_| async { Ok(lazy_pool("one")) })
            .await
            .unwrap();
        let second = manager
            .get_or_create_pool_with("two", "two", |_| async { Ok(lazy_pool("two")) })
            .await
            .unwrap();
        let waiting = manager.clone();
        let task = tokio::spawn(async move {
            waiting
                .get_or_create_pool_with("three", "three", |_| async { Ok(lazy_pool("three")) })
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        task.abort();
        let _ = task.await;
        drop(first);
        drop(second);
        assert!(manager
            .get_or_create_pool_with("three", "three", |_| async { Ok(lazy_pool("three")) })
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn aggregate_physical_connections_include_background_borrowers_and_draining_pools() {
        let fixture = school_test_db::create_named_test_pool("aggregate_budget").await;
        school_test_db::run_test_migrations(&fixture).await;
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&fixture)
            .await
            .unwrap();
        let base = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        let manager = small_manager();
        let mut first_url = base.clone();
        first_url
            .query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"))
            .append_pair("application_name", "budget-one");
        let mut second_url = base.clone();
        second_url
            .query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"))
            .append_pair("application_name", "budget-two");
        let mut third_url = base.clone();
        third_url
            .query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"))
            .append_pair("application_name", "budget-three");
        let first = manager.get_pool(first_url.as_str(), "one").await.unwrap();
        let second = manager.get_pool(second_url.as_str(), "two").await.unwrap();
        let held = first.acquire().await.unwrap();
        let retiring = manager.retire_pool(first_url.as_str(), "one");
        tokio::pin!(retiring);
        tokio::select! { _=&mut retiring=>panic!("borrower must retain retirement"), _=tokio::time::sleep(Duration::from_millis(20))=>{} }
        drop(first);
        assert!(manager.get_pool(third_url.as_str(), "three").await.is_err());
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE application_name IN ('budget-one','budget-two','budget-three')").fetch_one(&fixture).await.unwrap();
        assert_eq!(count, 2);
        drop(held);
        retiring.await.unwrap();
        let third = manager.get_pool(third_url.as_str(), "three").await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(&*third)
                .await
                .unwrap(),
            1
        );
        assert!(!second.is_closed());
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE application_name IN ('budget-one','budget-two','budget-three')").fetch_one(&fixture).await.unwrap();
        assert!(count <= 2);
    }

    #[tokio::test]
    async fn deployment_retirement_closes_old_clones_and_reopens_a_verified_pool() {
        let fixture = school_test_db::create_named_test_pool("runtime_pool_retirement").await;
        school_test_db::run_test_migrations(&fixture).await;
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&fixture)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let manager = super::PoolManager::new();
        let old = manager.get_pool(url.as_str(), "tenant").await.unwrap();
        assert_eq!(old.size(), 1);
        assert_eq!(
            old.num_idle(),
            1,
            "verified connection must be ready for the first request"
        );
        let held_clone = old.clone();
        manager.retire_pool(url.as_str(), "tenant").await.unwrap();
        assert_eq!(manager.pool_count().await, 0);
        assert!(old.is_closed());
        assert!(held_clone.is_closed());
        let fresh = manager.get_pool(url.as_str(), "tenant").await.unwrap();
        assert!(!fresh.is_closed());
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(&*fresh)
                .await
                .unwrap(),
            1
        );
        assert_eq!(manager.pool_count().await, 1);
        manager.retire_pool(url.as_str(), "tenant").await.unwrap();
    }

    #[tokio::test]
    async fn stalled_schema_query_times_out_and_recovers_after_database_unblocks() {
        let fixture = school_test_db::create_named_test_pool("blocked_schema").await;
        school_test_db::run_test_migrations(&fixture).await;
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&fixture)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let mut lock = fixture.begin().await.unwrap();
        sqlx::query("LOCK TABLE _sqlx_migrations IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        let mut manager = super::PoolManager::new();
        manager.lifecycle_timeout = Duration::from_millis(100);
        let failed = tokio::time::timeout(
            Duration::from_secs(1),
            manager.get_pool(url.as_str(), "blocked"),
        )
        .await
        .expect("schema verification must not hold the tenant gate indefinitely");
        assert!(failed.is_err());
        assert_eq!(manager.pool_count().await, 0);
        lock.rollback().await.unwrap();
        manager.lifecycle_timeout = Duration::from_secs(5);
        let recovered = manager.get_pool(url.as_str(), "blocked").await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(&*recovered)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn cancelled_schema_read_closes_socket_before_database_unblocks() {
        use tokio::io::AsyncWriteExt;
        let fixture = school_test_db::create_named_test_pool("schema_socket_cancellation").await;
        school_test_db::run_test_migrations(&fixture).await;
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&fixture)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        let upstream = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap_or(5432));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        url.set_host(Some("127.0.0.1")).unwrap();
        url.set_port(Some(listener.local_addr().unwrap().port()))
            .unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
        let proxy = tokio::spawn(async move {
            let (client, _) = listener.accept().await.unwrap();
            let server = tokio::net::TcpStream::connect(upstream).await.unwrap();
            let (mut client_read, mut client_write) = client.into_split();
            let (mut server_read, mut server_write) = server.into_split();
            let forward = async {
                tokio::io::copy(&mut client_read, &mut server_write)
                    .await
                    .unwrap();
                // Observe client EOF even while PostgreSQL is still blocked on its lock.
                let _ = closed_tx.send(());
                server_write.shutdown().await.unwrap();
            };
            let reverse = async {
                // A reset here is expected when a cancelled client closes its socket.
                let _ = tokio::io::copy(&mut server_read, &mut client_write).await;
            };
            tokio::join!(forward, reverse);
        });
        let mut lock = fixture.begin().await.unwrap();
        sqlx::query("LOCK TABLE _sqlx_migrations IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        let mut manager = super::PoolManager::new();
        manager.lifecycle_timeout = Duration::from_millis(150);
        assert!(manager
            .get_pool(url.as_str(), "cancel_socket")
            .await
            .is_err());
        let closed_before_unlock =
            tokio::time::timeout(Duration::from_millis(300), closed_rx).await;
        lock.rollback().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), proxy)
            .await
            .unwrap()
            .unwrap();
        assert!(
            closed_before_unlock.is_ok(),
            "cancelled initialization leaked a socket into a background pool-return task"
        );
    }

    #[tokio::test]
    async fn silent_tcp_peer_times_out_then_recovers_without_restarting_manager() {
        let fixture = school_test_db::create_named_test_pool("pool_transport_recovery").await;
        school_test_db::run_test_migrations(&fixture).await;
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&fixture)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        let upstream = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap_or(5432));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        url.set_host(Some("127.0.0.1")).unwrap();
        url.set_port(Some(listener.local_addr().unwrap().port()))
            .unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        let proxy = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let (mut silent, _) = listener.accept().await.unwrap();
            accepted_tx.send(()).unwrap();
            // Accept the TCP connection but never answer PostgreSQL startup.
            // Cancellation must close this socket, not leave an initialization task behind.
            let mut discarded = Vec::new();
            silent.read_to_end(&mut discarded).await.unwrap();
            let mut bridges = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    accepted = listener.accept() => {
                        let (mut client, _) = accepted.unwrap();
                        let upstream = upstream.clone();
                        bridges.spawn(async move {
                            let mut server = tokio::net::TcpStream::connect(upstream).await.unwrap();
                            tokio::io::copy_bidirectional(&mut client, &mut server).await.unwrap();
                        });
                    }
                }
            }
            while let Some(result) = bridges.join_next().await {
                result.unwrap();
            }
        });
        let mut manager = super::PoolManager::new();
        manager.lifecycle_timeout = Duration::from_millis(100);
        let (failed, accepted) =
            tokio::join!(manager.get_pool(url.as_str(), "transport"), accepted_rx);
        accepted.unwrap();
        assert!(failed.is_err());
        assert_eq!(manager.pool_count().await, 0);
        manager.lifecycle_timeout = Duration::from_secs(5);
        let recovered = manager.get_pool(url.as_str(), "transport").await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(&*recovered)
                .await
                .unwrap(),
            1
        );
        manager
            .retire_pool(url.as_str(), "transport")
            .await
            .unwrap();
        stop_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), proxy)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn retirement_does_not_wait_indefinitely_for_a_checked_out_connection() {
        let fixture = school_test_db::create_named_test_pool("blocked_retirement").await;
        let held = fixture.acquire().await.unwrap();
        let mut manager = super::PoolManager::new();
        manager.lifecycle_timeout = Duration::from_millis(40);
        manager
            .insert_test_pool("postgres://retirement", fixture.clone())
            .await;
        let manager = Arc::new(manager);
        let retire_manager = Arc::clone(&manager);
        let retiring = tokio::spawn(async move {
            retire_manager
                .retire_pool("postgres://retirement", "tenant")
                .await
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while !fixture.is_closed() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // A new request must not wait behind the old borrower's drainage.
        let fresh = tokio::time::timeout(
            Duration::from_millis(500),
            manager.get_or_create_pool_with(
                "postgres://retirement",
                "tenant",
                |_reservation| async { Ok(lazy_pool("after_retirement")) },
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!fresh.is_closed());
        let result = tokio::time::timeout(Duration::from_millis(500), retiring)
            .await
            .expect("retirement must report failure instead of blocking deployment")
            .unwrap();
        assert!(result.is_err());
        assert_eq!(manager.pool_count().await, 1);
        drop(held);
    }

    #[tokio::test]
    async fn runtime_pool_access_does_not_reconcile_permissions() {
        let pool = school_test_db::create_named_test_pool("pool_no_runtime_ddl").await;
        school_test_db::run_test_migrations(&pool).await;
        sqlx::raw_sql(r#"CREATE FUNCTION reject_runtime_sync() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN RAISE EXCEPTION 'runtime must not reconcile permissions'; END $$;
            CREATE TRIGGER reject_runtime_sync BEFORE INSERT ON permissions FOR EACH STATEMENT EXECUTE FUNCTION reject_runtime_sync();"#)
            .execute(&pool).await.unwrap();
        let manager = super::PoolManager::new();
        manager.insert_test_pool("test-only-runtime", pool).await;
        for _ in 0..4 {
            manager
                .get_pool("test-only-runtime", "tenant")
                .await
                .unwrap();
        }
    }

    use super::{PoolEntry, PoolManager};
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Weak,
    };
    use std::time::{Duration, Instant};
    use tokio::sync::Barrier;

    fn lazy_pool(database: &str) -> sqlx::PgPool {
        PgPoolOptions::new().connect_lazy_with(
            PgConnectOptions::new()
                .host("127.0.0.1")
                .username("postgres")
                .database(database),
        )
    }

    #[tokio::test]
    async fn stalled_creation_releases_the_tenant_gate_and_allows_retry() {
        let mut manager = PoolManager::new();
        manager.lifecycle_timeout = Duration::from_millis(40);
        let stalled = tokio::time::timeout(
            Duration::from_millis(500),
            manager.get_or_create_pool_with(
                "postgres://stalled",
                "stalled",
                |_reservation| async {
                    std::future::pending::<Result<sqlx::PgPool, String>>().await
                },
            ),
        )
        .await
        .expect("pool creation must have its own deadline");
        assert!(stalled.is_err());
        assert_eq!(manager.pool_count().await, 0);
        let recovered = manager
            .get_or_create_pool_with("postgres://stalled", "stalled", |_reservation| async {
                Ok(lazy_pool("recovered"))
            })
            .await;
        assert!(recovered.is_ok());
    }

    #[tokio::test]
    async fn cancelled_creator_releases_gate_for_waiting_request() {
        let manager = Arc::new(PoolManager::new());
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let creator_manager = Arc::clone(&manager);
        let creator = tokio::spawn(async move {
            creator_manager
                .get_or_create_pool_with("postgres://cancel", "cancel", |_reservation| async {
                    entered_tx.send(()).unwrap();
                    std::future::pending::<Result<sqlx::PgPool, String>>().await
                })
                .await
        });
        entered_rx.await.unwrap();
        creator.abort();
        assert!(creator.await.unwrap_err().is_cancelled());
        let retry = tokio::time::timeout(
            Duration::from_secs(1),
            manager.get_or_create_pool_with("postgres://cancel", "cancel", |_reservation| async {
                Ok(lazy_pool("retry_after_cancellation"))
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!retry.is_closed());
    }

    #[tokio::test]
    async fn closed_cached_pool_is_replaced() {
        let manager = PoolManager::new();
        let old = lazy_pool("closed");
        manager
            .insert_test_pool("postgres://closed", old.clone())
            .await;
        old.close().await;
        let pool = manager
            .get_or_create_pool_with("postgres://closed", "closed", |_reservation| async {
                Ok(lazy_pool("replacement"))
            })
            .await
            .unwrap();
        assert!(
            !pool.is_closed(),
            "closed handles must never be returned from the cache"
        );
    }

    #[tokio::test]
    async fn cache_entry_freshness_uses_the_latest_touch() {
        let started = Instant::now();
        let mut entry = PoolEntry {
            pool: lazy_pool("touch_test"),
            last_used: started,
            lease: Weak::new(),
            reservation: None,
        };
        let ttl = Duration::from_secs(30);

        assert!(entry.is_fresh_at(started + Duration::from_secs(29), ttl));
        entry.touch(started + Duration::from_secs(20));
        assert!(entry.is_fresh_at(started + Duration::from_secs(49), ttl));
        assert!(!entry.is_fresh_at(started + Duration::from_secs(50), ttl));
    }

    #[tokio::test]
    async fn cache_hit_updates_the_stored_last_used_time() {
        let manager = PoolManager::new();
        let database_url = "postgres://touch-cache";
        let started = Instant::now();
        let touched_at = started + Duration::from_secs(20);
        manager.pools.write().await.insert(
            database_url.to_string(),
            PoolEntry {
                pool: lazy_pool("touch_cache"),
                last_used: started,
                lease: Weak::new(),
                reservation: None,
            },
        );

        assert!(manager
            .cached_pool_at(database_url, touched_at)
            .await
            .unwrap()
            .is_some());
        assert_eq!(
            manager
                .pools
                .read()
                .await
                .get(database_url)
                .expect("cache entry must remain")
                .last_used,
            touched_at
        );
    }

    #[tokio::test]
    async fn expired_cache_entry_is_recreated() {
        let manager = PoolManager::new();
        let database_url = "postgres://expired";
        manager.pools.write().await.insert(
            database_url.to_string(),
            PoolEntry {
                pool: lazy_pool("expired_old"),
                last_used: Instant::now() - manager.pool_ttl - Duration::from_secs(1),
                lease: Weak::new(),
                reservation: None,
            },
        );
        let creations = AtomicUsize::new(0);

        let result = manager
            .get_or_create_pool_with(database_url, "expired", |_reservation| async {
                creations.fetch_add(1, Ordering::SeqCst);
                Ok(lazy_pool("expired_new"))
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(creations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn concurrent_misses_for_one_tenant_create_one_pool() {
        let manager = Arc::new(PoolManager::new());
        let creations = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for _ in 0..20 {
            let manager = Arc::clone(&manager);
            let creations = Arc::clone(&creations);
            tasks.push(tokio::spawn(async move {
                manager
                    .get_or_create_pool_with(
                        "postgres://tenant-one",
                        "tenant-one",
                        |_reservation| async move {
                            creations.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            Ok(lazy_pool("tenant_one"))
                        },
                    )
                    .await
            }));
        }

        for task in tasks {
            assert!(task.await.expect("pool task must join").is_ok());
        }
        assert_eq!(creations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn different_tenants_create_pools_concurrently() {
        let manager = Arc::new(PoolManager::new());
        let entered = Arc::new(Barrier::new(2));

        let make_task = |subdomain: &'static str, database_url: &'static str| {
            let manager = Arc::clone(&manager);
            let entered = Arc::clone(&entered);
            tokio::spawn(async move {
                manager
                    .get_or_create_pool_with(database_url, subdomain, |_reservation| async move {
                        entered.wait().await;
                        Ok(lazy_pool(subdomain))
                    })
                    .await
            })
        };

        let both = async {
            let first = make_task("tenant-a", "postgres://tenant-a");
            let second = make_task("tenant-b", "postgres://tenant-b");
            assert!(first.await.expect("first task must join").is_ok());
            assert!(second.await.expect("second task must join").is_ok());
        };

        tokio::time::timeout(Duration::from_millis(250), both)
            .await
            .expect("different tenants must not share a global creation lock");
    }

    #[tokio::test]
    async fn failed_creation_is_not_cached() {
        let manager = PoolManager::new();
        let creations = AtomicUsize::new(0);

        let first = manager
            .get_or_create_pool_with("postgres://retry", "retry", |_reservation| async {
                creations.fetch_add(1, Ordering::SeqCst);
                Err("first creation failed".to_string())
            })
            .await;
        assert_eq!(
            first.expect_err("first creation must fail"),
            "first creation failed"
        );

        let second = manager
            .get_or_create_pool_with("postgres://retry", "retry", |_reservation| async {
                creations.fetch_add(1, Ordering::SeqCst);
                Ok(lazy_pool("retry"))
            })
            .await;
        assert!(second.is_ok());
        assert_eq!(creations.load(Ordering::SeqCst), 2);
    }
}

static PRIVILEGED_BUDGET: std::sync::OnceLock<Result<Arc<Semaphore>, String>> =
    std::sync::OnceLock::new();
fn privileged_budget() -> Result<Arc<Semaphore>, String> {
    PRIVILEGED_BUDGET
        .get_or_init(|| {
            setting("SCHOOL_PRIVILEGED_CONNECTIONS", 5, 5, 100)
                .map(|count| Arc::new(Semaphore::new(count as usize)))
        })
        .clone()
}

/// Owner/provisioning pools never enter the tenant cache and share a separate physical reserve.
pub struct PrivilegedPool {
    pool: PgPool,
    _drain: PendingPool,
}
impl std::ops::Deref for PrivilegedPool {
    type Target = PgPool;
    fn deref(&self) -> &PgPool {
        &self.pool
    }
}
impl AsRef<PgPool> for PrivilegedPool {
    fn as_ref(&self) -> &PgPool {
        &self.pool
    }
}
pub async fn connect_privileged_pool(
    database_url: &str,
    maximum: u32,
) -> Result<PrivilegedPool, String> {
    let capacity = setting("SCHOOL_PRIVILEGED_CONNECTIONS", 5, 5, 100)?;
    let maximum = maximum.min(capacity).max(1);
    let permit = tokio::time::timeout(
        Duration::from_secs(5),
        privileged_budget()?.acquire_many_owned(maximum),
    )
    .await
    .map_err(|_| "Privileged database connection budget timed out")?
    .map_err(|_| "Privileged database connection budget closed")?;
    let options = PgConnectOptions::from_str(database_url)
        .map_err(|_| "Invalid privileged database configuration")?
        .disable_statement_logging();
    let pool = PgPoolOptions::new()
        .min_connections(0)
        .max_connections(maximum)
        .acquire_timeout(Duration::from_secs(30))
        .connect_lazy_with(options);
    let drain = PendingPool {
        pool: Some(pool.clone()),
        reservation: Some(Arc::new(Reservation { _permit: permit })),
    };
    let mut connection = pool
        .acquire()
        .await
        .map_err(|_| "Privileged database connection failed")?;
    connection.return_to_pool().await;
    Ok(PrivilegedPool {
        pool,
        _drain: drain,
    })
}

/// A directly owned verification socket. Field order closes it before releasing its slot.
pub struct PrivilegedConnection {
    connection: sqlx::PgConnection,
    reservation: OwnedSemaphorePermit,
}
impl std::ops::Deref for PrivilegedConnection {
    type Target = sqlx::PgConnection;
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}
impl std::ops::DerefMut for PrivilegedConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}
impl PrivilegedConnection {
    pub async fn close(self) -> Result<(), String> {
        use sqlx::Connection;
        // The close future owns the raw socket and drops before the earlier permit binding
        // if this future is cancelled. No SQLx PoolConnection return task is involved.
        let _reservation = self.reservation;
        self.connection
            .close()
            .await
            .map_err(|_| "Privileged verification connection close failed".to_string())
    }
}
pub async fn connect_privileged_connection(
    database_url: &str,
) -> Result<PrivilegedConnection, String> {
    use sqlx::Connection;
    let reservation =
        tokio::time::timeout(Duration::from_secs(5), privileged_budget()?.acquire_owned())
            .await
            .map_err(|_| "Privileged database connection budget timed out")?
            .map_err(|_| "Privileged database connection budget closed")?;
    let options = PgConnectOptions::from_str(database_url)
        .map_err(|_| "Invalid privileged database configuration")?
        .disable_statement_logging();
    let connection = sqlx::PgConnection::connect_with(&options)
        .await
        .map_err(|_| "Privileged verification connection failed")?;
    Ok(PrivilegedConnection {
        connection,
        reservation,
    })
}
