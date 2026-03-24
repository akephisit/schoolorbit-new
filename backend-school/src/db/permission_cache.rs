use dashmap::DashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(30 * 60); // 30 minutes

struct CacheEntry {
    permissions: Vec<String>,
    cached_at: Instant,
}

/// In-memory permission cache
///
/// Stores only Vec<String> (permission codes) per user_id — no sensitive data
/// (password_hash / national_id stay out of the cache).
///
/// - Cache hit within TTL: use cached permissions, fetch user with simple SELECT
/// - Cache hit expired: treated as miss, entry is evicted
/// - Cache miss: combined SQL query, result cached
///
/// Invalidation is explicit from mutation handlers, with TTL as safety net.
pub struct PermissionCache {
    inner: DashMap<Uuid, CacheEntry>,
}

impl PermissionCache {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    /// Returns cached permissions if present and within TTL
    pub fn get(&self, user_id: &Uuid) -> Option<Vec<String>> {
        let entry = self.inner.get(user_id)?;
        if entry.cached_at.elapsed() > TTL {
            drop(entry);
            self.inner.remove(user_id);
            return None;
        }
        Some(entry.permissions.clone())
    }

    /// Store permissions in cache
    pub fn set(&self, user_id: Uuid, permissions: Vec<String>) {
        self.inner.insert(
            user_id,
            CacheEntry {
                permissions,
                cached_at: Instant::now(),
            },
        );
    }

    /// Remove a single user's cache entry
    pub fn invalidate(&self, user_id: &Uuid) {
        self.inner.remove(user_id);
    }

    /// Clear entire cache (role/department permissions changed)
    pub fn clear_all(&self) {
        self.inner.clear();
    }
}
