use sqlx::{postgres::PgConnectOptions, PgPool};
use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Instant,
};

#[derive(Debug)]
pub(crate) struct Lease {
    started: Instant,
}
impl Lease {
    pub(crate) fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        tracing::debug!(
            operation = "tenant_operation",
            lease_ms = self.started.elapsed().as_millis() as u64,
            "Tenant database logical lease released"
        );
    }
}

#[derive(Clone, Copy)]
pub enum TenantDatabaseOperation {
    CalendarReminder,
    NotificationDispatch,
    FileReconciliation,
    AttendanceMaintenance,
}
impl TenantDatabaseOperation {
    fn label(self) -> &'static str {
        match self {
            Self::CalendarReminder => "calendar_reminder",
            Self::NotificationDispatch => "notification_dispatch",
            Self::FileReconciliation => "file_reconciliation",
            Self::AttendanceMaintenance => "attendance_maintenance",
        }
    }
}

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
    /// Observe a fixed-label checkout without changing SQLx connection ownership.
    pub async fn checkout_pool(
        pool: &PgPool,
        operation: TenantDatabaseOperation,
    ) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, sqlx::Error> {
        let started = Instant::now();
        let connection = pool.acquire().await?;
        tracing::debug!(
            operation = operation.label(),
            checkout_ms = started.elapsed().as_millis() as u64,
            "Tenant database connection checked out"
        );
        Ok(connection)
    }
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

/// A checked-out connection retaining the originating logical operation lease.
/// Only its PostgreSQL connection can be borrowed; pool detachment is unavailable.
pub struct ObservedConnection {
    connection: sqlx::pool::PoolConnection<sqlx::Postgres>,
    retention: Option<TenantPoolRetention>,
    observation: ConnectionHold,
}
impl std::ops::Deref for ObservedConnection {
    type Target = sqlx::PgConnection;
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}
impl std::ops::DerefMut for ObservedConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}
impl ObservedConnection {
    pub fn close_on_drop(&mut self) {
        self.connection.close_on_drop();
    }
    /// SQLx moves its live connection into a cancellation-safe floating guard
    /// before awaiting. Retain the logical lease through that handoff.
    pub async fn return_to_pool(self) {
        let Self {
            mut connection,
            retention,
            observation,
        } = self;
        connection.return_to_pool().await;
        drop(retention);
        drop(observation);
    }
    pub async fn close(self) -> Result<(), sqlx::Error> {
        let Self {
            connection,
            retention,
            observation,
        } = self;
        let result = connection.close().await;
        drop(retention);
        drop(observation);
        result
    }
}
struct ConnectionHold {
    operation: &'static str,
    started: Instant,
}
impl Drop for ConnectionHold {
    fn drop(&mut self) {
        tracing::debug!(
            operation = self.operation,
            hold_us = self.started.elapsed().as_micros() as u64,
            "Tenant database connection hold ended"
        );
    }
}
struct ConnectionCheckout {
    operation: &'static str,
    started: Instant,
    status: &'static str,
}
impl Drop for ConnectionCheckout {
    fn drop(&mut self) {
        tracing::debug!(
            operation = self.operation,
            checkout_us = self.started.elapsed().as_micros() as u64,
            status = self.status,
            "Tenant database connection checkout ended"
        );
    }
}
impl TenantPool {
    pub async fn checkout(
        &self,
        operation: &'static str,
    ) -> Result<ObservedConnection, sqlx::Error> {
        Self::observed_checkout_pool(&self.pool, operation).await
    }
    /// Thin work-boundary adapter for a provider that receives an already-leased pool.
    /// The originating managed operation must still be alive when this is called.
    pub async fn observed_checkout_pool(
        pool: &PgPool,
        operation: &'static str,
    ) -> Result<ObservedConnection, sqlx::Error> {
        let mut observation = ConnectionCheckout {
            operation,
            started: Instant::now(),
            status: "cancelled",
        };
        let retention = match Self::retain_for_pool(pool) {
            Ok(retention) => retention,
            Err(_) => {
                observation.status = "error";
                return Err(sqlx::Error::Protocol(
                    "tenant_operation_lease_unavailable".into(),
                ));
            }
        };
        let connection = match pool.acquire().await {
            Ok(connection) => connection,
            Err(error) => {
                observation.status = "error";
                return Err(error);
            }
        };
        observation.status = "ok";
        Ok(ObservedConnection {
            connection,
            retention,
            observation: ConnectionHold {
                operation,
                started: Instant::now(),
            },
        })
    }
}

/// Domain-selected native pool-owned transaction, with its logical lease retained.
/// The transaction policy, isolation and commit/rollback decisions remain with callers.
pub struct ObservedTransaction {
    transaction: sqlx::Transaction<'static, sqlx::Postgres>,
    retention: Option<TenantPoolRetention>,
    observation: TransactionHold,
}
impl std::ops::Deref for ObservedTransaction {
    type Target = sqlx::Transaction<'static, sqlx::Postgres>;
    fn deref(&self) -> &Self::Target {
        &self.transaction
    }
}
impl std::ops::DerefMut for ObservedTransaction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.transaction
    }
}
impl ObservedTransaction {
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        let Self {
            transaction,
            retention,
            observation,
        } = self;
        let result = transaction.commit().await;
        drop(retention);
        drop(observation);
        result
    }
    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        let Self {
            transaction,
            retention,
            observation,
        } = self;
        let result = transaction.rollback().await;
        drop(retention);
        drop(observation);
        result
    }
}
struct TransactionHold {
    operation: &'static str,
    started: Instant,
}
impl Drop for TransactionHold {
    fn drop(&mut self) {
        tracing::debug!(
            operation = self.operation,
            hold_us = self.started.elapsed().as_micros() as u64,
            resource = "transaction",
            "Tenant database transaction hold ended"
        );
    }
}
struct TransactionBegin {
    operation: &'static str,
    started: Instant,
    status: &'static str,
}
impl Drop for TransactionBegin {
    fn drop(&mut self) {
        tracing::debug!(
            operation = self.operation,
            begin_wait_us = self.started.elapsed().as_micros() as u64,
            status = self.status,
            "Tenant database transaction begin ended"
        );
    }
}
impl TenantPool {
    /// Explicit domain work boundary. This preserves SQLx's pool-owned transaction
    /// lifetime; begin_wait includes acquisition AND PostgreSQL BEGIN, not query time.
    pub async fn begin_observed(
        pool: &PgPool,
        operation: &'static str,
    ) -> Result<ObservedTransaction, sqlx::Error> {
        let mut observation = TransactionBegin {
            operation,
            started: Instant::now(),
            status: "cancelled",
        };
        let retention = match Self::retain_for_pool(pool) {
            Ok(retention) => retention,
            Err(_) => {
                observation.status = "error";
                return Err(sqlx::Error::Protocol(
                    "tenant_operation_lease_unavailable".into(),
                ));
            }
        };
        let transaction = match pool.begin().await {
            Ok(transaction) => transaction,
            Err(error) => {
                observation.status = "error";
                return Err(error);
            }
        };
        observation.status = "ok";
        Ok(ObservedTransaction {
            transaction,
            retention,
            observation: TransactionHold {
                operation,
                started: Instant::now(),
            },
        })
    }
}

#[cfg(test)]
mod transaction_api_tests {
    use super::*;
    async fn domain_transaction_port(_: &mut sqlx::Transaction<'_, sqlx::Postgres>) {}
    async fn owned_scope(pool: &PgPool) -> Result<(ObservedTransaction, i32), sqlx::Error> {
        Ok((
            TenantPool::begin_observed(pool, "test.owned_scope").await?,
            7,
        ))
    }
    async fn type_probe(pool: &PgPool) -> Result<(), sqlx::Error> {
        let (mut transaction, scope) = owned_scope(pool).await?;
        assert_eq!(scope, 7);
        domain_transaction_port(&mut transaction).await;
        let _connection: &mut sqlx::PgConnection = &mut transaction;
        transaction.rollback().await
    }
    #[test]
    fn owned_guard_preserves_native_domain_transaction_and_connection_ports() {
        let _probe = type_probe;
    }
}
