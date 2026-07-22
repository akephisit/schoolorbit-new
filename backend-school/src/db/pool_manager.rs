use super::migration::MigrationTracker;
use dashmap::DashMap;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
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
    migration_tracker: MigrationTracker,
    max_connections_per_school: u32,
    pool_ttl: Duration,
}

impl PoolManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            creation_locks: Arc::new(DashMap::new()),
            migration_tracker: MigrationTracker::new(),
            max_connections_per_school: 5,
            pool_ttl: Duration::from_secs(1800), // 30 minutes TTL
        }
    }

    async fn cached_pool_at(&self, key: &str, now: Instant) -> Option<PgPool> {
        let mut pools = self.pools.write().await;
        match pools.get_mut(key) {
            Some(entry) if entry.is_fresh_at(now, self.pool_ttl) => {
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

        let creation_lock = self
            .creation_locks
            .entry(subdomain.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _guard = creation_lock.lock().await;

        if let Some(pool) = self.cached_pool_at(database_url, Instant::now()).await {
            tracing::debug!(
                subdomain,
                "Using tenant pool created by a concurrent request"
            );
            return Ok(pool);
        }

        let pool = create_pool().await?;
        self.pools.write().await.insert(
            database_url.to_string(),
            PoolEntry {
                pool: pool.clone(),
                last_used: Instant::now(),
            },
        );
        Ok(pool)
    }

    /// Get or create a connection pool for a specific school
    /// Also runs migrations lazily on first connection
    pub async fn get_pool(&self, database_url: &str, subdomain: &str) -> Result<PgPool, String> {
        let pool = self
            .get_or_create_pool_with(database_url, subdomain, || async {
                tracing::info!(subdomain, "Creating tenant database pool");
                let connect_options = PgConnectOptions::from_str(database_url)
                    .map_err(|error| {
                        format!("Invalid database configuration for {subdomain}: {error}")
                    })?
                    .statement_cache_capacity(0);

                PgPoolOptions::new()
                    .min_connections(0)
                    .max_connections(self.max_connections_per_school)
                    .acquire_timeout(Duration::from_secs(20))
                    .idle_timeout(Duration::from_secs(300))
                    .test_before_acquire(true)
                    .connect_with(connect_options)
                    .await
                    .map_err(|error| {
                        format!("Failed to connect to database for {subdomain}: {error}")
                    })
            })
            .await?;

        // Run migrations (lazy - only once per school per session)
        self.migration_tracker
            .run_migrations_once(subdomain, &pool)
            .await?;

        // Sync permissions (lazy - only once per school per session)
        // This ensures existing schools get updated permissions after backend deploy
        self.migration_tracker
            .sync_permissions_once(subdomain, &pool)
            .await?;

        Ok(pool)
    }

    /// Get migration tracker for manual operations
    pub fn migration_tracker(&self) -> &MigrationTracker {
        &self.migration_tracker
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
    }

    /// Get current pool count (for monitoring)
    pub async fn pool_count(&self) -> usize {
        self.pools.read().await.len()
    }
}

impl Default for PoolManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
