use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Track which schools have been migrated and synced in this session
#[derive(Clone)]
pub struct MigrationTracker {
    migrated: Arc<RwLock<HashSet<String>>>,
    permissions_synced: Arc<RwLock<HashSet<String>>>,
}

impl MigrationTracker {
    pub fn new() -> Self {
        Self {
            migrated: Arc::new(RwLock::new(HashSet::new())),
            permissions_synced: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Check if school has been migrated
    async fn is_migrated(&self, subdomain: &str) -> bool {
        let migrated = self.migrated.read().await;
        migrated.contains(subdomain)
    }

    /// Mark school as migrated
    async fn mark_migrated(&self, subdomain: &str) {
        let mut migrated = self.migrated.write().await;
        migrated.insert(subdomain.to_string());
    }

    /// Check if permissions have been synced
    async fn is_permissions_synced(&self, subdomain: &str) -> bool {
        let synced = self.permissions_synced.read().await;
        synced.contains(subdomain)
    }

    /// Mark permissions as synced
    async fn mark_permissions_synced(&self, subdomain: &str) {
        let mut synced = self.permissions_synced.write().await;
        synced.insert(subdomain.to_string());
    }

    /// Run migrations for a school (once per session)
    pub async fn run_migrations_once(
        &self,
        subdomain: &str,
        pool: &PgPool,
    ) -> Result<bool, String> {
        // Check if already migrated
        if self.is_migrated(subdomain).await {
            return Ok(false); // Already migrated
        }

        println!("🔄 Running migrations for school: {}", subdomain);

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .map_err(|e| format!("Migration failed for {}: {}", subdomain, e))?;

        // Mark as migrated
        self.mark_migrated(subdomain).await;

        println!("✅ Migrations completed for: {}", subdomain);
        Ok(true) // Newly migrated
    }

    /// Sync permissions for a school (once per session)
    pub async fn sync_permissions_once(
        &self,
        subdomain: &str,
        pool: &PgPool,
    ) -> Result<bool, String> {
        // Check if already synced
        if self.is_permissions_synced(subdomain).await {
            return Ok(false); // Already synced
        }

        println!("🔄 Syncing permissions for school: {}", subdomain);

        // Sync permissions
        crate::utils::permission_sync::sync_permissions(pool)
            .await
            .map_err(|e| format!("Permission sync failed for {}: {}", subdomain, e))?;

        // Mark as synced
        self.mark_permissions_synced(subdomain).await;

        println!("✅ Permissions synced for: {}", subdomain);
        Ok(true) // Newly synced
    }

    /// Get list of migrated schools
    pub async fn get_migrated_schools(&self) -> Vec<String> {
        let migrated = self.migrated.read().await;
        migrated.iter().cloned().collect()
    }

    /// Get migration count
    pub async fn migration_count(&self) -> usize {
        let migrated = self.migrated.read().await;
        migrated.len()
    }
}

impl Default for MigrationTracker {
    fn default() -> Self {
        Self::new()
    }
}
