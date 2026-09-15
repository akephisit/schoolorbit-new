use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
struct CacheEntry {
    permissions: Vec<String>,
    cached_at: Instant,
    revision: PermissionCacheRevision,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TenantUserKey {
    pub tenant: String,
    pub user_id: Uuid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PermissionCacheRevision {
    tenant: u64,
    user: u64,
}

#[derive(Default)]
struct RevisionState {
    tenants: HashMap<String, u64>,
    users: HashMap<TenantUserKey, u64>,
}

impl RevisionState {
    fn snapshot(&self, tenant: &str, user_id: Uuid) -> PermissionCacheRevision {
        let key = PermissionCache::key(tenant, user_id);
        PermissionCacheRevision {
            tenant: self.tenants.get(tenant).copied().unwrap_or_default(),
            user: self.users.get(&key).copied().unwrap_or_default(),
        }
    }

    fn advance_user(&mut self, key: TenantUserKey) {
        let revision = self.users.entry(key).or_default();
        *revision = revision.wrapping_add(1);
    }

    fn advance_tenant(&mut self, tenant: &str) {
        let revision = self.tenants.entry(tenant.to_string()).or_default();
        *revision = revision.wrapping_add(1);
    }
}

pub struct PermissionCache {
    inner: DashMap<TenantUserKey, CacheEntry>,
    revisions: Mutex<RevisionState>,
}

impl PermissionCache {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
            revisions: Mutex::new(RevisionState::default()),
        }
    }

    fn revision_state(&self) -> MutexGuard<'_, RevisionState> {
        match self.revisions.lock() {
            Ok(revisions) => revisions,
            Err(poisoned) => {
                tracing::warn!("Permission cache revision mutex was poisoned; recovering state");
                poisoned.into_inner()
            }
        }
    }

    fn remove_if_same(&self, key: &TenantUserKey, expected: &CacheEntry) {
        let should_remove = self
            .inner
            .get(key)
            .map(|current| {
                current.revision == expected.revision && current.cached_at == expected.cached_at
            })
            .unwrap_or(false);
        if should_remove {
            self.inner.remove(key);
        }
    }

    pub fn snapshot_revision(&self, tenant: &str, user_id: Uuid) -> PermissionCacheRevision {
        self.revision_state().snapshot(tenant, user_id)
    }

    fn key(tenant: &str, user_id: Uuid) -> TenantUserKey {
        TenantUserKey {
            tenant: tenant.to_string(),
            user_id,
        }
    }

    pub fn get(&self, tenant: &str, user_id: Uuid) -> Option<Vec<String>> {
        let key = Self::key(tenant, user_id);
        let entry = self.inner.get(&key).map(|entry| entry.clone())?;
        let revisions = self.revision_state();
        if entry.revision != revisions.snapshot(tenant, user_id) || entry.cached_at.elapsed() > TTL
        {
            self.remove_if_same(&key, &entry);
            return None;
        }
        Some(entry.permissions.clone())
    }

    pub fn fill_if_current(
        &self,
        tenant: &str,
        user_id: Uuid,
        revision: PermissionCacheRevision,
        permissions: Vec<String>,
    ) -> bool {
        let revisions = self.revision_state();
        if revisions.snapshot(tenant, user_id) != revision {
            return false;
        }
        self.inner.insert(
            Self::key(tenant, user_id),
            CacheEntry {
                permissions,
                cached_at: Instant::now(),
                revision,
            },
        );
        true
    }

    pub fn invalidate_user(&self, tenant: &str, user_id: Uuid) {
        let key = Self::key(tenant, user_id);
        let mut revisions = self.revision_state();
        revisions.advance_user(key.clone());
        self.inner.remove(&key);
    }

    pub fn invalidate_tenant(&self, tenant: &str) {
        let mut revisions = self.revision_state();
        revisions.advance_tenant(tenant);
        self.inner.retain(|key, _| key.tenant != tenant);
    }
}

impl Default for PermissionCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(cache: &PermissionCache, tenant: &str, user_id: Uuid, permissions: Vec<String>) {
        let revision = cache.snapshot_revision(tenant, user_id);
        assert!(cache.fill_if_current(tenant, user_id, revision, permissions));
    }

    #[test]
    fn entries_and_invalidations_are_tenant_scoped() {
        let cache = PermissionCache::new();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        fill(&cache, "tenant-a", first, vec!["a.read".into()]);
        fill(&cache, "tenant-a", second, vec!["a.write".into()]);
        fill(&cache, "tenant-b", first, vec!["b.read".into()]);

        cache.invalidate_user("tenant-a", first);
        assert!(cache.get("tenant-a", first).is_none());
        assert_eq!(cache.get("tenant-b", first), Some(vec!["b.read".into()]));
        cache.invalidate_tenant("tenant-a");
        assert!(cache.get("tenant-a", second).is_none());
        assert!(cache.get("tenant-b", first).is_some());
    }

    #[test]
    fn invalidation_rejects_in_flight_stale_fills() {
        let cache = PermissionCache::new();
        let user_id = Uuid::new_v4();
        let user_revision = cache.snapshot_revision("tenant-a", user_id);
        cache.invalidate_user("tenant-a", user_id);
        assert!(!cache.fill_if_current(
            "tenant-a",
            user_id,
            user_revision,
            vec!["stale.read".into()]
        ));

        let tenant_revision = cache.snapshot_revision("tenant-a", user_id);
        cache.invalidate_tenant("tenant-a");
        assert!(!cache.fill_if_current(
            "tenant-a",
            user_id,
            tenant_revision,
            vec!["stale.read".into()]
        ));
    }

    #[test]
    fn expired_entries_are_removed() {
        let cache = PermissionCache::new();
        let user_id = Uuid::new_v4();
        let key = PermissionCache::key("tenant-a", user_id);
        let revision = cache.snapshot_revision("tenant-a", user_id);
        cache.inner.insert(
            key,
            CacheEntry {
                permissions: vec!["a.read".into()],
                cached_at: Instant::now() - TTL - Duration::from_secs(1),
                revision,
            },
        );
        assert!(cache.get("tenant-a", user_id).is_none());
        assert!(cache.inner.is_empty());
    }
}
