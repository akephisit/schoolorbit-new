use crate::modules::auth::models::User;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(30 * 60); // 30 minutes

struct CacheEntry {
    user: User,              // password_hash cleared, national_id = None
    permissions: Vec<String>,
    cached_at: Instant,
}

/// In-memory permission cache
///
/// Stores (sanitized User, Vec<String>) per user_id:
///   - password_hash is cleared before storing
///   - national_id is not stored (PII)
///
/// Cache hit (within TTL): 0 DB trips
/// Cache miss / expired:    1 DB trip (combined query), then cached
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

    /// Returns (user, permissions) if present and within TTL
    pub fn get(&self, user_id: &Uuid) -> Option<(User, Vec<String>)> {
        let entry = self.inner.get(user_id)?;
        if entry.cached_at.elapsed() > TTL {
            drop(entry);
            self.inner.remove(user_id);
            return None;
        }
        Some((entry.user.clone(), entry.permissions.clone()))
    }

    /// Store sanitized user + permissions (strips password_hash and national_id)
    pub fn set(&self, user_id: Uuid, mut user: User, permissions: Vec<String>) {
        user.password_hash = String::new();
        user.national_id = None;
        self.inner.insert(
            user_id,
            CacheEntry {
                user,
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
