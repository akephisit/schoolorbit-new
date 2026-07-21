use dashmap::DashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(30 * 60); // 30 minutes

struct CacheEntry {
    permissions: Vec<String>,
    cached_at: Instant,
}

/// In-memory permission cache — stores only Vec<String> per tenant and user_id.
///
/// Cache hit (within TTL): 0 DB trips — JWT verify + cache lookup only
/// Cache miss / expired:   1 DB trip — permissions-only query, then cached
///
/// Invalidation is explicit from mutation handlers, with TTL as safety net.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TenantUserKey {
    pub tenant: String,
    pub user_id: Uuid,
}

pub struct PermissionCache {
    inner: DashMap<TenantUserKey, CacheEntry>,
}

impl PermissionCache {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    fn key(tenant: &str, user_id: Uuid) -> TenantUserKey {
        TenantUserKey {
            tenant: tenant.to_string(),
            user_id,
        }
    }

    /// Returns cached permissions if present and within TTL
    pub fn get(&self, tenant: &str, user_id: Uuid) -> Option<Vec<String>> {
        let key = Self::key(tenant, user_id);
        let entry = self.inner.get(&key)?;
        if entry.cached_at.elapsed() > TTL {
            drop(entry);
            self.inner.remove(&key);
            return None;
        }
        Some(entry.permissions.clone())
    }

    /// Store permissions in cache
    pub fn set(&self, tenant: &str, user_id: Uuid, permissions: Vec<String>) {
        self.inner.insert(
            Self::key(tenant, user_id),
            CacheEntry {
                permissions,
                cached_at: Instant::now(),
            },
        );
    }

    /// Remove a single user's cache entry
    pub fn invalidate_user(&self, tenant: &str, user_id: Uuid) {
        self.inner.remove(&Self::key(tenant, user_id));
    }

    /// Clear one tenant's cache (role/organization permissions changed)
    pub fn invalidate_tenant(&self, tenant: &str) {
        self.inner.retain(|key, _| key.tenant != tenant);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_user_ids_are_isolated_by_tenant() {
        let cache = PermissionCache::new();
        let user_id = Uuid::new_v4();
        cache.set("tenant-a", user_id, vec!["a.read".into()]);
        cache.set("tenant-b", user_id, vec!["b.read".into()]);
        assert_eq!(cache.get("tenant-a", user_id), Some(vec!["a.read".into()]));
        assert_eq!(cache.get("tenant-b", user_id), Some(vec!["b.read".into()]));
    }

    #[test]
    fn invalidation_never_crosses_tenant_boundary() {
        let cache = PermissionCache::new();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        cache.set("tenant-a", first, vec!["a".into()]);
        cache.set("tenant-a", second, vec!["a".into()]);
        cache.set("tenant-b", first, vec!["b".into()]);
        cache.invalidate_user("tenant-a", first);
        assert!(cache.get("tenant-a", first).is_none());
        assert!(cache.get("tenant-b", first).is_some());
        cache.invalidate_tenant("tenant-a");
        assert!(cache.get("tenant-a", second).is_none());
        assert!(cache.get("tenant-b", first).is_some());
    }

    #[test]
    fn expired_entries_are_removed() {
        let cache = PermissionCache::new();
        let user_id = Uuid::new_v4();
        let key = TenantUserKey {
            tenant: "tenant-a".into(),
            user_id,
        };
        cache.inner.insert(
            key,
            CacheEntry {
                permissions: vec!["a.read".into()],
                cached_at: Instant::now() - TTL - Duration::from_secs(1),
            },
        );
        assert!(cache.get("tenant-a", user_id).is_none());
        assert!(cache.inner.is_empty());
    }
}
