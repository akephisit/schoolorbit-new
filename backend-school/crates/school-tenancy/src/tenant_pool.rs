use sqlx::{postgres::PgConnectOptions, PgPool};
use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, Mutex, OnceLock, Weak},
};

#[derive(Debug)]
pub(crate) struct Lease;

struct RegistryEntry {
    options: Weak<PgConnectOptions>,
    lease: Weak<Lease>,
}
static REGISTRY: OnceLock<Mutex<HashMap<usize, RegistryEntry>>> = OnceLock::new();
fn registry() -> &'static Mutex<HashMap<usize, RegistryEntry>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// An owned logical database operation. Cloning retains eviction protection across queries.
#[derive(Clone)]
pub struct TenantPool {
    pool: PgPool,
    lease: Option<Arc<Lease>>,
}
impl std::fmt::Debug for TenantPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TenantPool")
            .field("closed", &self.pool.is_closed())
            .finish()
    }
}
impl Deref for TenantPool {
    type Target = PgPool;
    fn deref(&self) -> &PgPool {
        &self.pool
    }
}
impl AsRef<PgPool> for TenantPool {
    fn as_ref(&self) -> &PgPool {
        &self.pool
    }
}
// Unmanaged pools are used by isolated fixtures. Production pools come from PoolManager.
impl From<PgPool> for TenantPool {
    fn from(pool: PgPool) -> Self {
        Self { pool, lease: None }
    }
}

/// Retention token for a detached task whose provider accepts a borrowed raw pool.
pub struct TenantPoolRetention {
    _lease: Arc<Lease>,
}
impl TenantPool {
    pub(crate) fn managed(pool: PgPool, lease: Arc<Lease>) -> Result<Self, String> {
        let options = pool.connect_options();
        let key = Arc::as_ptr(&options) as usize;
        let mut entries = registry()
            .lock()
            .map_err(|_| "Tenant pool lease registry unavailable")?;
        entries.retain(|_, entry| entry.options.strong_count() > 0);
        // One entry per admitted pool; refuse unexpected unbounded manager proliferation.
        if !entries.contains_key(&key) && entries.len() >= 100_000 {
            return Err("Tenant pool lease registry capacity unavailable".into());
        }
        entries.insert(
            key,
            RegistryEntry {
                options: Arc::downgrade(&options),
                lease: Arc::downgrade(&lease),
            },
        );
        Ok(Self {
            pool,
            lease: Some(lease),
        })
    }
    pub(crate) fn unregister(pool: &PgPool) {
        let options = pool.connect_options();
        if let Ok(mut entries) = registry().lock() {
            entries.remove(&(Arc::as_ptr(&options) as usize));
        }
    }
    /// Call while the originating managed context is still alive, before spawning.
    /// The registry owns only Weak identities and cannot keep a pool or lease alive.
    pub fn retain_for_pool(pool: &PgPool) -> Result<Option<TenantPoolRetention>, String> {
        if pool.is_closed() {
            return Err("Tenant database pool closed".into());
        }
        let options = pool.connect_options();
        let entries = registry()
            .lock()
            .map_err(|_| "Tenant pool lease registry unavailable")?;
        let Some(entry) = entries.get(&(Arc::as_ptr(&options) as usize)) else {
            return Ok(None);
        };
        let matches = entry
            .options
            .upgrade()
            .is_some_and(|registered| Arc::ptr_eq(&registered, &options));
        if !matches {
            return Err("Tenant pool identity unavailable".into());
        }
        let lease = entry
            .lease
            .upgrade()
            .ok_or("Tenant pool operation lease expired")?;
        Ok(Some(TenantPoolRetention { _lease: lease }))
    }
    /// Dormant realtime sessions retain identity, then reacquire before every database access.
    pub fn release_for_realtime(&mut self) {
        self.lease = None;
    }
}
