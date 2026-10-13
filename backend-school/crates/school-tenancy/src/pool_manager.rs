use dashmap::DashMap;
use sqlx::ConnectOptions;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

/// Pool cache entry
struct PoolEntry {
    pool: PgPool,
    last_used: Instant,
}

impl PoolEntry {
    fn is_fresh_at(&self, now: Instant, ttl: Duration) -> bool {
        now.saturating_duration_since(self.last_used) < ttl
    }

    fn touch(&mut self, now: Instant) {
        self.last_used = now;
    }
}

/// Dynamic connection pool manager for multi-tenant
pub struct PoolManager {
    pools: Arc<RwLock<HashMap<String, PoolEntry>>>,
    creation_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
    max_connections_per_school: u32,
    pool_ttl: Duration,
    lifecycle_timeout: Duration,
}

impl PoolManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            creation_locks: Arc::new(DashMap::new()),
            max_connections_per_school: 5,
            pool_ttl: Duration::from_secs(1800), // 30 minutes TTL
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
            },
        );
    }

    async fn cached_pool_at(&self, key: &str, now: Instant) -> Option<PgPool> {
        let mut pools = self.pools.write().await;
        match pools.get_mut(key) {
            Some(entry) if !entry.pool.is_closed() && entry.is_fresh_at(now, self.pool_ttl) => {
                entry.touch(now);
                Some(entry.pool.clone())
            }
            Some(_) => {
                pools.remove(key);
                None
            }
            None => None,
        }
    }

    async fn get_or_create_pool_with<F, Fut>(
        &self,
        database_url: &str,
        subdomain: &str,
        create_pool: F,
    ) -> Result<PgPool, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<PgPool, String>>,
    {
        if let Some(pool) = self.cached_pool_at(database_url, Instant::now()).await {
            tracing::debug!(subdomain, "Using cached tenant database pool");
            return Ok(pool);
        }

        let started = Instant::now();
        let mut stage = "creation_gate";
        let result = tokio::time::timeout(self.lifecycle_timeout, async {
            let creation_lock = self
                .creation_locks
                .entry(subdomain.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone();
            let _guard = creation_lock.lock().await;

            if let Some(pool) = self.cached_pool_at(database_url, Instant::now()).await {
                return Ok(pool);
            }

            stage = "connect_and_verify";
            let pool = create_pool().await?;
            self.pools.write().await.insert(
                database_url.to_string(),
                PoolEntry {
                    pool: pool.clone(),
                    last_used: Instant::now(),
                },
            );
            Ok(pool)
        })
        .await;
        match result {
            Ok(result) => {
                tracing::info!(
                    subdomain,
                    operation = "tenant_pool_creation",
                    stage,
                    duration_ms = started.elapsed().as_secs_f64() * 1000.0,
                    succeeded = result.is_ok()
                );
                result
            }
            Err(_) => {
                tracing::warn!(
                    subdomain,
                    operation = "tenant_pool_creation",
                    stage,
                    error_code = "tenant_pool_creation_timeout",
                    duration_ms = started.elapsed().as_secs_f64() * 1000.0
                );
                Err("Tenant database initialization timed out".into())
            }
        }
    }

    /// Get or create a connection pool for a specific school
    /// Runtime requests never run migrations or permission synchronization.
    pub async fn get_pool(&self, database_url: &str, subdomain: &str) -> Result<PgPool, String> {
        self.get_pool_without_migrations(database_url, subdomain)
            .await
    }

    /// Get or create a tenant pool without running migrations or permission synchronization.
    /// Scheduled work uses this only after the centralized deployment migration gate.
    pub async fn get_pool_without_migrations(
        &self,
        database_url: &str,
        subdomain: &str,
    ) -> Result<PgPool, String> {
        let pool = self
            .get_or_create_pool_with(database_url, subdomain, || async {
                tracing::info!(subdomain, "Creating tenant database pool");
                let connect_options = PgConnectOptions::from_str(database_url)
                    .map_err(|error| {
                        let _ = error; format!("Invalid database configuration for {subdomain}")
                    })?
                    .statement_cache_capacity(100)
                    .disable_statement_logging();

                // Verify the first connection while SQLx still owns it as a raw
                // connection. Cancelling initialization then drops its socket directly,
                // instead of spawning an unbounded PoolConnection return/ping task.
                // Later connections in this already-verified pool need no repeated scan.
                let verified = Arc::new(AtomicBool::new(false));
                let tenant = subdomain.to_owned();
                let pool = PgPoolOptions::new()
                    .after_connect(move |connection, _| {
                        let verified = Arc::clone(&verified);
                        let tenant = tenant.clone();
                        Box::pin(async move {
                            if !verified.load(Ordering::Acquire) {
                                tracing::info!(subdomain = tenant, operation = "tenant_pool_creation", stage = "schema_history_started");
                                let version = school_migrations::verify_tenant_schema(connection)
                                    .await.map_err(sqlx::Error::Protocol)?;
                                verified.store(true, Ordering::Release);
                                tracing::info!(subdomain = tenant, operation = "tenant_pool_creation", stage = "schema_history_verified", version);
                            }
                            Ok(())
                        })
                    })
                    .min_connections(0)
                    .acquire_time_level(log::LevelFilter::Info)
                    .acquire_slow_threshold(Duration::from_millis(250))
                    .max_connections(self.max_connections_per_school)
                    .acquire_timeout(Duration::from_secs(20))
                    .idle_timeout(Duration::from_secs(300))
                    .test_before_acquire(true)
                    .connect_with(connect_options)
                    .await
                    .map_err(|error| {
                        tracing::warn!(database_code = ?error.as_database_error().and_then(|e| e.code()), "Tenant connection failed"); format!("Failed to connect to database for {subdomain}")
                    })?;
                Ok(pool)
            })
            .await?;

        Ok(pool)
    }

    /// Get a non-migrating tenant pool for read-only operational status checks.
    pub async fn get_pool_for_read_only_status(
        &self,
        database_url: &str,
        subdomain: &str,
    ) -> Result<PgPool, String> {
        self.get_pool_without_migrations(database_url, subdomain)
            .await
    }

    /// Cleanup expired pools (call periodically)
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        let mut pools = self.pools.write().await;
        pools.retain(|_, entry| {
            let expired = !entry.is_fresh_at(now, self.pool_ttl);
            if expired {
                tracing::info!("🧹 Removing expired pool");
            }
            !expired
        });
        self.creation_locks
            .retain(|_, lock| Arc::strong_count(lock) > 1);
    }

    /// Deployment retires pre-migration connections before registering runtime work.
    pub async fn retire_pool(&self, database_url: &str, subdomain: &str) -> Result<(), String> {
        let retired = tokio::time::timeout(self.lifecycle_timeout, async {
            let creation_lock = self
                .creation_locks
                .entry(subdomain.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone();
            let _guard = creation_lock.lock().await;
            let retired = self.pools.write().await.remove(database_url);
            // close() marks the pool closed synchronously, before releasing the gate.
            if let Some(entry) = &retired {
                drop(entry.pool.close());
            }
            retired
        })
        .await
        .map_err(|_| {
            tracing::warn!(
                subdomain,
                operation = "tenant_pool_retirement",
                stage = "creation_gate",
                error_code = "tenant_pool_retirement_timeout"
            );
            "Tenant pool retirement timed out".to_string()
        })?;
        if let Some(entry) = retired {
            close_pool_with_timeout(
                &entry.pool,
                "tenant_pool_retirement",
                self.lifecycle_timeout,
            )
            .await?;
        }
        Ok(())
    }

    /// Get current pool count (for monitoring)
    pub async fn pool_count(&self) -> usize {
        self.pools.read().await.len()
    }
}

/// Stop accepting acquisitions immediately and bound graceful drainage. This does not
/// cancel a borrower's transaction or retry its writes. Deployment fails closed on timeout.
pub async fn close_pool(pool: &PgPool, operation: &'static str) -> Result<(), String> {
    close_pool_with_timeout(pool, operation, Duration::from_secs(30)).await
}

async fn close_pool_with_timeout(
    pool: &PgPool,
    operation: &'static str,
    timeout: Duration,
) -> Result<(), String> {
    let started = Instant::now();
    match tokio::time::timeout(timeout, pool.close()).await {
        Ok(()) => {
            tracing::info!(
                operation,
                duration_ms = started.elapsed().as_secs_f64() * 1000.0,
                "Database pool drained"
            );
            Ok(())
        }
        Err(_) => {
            tracing::warn!(
                operation,
                error_code = "database_pool_drain_timeout",
                pool_connections = pool.size(),
                pool_idle_connections = pool.num_idle(),
                duration_ms = started.elapsed().as_secs_f64() * 1000.0,
                "Database pool stopped accepting connections but has outstanding borrowers"
            );
            Err("Database pool drainage timed out".into())
        }
    }
}

impl Default for PoolManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
                .fetch_one(&fresh)
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
                .fetch_one(&recovered)
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
                .fetch_one(&recovered)
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
            manager.get_or_create_pool_with("postgres://retirement", "tenant", || async {
                Ok(lazy_pool("after_retirement"))
            }),
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
        Arc,
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
            manager.get_or_create_pool_with("postgres://stalled", "stalled", || async {
                std::future::pending::<Result<sqlx::PgPool, String>>().await
            }),
        )
        .await
        .expect("pool creation must have its own deadline");
        assert!(stalled.is_err());
        assert_eq!(manager.pool_count().await, 0);
        let recovered = manager
            .get_or_create_pool_with("postgres://stalled", "stalled", || async {
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
                .get_or_create_pool_with("postgres://cancel", "cancel", || async {
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
            manager.get_or_create_pool_with("postgres://cancel", "cancel", || async {
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
            .get_or_create_pool_with("postgres://closed", "closed", || async {
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
            },
        );

        assert!(manager
            .cached_pool_at(database_url, touched_at)
            .await
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
            },
        );
        let creations = AtomicUsize::new(0);

        let result = manager
            .get_or_create_pool_with(database_url, "expired", || async {
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
                    .get_or_create_pool_with("postgres://tenant-one", "tenant-one", || async move {
                        creations.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok(lazy_pool("tenant_one"))
                    })
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
                    .get_or_create_pool_with(database_url, subdomain, || async move {
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
            .get_or_create_pool_with("postgres://retry", "retry", || async {
                creations.fetch_add(1, Ordering::SeqCst);
                Err("first creation failed".to_string())
            })
            .await;
        assert_eq!(
            first.expect_err("first creation must fail"),
            "first creation failed"
        );

        let second = manager
            .get_or_create_pool_with("postgres://retry", "retry", || async {
                creations.fetch_add(1, Ordering::SeqCst);
                Ok(lazy_pool("retry"))
            })
            .await;
        assert!(second.is_ok());
        assert_eq!(creations.load(Ordering::SeqCst), 2);
    }
}
