//! Bounded retention turns; remaining durable work is owned by the session tables.
use crate::session_repository::cleanup_auth_state;
use futures::{stream, StreamExt};
use school_tenancy::{
    background::{TenantJobKind, TenantTurnExecutor},
    ActiveSchool, AdminClient, PoolManager,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const SWEEP_BUDGET: Duration = Duration::from_secs(120);
const TENANT_BUDGET: Duration = Duration::from_secs(10);

pub struct AuthRetentionRuntime {
    turns: Arc<TenantTurnExecutor>,
    cursor: Mutex<Option<String>>,
    running: tokio::sync::Mutex<()>,
}

fn rotate_after(schools: &mut [ActiveSchool], cursor: Option<&str>) {
    schools.sort_by(|left, right| left.subdomain.cmp(&right.subdomain));
    if let Some(cursor) = cursor {
        let next = schools.partition_point(|school| school.subdomain.as_str() <= cursor);
        let length = schools.len();
        if length != 0 {
            schools.rotate_left(next % length);
        }
    }
}

impl AuthRetentionRuntime {
    pub fn new(turns: Arc<TenantTurnExecutor>) -> Self {
        Self {
            turns,
            cursor: Mutex::new(None),
            running: tokio::sync::Mutex::new(()),
        }
    }

    async fn run_schools<F, Fut>(&self, mut schools: Vec<ActiveSchool>, cleanup: F)
    where
        F: Fn(ActiveSchool) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let cursor = self.cursor.lock().ok().and_then(|cursor| cursor.clone());
        rotate_after(&mut schools, cursor.as_deref());
        stream::iter(schools)
            .map(|school| {
                let cleanup = &cleanup;
                async move {
                    // Advance when this bounded future is first polled, before
                    // admission: Calendar can occupy every slot for this sweep.
                    if let Ok(mut cursor) = self.cursor.lock() {
                        *cursor = Some(school.subdomain.clone());
                    }
                    let tenant = school.subdomain.clone();
                    self.turns
                        .run(&tenant, TenantJobKind::AuthRetention, || cleanup(school))
                        .await;
                }
            })
            .buffer_unordered(self.turns.limit())
            .for_each(|_| async {})
            .await;
    }

    pub async fn sweep(&self, admin: &AdminClient, pools: &PoolManager) {
        let Ok(_running) = self.running.try_lock() else {
            return;
        };
        let sweep = async {
            let schools = match admin.list_active_schools().await {
                Ok(schools) => schools,
                Err(error) => {
                    tracing::warn!(operation = "auth_retention_cleanup", %error, "Tenant directory unavailable");
                    return;
                }
            };
            self.run_schools(schools, |school| async move {
                let cleanup = async {
                    let Some(url) = school.db_connection_string.filter(|url| !url.is_empty()) else { return; };
                    let pool = match pools.get_pool_without_migrations(&url, &school.subdomain).await {
                        Ok(pool) => pool,
                        Err(error) => {
                            tracing::warn!(operation = "auth_retention_cleanup", %error, "Tenant pool unavailable");
                            return;
                        }
                    };
                    // Retain the logical lease through the complete transaction.
                    if cleanup_auth_state(&pool, chrono::Utc::now()).await.is_err() {
                        tracing::warn!(operation = "auth_retention_cleanup", "Tenant cleanup failed");
                    }
                };
                if tokio::time::timeout(TENANT_BUDGET, cleanup).await.is_err() {
                    tracing::warn!(operation = "auth_retention_cleanup", "Tenant cleanup time budget exhausted");
                }
            }).await;
        };
        if tokio::time::timeout(SWEEP_BUDGET, sweep).await.is_err() {
            tracing::warn!(
                operation = "auth_retention_cleanup",
                "Cleanup sweep time budget exhausted"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn school(subdomain: &str) -> ActiveSchool {
        ActiveSchool {
            subdomain: subdomain.into(),
            db_connection_string: None,
            migration_version: None,
            migration_status: None,
            last_migrated_at: None,
            migration_error: None,
        }
    }
    #[tokio::test]
    async fn cancelled_sweep_waiting_for_all_slots_rotates_past_its_selected_prefix() {
        let turns = Arc::new(TenantTurnExecutor::for_tests(2));
        let mut held = Vec::new();
        for tenant in ["calendar-a", "calendar-b"] {
            let turns = turns.clone();
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            held.push(tokio::spawn(async move {
                turns
                    .run(tenant, TenantJobKind::CalendarReminders, || async {
                        started_tx.send(()).unwrap();
                        std::future::pending::<()>().await;
                    })
                    .await
            }));
            started_rx.await.unwrap();
        }
        let runtime = AuthRetentionRuntime::new(turns);
        let selected = || vec![school("a"), school("b"), school("c"), school("d")];
        assert!(tokio::time::timeout(
            Duration::from_millis(5),
            runtime.run_schools(selected(), |_| async { panic!("all slots occupied") })
        )
        .await
        .is_err());
        assert_eq!(runtime.cursor.lock().unwrap().as_deref(), Some("b"));
        for task in held {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        }
        let visited = Mutex::new(Vec::new());
        runtime
            .run_schools(selected(), |school| {
                visited.lock().unwrap().push(school.subdomain);
                async {}
            })
            .await;
        assert_eq!(*visited.lock().unwrap(), ["c", "d", "a", "b"]);
    }

    #[test]
    fn rotation_reaches_suffix_and_survives_directory_changes() {
        let mut schools = vec![school("c"), school("a"), school("b")];
        rotate_after(&mut schools, Some("a"));
        assert_eq!(
            schools
                .iter()
                .map(|school| school.subdomain.as_str())
                .collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
        rotate_after(&mut schools, Some("b-gone"));
        assert_eq!(schools[0].subdomain, "c");
        rotate_after(&mut schools, Some("z"));
        assert_eq!(schools[0].subdomain, "a");
        rotate_after(&mut [], Some("a"));
    }
}
