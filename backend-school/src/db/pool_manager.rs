use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};
use super::migration::MigrationTracker;

/// Pool cache entry
struct PoolEntry {
    pool: PgPool,
    last_used: Instant,
}

/// Dynamic connection pool manager for multi-tenant
pub struct PoolManager {
    pools: Arc<RwLock<HashMap<String, PoolEntry>>>,
    migration_tracker: MigrationTracker,
    max_connections_per_school: u32,
    pool_ttl: Duration,
}

impl PoolManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            migration_tracker: MigrationTracker::new(),
            max_connections_per_school: 5,
            pool_ttl: Duration::from_secs(300), // 5 minutes TTL
        }
    }

    /// Get or create a connection pool for a specific school
    /// Also runs migrations lazily on first connection
    pub async fn get_pool(&self, database_url: &str, subdomain: &str) -> Result<PgPool, String> {
        let key = database_url.to_string();

        // Try to get existing pool
        let pool = {
            let pools = self.pools.read().await;
            if let Some(entry) = pools.get(&key) {
                // Check if pool is still valid
                if entry.last_used.elapsed() < self.pool_ttl {
                    println!("📦 Using cached pool for school: {}", subdomain);
                    Some(entry.pool.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        let pool = match pool {
            Some(p) => p,
            None => {
                // Create new pool
                println!("🔄 Creating new connection pool for: {}", subdomain);
                let pool = PgPoolOptions::new()
                    .max_connections(self.max_connections_per_school)
                    .acquire_timeout(Duration::from_secs(10))
                    .connect(&database_url)
                    .await
                    .map_err(|e| format!("Failed to connect to database: {}", e))?;

                // Store in cache
                {
                    let mut pools = self.pools.write().await;
                    pools.insert(key, PoolEntry {
                        pool: pool.clone(),
                        last_used: Instant::now(),
                    });
                }

                println!("✅ New pool created for: {}", subdomain);
                pool
            }
        };

        // Run migrations (lazy - only once per school per session)
        self.migration_tracker
            .run_migrations_once(subdomain, &pool)
            .await?;

        // Sync permissions to database after migrations
        if let Err(e) = crate::utils::permission_sync::sync_permissions(&pool).await {
            eprintln!("⚠️  Failed to sync permissions for {}: {}", subdomain, e);
            // Don't fail the request, just log the error
        } else {
            println!("✅ Permissions synced for: {}", subdomain);
        }

        Ok(pool)
    }

    /// Get migration tracker for manual operations
    pub fn migration_tracker(&self) -> &MigrationTracker {
        &self.migration_tracker
    }

    /// Cleanup expired pools (call periodically)
    pub async fn cleanup_expired(&self) {
        let mut pools = self.pools.write().await;
        pools.retain(|_, entry| {
            let expired = entry.last_used.elapsed() >= self.pool_ttl;
            if expired {
                println!("🧹 Removing expired pool");
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
