//! Shared database-phase admission in the single School process.
//! Domains retain their own durable work, progress, turn limits and pool leases.
use std::{collections::HashSet, future::Future, sync::Mutex, time::Instant};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TenantJobKind {
    CalendarReminders,
    AuthRetention,
    NotificationDispatch,
    AttendanceMaintenance,
    FileReconciliation,
}

pub struct TenantTurnExecutor {
    slots: Semaphore,
    active: Mutex<HashSet<(String, TenantJobKind)>>,
    limit: usize,
}

pub struct TenantTurn<'a> {
    executor: &'a TenantTurnExecutor,
    key: (String, TenantJobKind),
}
impl Drop for TenantTurn<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.executor.active.lock() {
            active.remove(&self.key);
        }
    }
}

impl TenantTurnExecutor {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            slots: Semaphore::new(limit),
            active: Mutex::new(HashSet::new()),
            limit,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn for_tests(limit: usize) -> Self {
        assert!(limit > 0);
        Self::new(limit)
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Exclude the same tenant/kind across both database and external-I/O phases.
    /// This guard itself owns no database permit or pool.
    pub fn begin(&self, tenant: &str, kind: TenantJobKind) -> Option<TenantTurn<'_>> {
        let key = (tenant.to_owned(), kind);
        if !self.active.lock().ok()?.insert(key.clone()) {
            return None;
        }
        Some(TenantTurn {
            executor: self,
            key,
        })
    }

    /// Callers bound their polled future set to `limit()`. Cancellation releases
    /// both guards; same-key overlap skips without consuming a shared slot.
    pub async fn run<F, Fut, T>(&self, tenant: &str, kind: TenantJobKind, turn: F) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        self.begin(tenant, kind)?.run(turn).await
    }
}

impl TenantJobKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::CalendarReminders => "calendar_reminders",
            Self::AuthRetention => "auth_retention",
            Self::NotificationDispatch => "notification_dispatch",
            Self::AttendanceMaintenance => "attendance_maintenance",
            Self::FileReconciliation => "file_reconciliation",
        }
    }
}

struct PhaseObservation {
    kind: TenantJobKind,
    started: Instant,
}
impl Drop for PhaseObservation {
    fn drop(&mut self) {
        tracing::debug!(
            work = self.kind.label(),
            hold_ms = self.started.elapsed().as_millis() as u64,
            "Tenant background database phase released"
        );
    }
}
impl TenantTurn<'_> {
    /// A batch may run independent claimed items concurrently through this shared
    /// guard. Each executing DB phase counts separately against the semaphore;
    /// the guard excludes other batches rather than serializing item futures.
    /// Only owned domain values may leave a phase. Drop all pool leases and
    /// transactions before returning to external I/O; this API owns no business SQL.
    pub async fn run<F, Fut, T>(&self, phase: F) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let started = Instant::now();
        let _permit = self.executor.slots.acquire().await.ok()?;
        tracing::debug!(
            work = self.key.1.label(),
            admission_ms = started.elapsed().as_millis() as u64,
            "Tenant background database phase admitted"
        );
        let _observation = PhaseObservation {
            kind: self.key.1,
            started: Instant::now(),
        };
        Some(phase().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PoolManagerConfig;
    use std::sync::Arc;
    use std::{
        future::pending,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    #[test]
    fn effective_limit_leaves_a_feasible_resident_slot() {
        assert_eq!(PoolManagerConfig::default().background_turn_limit(64), 3);
        assert_eq!(PoolManagerConfig::default().background_turn_limit(2), 2);
    }

    #[tokio::test]
    async fn stalled_tenant_does_not_block_other_slot_and_overlap_consumes_none() {
        let executor = Arc::new(TenantTurnExecutor::new(2));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let worker = executor.clone();
        let stalled = tokio::spawn(async move {
            worker
                .run("slow", TenantJobKind::CalendarReminders, || async {
                    started_tx.send(()).unwrap();
                    pending::<()>().await;
                })
                .await
        });
        started_rx.await.unwrap();
        assert_eq!(
            executor
                .run("slow", TenantJobKind::CalendarReminders, || async {
                    panic!("overlap")
                })
                .await,
            None::<()>
        );
        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(1),
                executor.run("fast", TenantJobKind::CalendarReminders, || async { 7 })
            )
            .await
            .unwrap(),
            Some(7)
        );
        // Different job kinds use the same aggregate bound but independent identity.
        assert_eq!(
            executor
                .run("slow", TenantJobKind::AuthRetention, || async { 8 })
                .await,
            Some(8)
        );
        stalled.abort();
        assert!(stalled.await.unwrap_err().is_cancelled());
        assert_eq!(
            executor
                .run("slow", TenantJobKind::CalendarReminders, || async { 9 })
                .await,
            Some(9)
        );
    }

    #[tokio::test]
    async fn every_kind_shares_admission_and_io_guard_retains_only_same_key() {
        let executor = Arc::new(TenantTurnExecutor::new(1));
        let notification = executor
            .begin("tenant", TenantJobKind::NotificationDispatch)
            .unwrap();
        assert_eq!(notification.run(|| async { 1 }).await, Some(1));
        assert!(executor
            .begin("tenant", TenantJobKind::NotificationDispatch)
            .is_none());
        // Simulated external I/O retains the batch identity, but no DB slot.
        for kind in [
            TenantJobKind::CalendarReminders,
            TenantJobKind::AuthRetention,
            TenantJobKind::AttendanceMaintenance,
            TenantJobKind::FileReconciliation,
        ] {
            assert_eq!(
                tokio::time::timeout(
                    Duration::from_millis(100),
                    executor.run("other", kind, || async { 2 })
                )
                .await
                .unwrap(),
                Some(2)
            );
        }
        drop(notification);
        assert!(executor
            .begin("tenant", TenantJobKind::NotificationDispatch)
            .is_some());
        assert_eq!(executor.slots.available_permits(), 1);
    }

    #[tokio::test]
    async fn cancellation_while_waiting_or_running_releases_every_guard() {
        let executor = Arc::new(TenantTurnExecutor::new(1));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let worker = executor.clone();
        let running = tokio::spawn(async move {
            worker
                .run("first", TenantJobKind::AuthRetention, || async {
                    started_tx.send(()).unwrap();
                    pending::<()>().await;
                })
                .await
        });
        started_rx.await.unwrap();
        let called = Arc::new(AtomicUsize::new(0));
        let called_by_waiter = called.clone();
        let result = tokio::time::timeout(
            Duration::from_millis(5),
            executor.run("waiting", TenantJobKind::AuthRetention, || async {
                called_by_waiter.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(called.load(Ordering::SeqCst), 0);
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        assert_eq!(
            executor
                .run("waiting", TenantJobKind::AuthRetention, || async { 1 })
                .await,
            Some(1)
        );
        assert!(executor.active.lock().unwrap().is_empty());
        assert_eq!(executor.slots.available_permits(), 1);
    }
}
