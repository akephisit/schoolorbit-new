use crate::SchoolDatabaseInfo;
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{watch, Semaphore};
use uuid::Uuid;

type FillResult = Result<SchoolDatabaseInfo, String>;

struct Flight {
    result: watch::Sender<Option<FillResult>>,
}

struct Entry {
    flight: Option<Arc<Flight>>,
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
    fills: Semaphore,
    admission_timeout: Duration,
}

/// Completing or dropping the leader wakes its original cohort. Outcomes are not
/// retained by the cache, so an independent caller can immediately retry failure.
struct FillLeader<'a> {
    cache: &'a DirectoryCache,
    key: &'a str,
    flight: Arc<Flight>,
    finished: bool,
}

impl FillLeader<'_> {
    fn complete(&mut self, result: FillResult) {
        if let Ok(mut entries) = self.cache.entries.lock() {
            if let Some(entry) = entries.get_mut(self.key) {
                if entry
                    .flight
                    .as_ref()
                    .is_some_and(|flight| Arc::ptr_eq(flight, &self.flight))
                {
                    entry.flight = None;
                }
            }
        }
        self.flight.result.send_replace(Some(result));
        self.finished = true;
    }
}

impl Drop for FillLeader<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.complete(Err("Tenant directory lookup cancelled".into()));
        }
    }
}

impl DirectoryCache {
    pub(crate) fn new(
        capacity: usize,
        ttl: Duration,
        max_in_flight: usize,
        admission_timeout: Duration,
    ) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            capacity,
            ttl,
            fills: Semaphore::new(max_in_flight),
            admission_timeout,
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
                    flight: None,
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

    fn validate_revision(&self, key: &str, revision: Uuid) -> Result<(), String> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| "Tenant directory unavailable".to_string())?;
        entries
            .get(key)
            .filter(|entry| entry.revision == revision && entry.fence.is_none())
            .ok_or_else(|| "Tenant directory lookup superseded".to_string())?;
        Ok(())
    }

    pub(crate) async fn resolve<F, Fut>(&self, key: &str, fetch: F) -> FillResult
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = FillResult>,
    {
        let (flight, revision, leader) = {
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
            if let Some(flight) = &entry.flight {
                (Arc::clone(flight), entry.revision, false)
            } else {
                let (result, _) = watch::channel(None);
                let flight = Arc::new(Flight { result });
                entry.flight = Some(Arc::clone(&flight));
                (flight, entry.revision, true)
            }
        };
        if !leader {
            let mut receiver = flight.result.subscribe();
            loop {
                let result = receiver.borrow_and_update().clone();
                if let Some(result) = result {
                    // Mutation/eviction can happen after completion but before a
                    // follower wakes; never return old credentials in that gap.
                    self.validate_revision(key, revision)?;
                    return result;
                }
                receiver
                    .changed()
                    .await
                    .map_err(|_| "Tenant directory unavailable".to_string())?;
            }
        }
        let mut leader = FillLeader {
            cache: self,
            key,
            flight,
            finished: false,
        };
        let result = async {
            let _permit = tokio::time::timeout(self.admission_timeout, self.fills.acquire())
                .await
                .map_err(|_| "Tenant directory admission timed out".to_string())?
                .map_err(|_| "Tenant directory unavailable".to_string())?;
            self.validate_revision(key, revision)?;
            let fetched = fetch().await;
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| "Tenant directory unavailable".to_string())?;
            let entry = entries
                .get_mut(key)
                .filter(|entry| entry.revision == revision && entry.fence.is_none())
                .ok_or("Tenant directory lookup superseded")?;
            if let Ok(value) = &fetched {
                entry.value = Some((value.clone(), Instant::now() + self.ttl));
                entry.touched = Instant::now();
            }
            fetched
        }
        .await;
        leader.complete(result.clone());
        result
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

    async fn wait_for_followers(cache: &DirectoryCache, key: &str, count: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let ready = cache
                    .entries
                    .lock()
                    .unwrap()
                    .get(key)
                    .and_then(|entry| entry.flight.as_ref())
                    .is_some_and(|flight| flight.result.receiver_count() == count);
                if ready {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn failure_is_shared_with_waiting_cohort_but_next_request_retries() {
        let cache = Arc::new(DirectoryCache::new(
            8,
            Duration::from_secs(60),
            4,
            Duration::from_secs(1),
        ));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let reader = cache.clone();
        let leader = tokio::spawn(async move {
            reader
                .resolve("a", || async {
                    started_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    Err("synthetic outage".into())
                })
                .await
        });
        started_rx.await.unwrap();
        let mut followers = Vec::new();
        for _ in 0..12 {
            let reader = cache.clone();
            followers.push(tokio::spawn(async move {
                reader
                    .resolve("a", || async { panic!("cohort must share failure") })
                    .await
            }));
        }
        wait_for_followers(&cache, "a", 12).await;
        release_tx.send(()).unwrap();
        assert_eq!(leader.await.unwrap().unwrap_err(), "synthetic outage");
        for follower in followers {
            assert_eq!(follower.await.unwrap().unwrap_err(), "synthetic outage");
        }
        assert_eq!(
            cache
                .resolve("a", || async { Ok(value("retry")) })
                .await
                .unwrap()
                .database_url,
            "retry"
        );
    }

    #[tokio::test]
    async fn cancelled_leader_wakes_waiters_and_releases_admission() {
        let cache = Arc::new(DirectoryCache::new(
            8,
            Duration::from_secs(60),
            1,
            Duration::from_secs(1),
        ));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let reader = cache.clone();
        let leader = tokio::spawn(async move {
            reader
                .resolve("a", || async {
                    started_tx.send(()).unwrap();
                    std::future::pending::<FillResult>().await
                })
                .await
        });
        started_rx.await.unwrap();
        let reader = cache.clone();
        let follower = tokio::spawn(async move {
            reader
                .resolve("a", || async { panic!("must share leader") })
                .await
        });
        wait_for_followers(&cache, "a", 1).await;
        leader.abort();
        assert!(leader.await.unwrap_err().is_cancelled());
        assert_eq!(
            follower.await.unwrap().unwrap_err(),
            "Tenant directory lookup cancelled"
        );
        assert_eq!(
            cache
                .resolve("b", || async { Ok(value("b")) })
                .await
                .unwrap()
                .database_url,
            "b"
        );
        assert!(cache
            .resolve("a", || async { Ok(value("retry")) })
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn cancelled_waiter_does_not_cancel_shared_fill() {
        let cache = Arc::new(DirectoryCache::new(
            8,
            Duration::from_secs(60),
            1,
            Duration::from_secs(1),
        ));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let reader = cache.clone();
        let leader = tokio::spawn(async move {
            reader
                .resolve("a", || async {
                    started_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    Ok(value("a"))
                })
                .await
        });
        started_rx.await.unwrap();
        let reader = cache.clone();
        let follower = tokio::spawn(async move {
            reader
                .resolve("a", || async { panic!("must share leader") })
                .await
        });
        wait_for_followers(&cache, "a", 1).await;
        follower.abort();
        assert!(follower.await.unwrap_err().is_cancelled());
        release_tx.send(()).unwrap();
        assert_eq!(leader.await.unwrap().unwrap().database_url, "a");
    }

    #[tokio::test]
    async fn independent_keys_use_available_slots_without_exceeding_the_global_bound() {
        let cache = Arc::new(DirectoryCache::new(
            8,
            Duration::from_secs(60),
            2,
            Duration::from_millis(20),
        ));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let (started_tx, mut started_rx) = tokio::sync::mpsc::channel(2);
        let mut leaders = Vec::new();
        for key in ["a", "b"] {
            let reader = cache.clone();
            let release = release.clone();
            let started = started_tx.clone();
            leaders.push(tokio::spawn(async move {
                reader
                    .resolve(key, || async {
                        started.send(()).await.unwrap();
                        let _release = release.acquire().await.unwrap();
                        Ok(value(key))
                    })
                    .await
            }));
        }
        tokio::time::timeout(Duration::from_secs(1), async {
            started_rx.recv().await.unwrap();
            started_rx.recv().await.unwrap();
        })
        .await
        .unwrap();
        assert_eq!(
            cache
                .resolve("c", || async { panic!("both slots occupied") })
                .await
                .unwrap_err(),
            "Tenant directory admission timed out"
        );
        release.add_permits(2);
        for leader in leaders {
            assert!(leader.await.unwrap().is_ok());
        }
        assert!(cache
            .resolve("c", || async { Ok(value("c")) })
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn distinct_keys_and_evicted_flights_share_bounded_admission() {
        let cache = Arc::new(DirectoryCache::new(
            1,
            Duration::from_secs(60),
            1,
            Duration::from_millis(20),
        ));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let reader = cache.clone();
        let leader = tokio::spawn(async move {
            reader
                .resolve("a", || async {
                    started_tx.send(()).unwrap();
                    std::future::pending::<FillResult>().await
                })
                .await
        });
        started_rx.await.unwrap();
        assert_eq!(
            cache
                .resolve("b", || async {
                    panic!("active evicted fill retains permit")
                })
                .await
                .unwrap_err(),
            "Tenant directory admission timed out"
        );
        leader.abort();
        assert!(leader.await.unwrap_err().is_cancelled());
        assert!(cache
            .resolve("b", || async { Ok(value("b")) })
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn completed_fill_cannot_leak_to_waiter_after_mutation() {
        let cache = Arc::new(DirectoryCache::new(
            8,
            Duration::from_secs(60),
            1,
            Duration::from_secs(1),
        ));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let reader = cache.clone();
        let leader = tokio::spawn(async move {
            let result = reader
                .resolve("a", || async {
                    started_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    Ok(value("old"))
                })
                .await;
            reader.begin("a", Uuid::new_v4()).unwrap();
            result
        });
        started_rx.await.unwrap();
        let reader = cache.clone();
        let follower = tokio::spawn(async move {
            reader
                .resolve("a", || async { panic!("must share leader") })
                .await
        });
        wait_for_followers(&cache, "a", 1).await;
        release_tx.send(()).unwrap();
        assert!(leader.await.unwrap().is_ok());
        assert_eq!(
            follower.await.unwrap().unwrap_err(),
            "Tenant directory lookup superseded"
        );
    }

    #[tokio::test]
    async fn bursts_share_one_fill_and_hits_do_not_fetch() {
        let cache = Arc::new(DirectoryCache::new(
            8,
            Duration::from_secs(1),
            4,
            Duration::from_secs(1),
        ));
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
        let cache = DirectoryCache::new(2, Duration::from_millis(5), 4, Duration::from_secs(1));
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
        let cache = Arc::new(DirectoryCache::new(
            2,
            Duration::from_secs(60),
            4,
            Duration::from_secs(1),
        ));
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
        let cache = Arc::new(DirectoryCache::new(
            1,
            Duration::from_secs(60),
            4,
            Duration::from_secs(1),
        ));
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
