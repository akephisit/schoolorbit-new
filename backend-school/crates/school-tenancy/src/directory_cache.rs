use crate::SchoolDatabaseInfo;
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

struct Entry {
    gate: Arc<AsyncMutex<()>>,
    revision: Uuid,
    value: Option<(SchoolDatabaseInfo, Instant)>,
    touched: Instant,
    fence: Option<Uuid>,
    completed: Option<Uuid>,
}

/// Process-local runtime credentials only. Fences never expire open.
pub(crate) struct DirectoryCache {
    entries: Mutex<HashMap<String, Entry>>,
    capacity: usize,
    ttl: Duration,
}

impl DirectoryCache {
    pub(crate) fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            capacity,
            ttl,
        }
    }

    fn ensure_entry<'a>(
        &self,
        entries: &'a mut HashMap<String, Entry>,
        key: &str,
    ) -> Result<&'a mut Entry, String> {
        if !entries.contains_key(key) {
            if entries.len() >= self.capacity {
                let oldest = entries
                    .iter()
                    .filter(|(_, e)| e.fence.is_none())
                    .min_by_key(|(_, e)| e.touched)
                    .map(|(key, _)| key.clone());
                if let Some(oldest) = oldest {
                    entries.remove(&oldest);
                } else {
                    return Err("Tenant directory capacity unavailable".into());
                }
            }
            entries.insert(
                key.to_owned(),
                Entry {
                    gate: Arc::new(AsyncMutex::new(())),
                    revision: Uuid::new_v4(),
                    value: None,
                    touched: Instant::now(),
                    fence: None,
                    completed: None,
                },
            );
        }
        entries
            .get_mut(key)
            .ok_or_else(|| "Tenant directory entry unavailable".into())
    }

    pub(crate) async fn resolve<F, Fut>(
        &self,
        key: &str,
        fetch: F,
    ) -> Result<SchoolDatabaseInfo, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<SchoolDatabaseInfo, String>>,
    {
        let gate = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| "Tenant directory unavailable".to_string())?;
            let entry = self.ensure_entry(&mut entries, key)?;
            if entry.fence.is_some() {
                return Err("Tenant directory mutation in progress".into());
            }
            entry.touched = Instant::now();
            if let Some((value, expires)) = &entry.value {
                if Instant::now() < *expires {
                    return Ok(value.clone());
                }
            }
            Arc::clone(&entry.gate)
        };
        let _guard = gate.lock().await;
        let revision = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| "Tenant directory unavailable".to_string())?;
            let entry = entries
                .get_mut(key)
                .filter(|entry| Arc::ptr_eq(&entry.gate, &gate))
                .ok_or("Tenant directory lookup superseded")?;
            if entry.fence.is_some() {
                return Err("Tenant directory mutation in progress".into());
            }
            if let Some((value, expires)) = &entry.value {
                if Instant::now() < *expires {
                    return Ok(value.clone());
                }
            }
            entry.revision
        };
        let value = fetch().await?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "Tenant directory unavailable".to_string())?;
        let entry = entries
            .get_mut(key)
            .filter(|entry| {
                entry.revision == revision
                    && entry.fence.is_none()
                    && Arc::ptr_eq(&entry.gate, &gate)
            })
            .ok_or("Tenant directory lookup superseded")?;
        entry.value = Some((value.clone(), Instant::now() + self.ttl));
        entry.touched = Instant::now();
        Ok(value)
    }

    pub(crate) fn begin(&self, key: &str, token: Uuid) -> Result<(), String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "Tenant directory unavailable".to_string())?;
        let entry = self.ensure_entry(&mut entries, key)?;
        if entry.fence == Some(token) {
            return Ok(());
        }
        if entry.fence.is_some() {
            return Err("Tenant directory mutation already in progress".into());
        }
        if entry.completed == Some(token) {
            return Err("Tenant directory mutation already completed".into());
        }
        entry.fence = Some(token);
        entry.value = None;
        entry.revision = Uuid::new_v4();
        Ok(())
    }

    pub(crate) fn finish(&self, key: &str, token: Uuid) -> Result<(), String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "Tenant directory unavailable".to_string())?;
        // Restart loses volatile fences; the Admin durable barrier owns mutation ordering.
        // An absent local fence can be invalidated safely, but never a different active token.
        let entry = self.ensure_entry(&mut entries, key)?;
        if entry.fence.is_none() {
            entry.value = None;
            entry.revision = Uuid::new_v4();
            return Ok(());
        }
        if entry.fence.is_none() && entry.completed == Some(token) {
            return Ok(());
        }
        if entry.fence != Some(token) {
            return Err("Tenant directory mutation token mismatch".into());
        }
        entry.fence = None;
        entry.completed = Some(token);
        entry.value = None;
        entry.revision = Uuid::new_v4();
        entry.touched = Instant::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn value(url: &str) -> SchoolDatabaseInfo {
        SchoolDatabaseInfo {
            tenant_id: Uuid::nil(),
            database_url: url.into(),
        }
    }

    #[tokio::test]
    async fn bursts_share_one_fill_and_hits_do_not_fetch() {
        let cache = Arc::new(DirectoryCache::new(8, Duration::from_secs(1)));
        let fetches = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..24 {
            let cache = cache.clone();
            let fetches = fetches.clone();
            tasks.push(tokio::spawn(async move {
                cache
                    .resolve("a", || async {
                        fetches.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        Ok(value("runtime-a"))
                    })
                    .await
                    .unwrap()
            }));
        }
        for task in tasks {
            assert_eq!(task.await.unwrap().database_url, "runtime-a");
        }
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ttl_failure_has_no_stale_fallback_and_retry_recovers() {
        let cache = DirectoryCache::new(2, Duration::from_millis(5));
        cache
            .resolve("a", || async { Ok(value("old")) })
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(cache
            .resolve("a", || async { Err("outage".into()) })
            .await
            .is_err());
        assert_eq!(
            cache
                .resolve("a", || async { Ok(value("new")) })
                .await
                .unwrap()
                .database_url,
            "new"
        );
    }

    #[tokio::test]
    async fn disable_and_credential_mutation_reject_old_fills_and_tokens() {
        let cache = Arc::new(DirectoryCache::new(2, Duration::from_secs(60)));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let reader = cache.clone();
        let task = tokio::spawn(async move {
            reader
                .resolve("a", || async {
                    started_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    Ok(value("old-credentials"))
                })
                .await
        });
        started_rx.await.unwrap();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        cache.begin("a", first).unwrap();
        cache.begin("a", first).unwrap();
        assert!(cache
            .resolve("a", || async { panic!("fenced cache must not fetch") })
            .await
            .is_err());
        assert!(cache.finish("a", second).is_err());
        cache.finish("a", first).unwrap();
        cache.finish("a", first).unwrap();
        release_tx.send(()).unwrap();
        assert!(task.await.unwrap().is_err());
        assert!(cache
            .resolve("a", || async { Err("not found or inactive".into()) })
            .await
            .is_err());
        assert_eq!(
            cache
                .resolve("a", || async { Ok(value("new-credentials")) })
                .await
                .unwrap()
                .database_url,
            "new-credentials"
        );
        cache.begin("a", second).unwrap();
        assert!(cache.finish("a", first).is_err());
        assert!(cache
            .resolve("a", || async { Ok(value("must-not-return")) })
            .await
            .is_err());
    }

    #[tokio::test]
    async fn bounded_capacity_never_evicts_a_fence_or_accepts_evicted_fill() {
        let cache = Arc::new(DirectoryCache::new(1, Duration::from_secs(60)));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let reader = cache.clone();
        let task = tokio::spawn(async move {
            reader
                .resolve("a", || async {
                    started_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    Ok(value("evicted"))
                })
                .await
        });
        started_rx.await.unwrap();
        let token = Uuid::new_v4();
        cache.begin("b", token).unwrap();
        assert!(cache.begin("c", Uuid::new_v4()).is_err());
        assert!(cache
            .resolve("c", || async { Ok(value("c")) })
            .await
            .is_err());
        release_tx.send(()).unwrap();
        assert!(task.await.unwrap().is_err());
        assert_eq!(cache.entries.lock().unwrap().len(), 1);
        cache.finish("b", token).unwrap();
        assert!(cache
            .resolve("c", || async { Ok(value("c")) })
            .await
            .is_ok());
    }
}
