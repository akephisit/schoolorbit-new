//! Admission for Calendar and Auth turns in the single School process.
//! Domains retain their own durable work, progress, turn limits and pool leases.
use crate::PoolManagerConfig;
use std::{collections::HashSet, future::Future, sync::Mutex};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TenantJobKind {
    CalendarReminders,
    AuthRetention,
}

pub struct TenantTurnExecutor {
    slots: Semaphore,
    active: Mutex<HashSet<(String, TenantJobKind)>>,
    limit: usize,
}

struct ActiveTurn<'a> {
    executor: &'a TenantTurnExecutor,
    key: (String, TenantJobKind),
}
impl Drop for ActiveTurn<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.executor.active.lock() {
            active.remove(&self.key);
        }
    }
}

fn requested_limit(raw: Option<&str>) -> Result<usize, String> {
    let requested = raw
        .map(str::parse::<usize>)
        .transpose()
        .map_err(|_| "TENANT_BACKGROUND_MAX_TURNS must be an integer")?
        .unwrap_or(2);
    if !(1..=64).contains(&requested) {
        return Err("TENANT_BACKGROUND_MAX_TURNS is outside the supported range".into());
    }
    Ok(requested)
}

impl TenantTurnExecutor {
    pub fn from_env(pools: &PoolManagerConfig) -> Result<Self, String> {
        let requested =
            requested_limit(std::env::var("TENANT_BACKGROUND_MAX_TURNS").ok().as_deref())?;
        Ok(Self::new(pools.background_turn_limit(requested)))
    }

    fn new(limit: usize) -> Self {
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

    /// Callers bound their polled future set to `limit()`. The closure starts only
    /// after admission; same-tenant/kind overlap skips without consuming a slot.
    /// Dropping this future releases both guards and all caller-owned work.
    pub async fn run<F, Fut, T>(&self, tenant: &str, kind: TenantJobKind, turn: F) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let key = (tenant.to_owned(), kind);
        {
            let mut active = self.active.lock().ok()?;
            if !active.insert(key.clone()) {
                return None;
            }
        }
        let _active = ActiveTurn {
            executor: self,
            key,
        };
        let _permit = self.slots.acquire().await.ok()?;
        Some(turn().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::{
        future::pending,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    #[test]
    fn requested_limit_is_bounded_without_mutating_process_environment() {
        assert_eq!(requested_limit(None).unwrap(), 2);
        assert_eq!(requested_limit(Some("64")).unwrap(), 64);
        for raw in ["0", "65", "invalid"] {
            assert!(requested_limit(Some(raw)).is_err());
        }
    }

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
