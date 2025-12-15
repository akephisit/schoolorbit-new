use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

/// Pool cache entry
struct PoolEntry {
    pool: PgPool,
    last_used: Instant,
}

/// Dynamic connection pool manager for multi-tenant
pub struct PoolManager {
    pools: Arc<RwLock<HashMap<String, PoolEntry>>>,
    max_connections_per_school: u32,
    pool_ttl: Duration,
}

impl PoolManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            max_connections_per_school: 5,
            pool_ttl: Duration::from_secs(300), // 5 minutes TTL
        }
    }

    /// Get or create a connection pool for a specific school
    pub async fn get_pool(&self, database_url: &str) -> Result<PgPool, String> {
        let key = database_url.to_string();

        // Try to get existing pool
        {
            let pools = self.pools.read().await;
            if let Some(entry) = pools.get(&key) {
                // Check if pool is still valid
                if entry.last_used.elapsed() < self.pool_ttl {
                    println!("📦 Using cached pool for school");
                    return Ok(entry.pool.clone());
                }
            }
        }

        // Create new pool
        println!("🔄 Creating new connection pool...");
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

        println!("✅ New pool created");
        Ok(pool)
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
