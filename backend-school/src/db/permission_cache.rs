use crate::modules::auth::models::User;
use dashmap::DashMap;
use uuid::Uuid;

/// In-memory permission cache
///
/// Stores (User, permissions) per user_id so check_permission can skip
/// DB entirely on subsequent requests within the same server process.
///
/// Invalidation is explicit — call invalidate/clear_all from handlers
/// that mutate user_roles, role_permissions, or department_permissions.
pub struct PermissionCache {
    inner: DashMap<Uuid, (User, Vec<String>)>,
}

impl PermissionCache {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    /// Returns cached (user, permissions) if present
    pub fn get(&self, user_id: &Uuid) -> Option<(User, Vec<String>)> {
        self.inner
            .get(user_id)
            .map(|entry| entry.value().clone())
    }

    /// Store user + permissions in cache
    pub fn set(&self, user_id: Uuid, user: User, permissions: Vec<String>) {
        self.inner.insert(user_id, (user, permissions));
    }

    /// Remove a single user's cache entry (role assigned/removed, profile updated)
    pub fn invalidate(&self, user_id: &Uuid) {
        self.inner.remove(user_id);
    }

    /// Clear entire cache (role permissions changed, department permissions changed)
    pub fn clear_all(&self) {
        self.inner.clear();
    }
}
