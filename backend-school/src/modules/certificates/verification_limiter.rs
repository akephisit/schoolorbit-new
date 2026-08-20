use std::{
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use dashmap::DashMap;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::AppError;

const IP_ATTEMPT_WINDOW: Duration = Duration::from_secs(5 * 60);
const FAILED_TARGET_WINDOW: Duration = Duration::from_secs(10 * 60);
const STALE_ENTRY_RETENTION: Duration = Duration::from_secs(15 * 60);
const IP_ATTEMPT_LIMIT: u32 = 20;
const FAILED_TARGET_LIMIT: u32 = 6;
const DEFAULT_IP_CAPACITY: usize = 50_000;
const DEFAULT_TARGET_CAPACITY: usize = 50_000;

type VerificationClock = Arc<dyn Fn() -> Instant + Send + Sync>;
type TargetDigest = [u8; 32];
type IpKey = (Uuid, IpAddr);
type TargetKey = (Uuid, IpAddr, TargetDigest);

#[derive(Clone, Copy)]
struct WindowCounter {
    started_at: Instant,
    count: u32,
}

pub struct CertificateVerificationLimiter {
    ip_attempts: DashMap<IpKey, WindowCounter>,
    failed_targets: DashMap<TargetKey, WindowCounter>,
    clock: VerificationClock,
    ip_capacity: usize,
    target_capacity: usize,
    mutation_lock: Mutex<()>,
}

impl CertificateVerificationLimiter {
    pub fn new() -> Self {
        Self::with_clock_and_capacity(
            Arc::new(Instant::now),
            DEFAULT_IP_CAPACITY,
            DEFAULT_TARGET_CAPACITY,
        )
    }

    fn with_clock_and_capacity(
        clock: VerificationClock,
        ip_capacity: usize,
        target_capacity: usize,
    ) -> Self {
        Self {
            ip_attempts: DashMap::new(),
            failed_targets: DashMap::new(),
            clock,
            ip_capacity: ip_capacity.max(1),
            target_capacity: target_capacity.max(1),
            mutation_lock: Mutex::new(()),
        }
    }

    pub fn target_digest(value: &str) -> TargetDigest {
        let mut digest = Sha256::new();
        digest.update(b"schoolorbit-certificate-verification-target-v1\0");
        digest.update(value.as_bytes());
        digest.finalize().into()
    }

    pub fn begin_attempt(
        &self,
        tenant_id: Uuid,
        ip: IpAddr,
        target: TargetDigest,
    ) -> Result<(), AppError> {
        self.begin_ip_attempt(tenant_id, ip)?;
        self.check_target(tenant_id, ip, target)
    }

    pub fn begin_ip_attempt(&self, tenant_id: Uuid, ip: IpAddr) -> Result<(), AppError> {
        let now = (self.clock)();
        let _guard = self
            .mutation_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.remove_stale(now);

        let ip_key = (tenant_id, ip);
        if let Some(mut counter) = self.ip_attempts.get_mut(&ip_key) {
            if now.saturating_duration_since(counter.started_at) >= IP_ATTEMPT_WINDOW {
                *counter = WindowCounter {
                    started_at: now,
                    count: 1,
                };
            } else if counter.count >= IP_ATTEMPT_LIMIT {
                return Err(rate_limited(counter.started_at, now, IP_ATTEMPT_WINDOW));
            } else {
                counter.count += 1;
            }
        } else {
            if self.ip_attempts.len() >= self.ip_capacity {
                return Err(capacity_limited());
            }
            self.ip_attempts.insert(
                ip_key,
                WindowCounter {
                    started_at: now,
                    count: 1,
                },
            );
        }

        Ok(())
    }

    pub fn check_target(
        &self,
        tenant_id: Uuid,
        ip: IpAddr,
        target: TargetDigest,
    ) -> Result<(), AppError> {
        let now = (self.clock)();
        let _guard = self
            .mutation_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.remove_stale(now);

        if let Some(counter) = self.failed_targets.get(&(tenant_id, ip, target)) {
            if now.saturating_duration_since(counter.started_at) < FAILED_TARGET_WINDOW
                && counter.count >= FAILED_TARGET_LIMIT
            {
                return Err(rate_limited(counter.started_at, now, FAILED_TARGET_WINDOW));
            }
        }
        Ok(())
    }

    pub fn record_failure(
        &self,
        tenant_id: Uuid,
        ip: IpAddr,
        target: TargetDigest,
    ) -> Result<(), AppError> {
        let now = (self.clock)();
        let _guard = self
            .mutation_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.remove_stale(now);
        let key = (tenant_id, ip, target);
        if let Some(mut counter) = self.failed_targets.get_mut(&key) {
            if now.saturating_duration_since(counter.started_at) >= FAILED_TARGET_WINDOW {
                *counter = WindowCounter {
                    started_at: now,
                    count: 1,
                };
            } else {
                counter.count = counter.count.saturating_add(1);
            }
            return Ok(());
        }
        if self.failed_targets.len() >= self.target_capacity {
            return Err(capacity_limited());
        }
        self.failed_targets.insert(
            key,
            WindowCounter {
                started_at: now,
                count: 1,
            },
        );
        Ok(())
    }

    pub fn record_success(&self, tenant_id: Uuid, ip: IpAddr, target: TargetDigest) {
        let _guard = self
            .mutation_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.failed_targets.remove(&(tenant_id, ip, target));
    }

    fn remove_stale(&self, now: Instant) {
        self.ip_attempts.retain(|_, counter| {
            now.saturating_duration_since(counter.started_at) <= STALE_ENTRY_RETENTION
        });
        self.failed_targets.retain(|_, counter| {
            now.saturating_duration_since(counter.started_at) <= STALE_ENTRY_RETENTION
        });
    }

    #[cfg(test)]
    fn entry_counts(&self) -> (usize, usize) {
        (self.ip_attempts.len(), self.failed_targets.len())
    }
}

impl Default for CertificateVerificationLimiter {
    fn default() -> Self {
        Self::new()
    }
}

fn rate_limited(started_at: Instant, now: Instant, window: Duration) -> AppError {
    let elapsed = now.saturating_duration_since(started_at);
    AppError::RateLimited {
        retry_after_seconds: window.saturating_sub(elapsed).as_secs().max(1),
    }
}

fn capacity_limited() -> AppError {
    AppError::RateLimited {
        retry_after_seconds: 30,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use uuid::Uuid;

    use super::*;

    #[derive(Clone)]
    struct TestClock(Arc<Mutex<Instant>>);

    impl TestClock {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(Instant::now())))
        }

        fn now(&self) -> Instant {
            *self.0.lock().unwrap()
        }

        fn advance(&self, duration: Duration) {
            let mut now = self.0.lock().unwrap();
            *now += duration;
        }
    }

    fn limiter(clock: &TestClock) -> CertificateVerificationLimiter {
        let clock = clock.clone();
        CertificateVerificationLimiter::with_clock_and_capacity(
            Arc::new(move || clock.now()),
            64,
            64,
        )
    }

    #[test]
    fn limits_twenty_attempts_per_tenant_ip() {
        let clock = TestClock::new();
        let limiter = limiter(&clock);
        let tenant_id = Uuid::new_v4();
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10));
        let target = CertificateVerificationLimiter::target_digest("2569-0001-000001-0");

        for _ in 0..20 {
            limiter.begin_attempt(tenant_id, ip, target).unwrap();
        }
        let error = limiter.begin_attempt(tenant_id, ip, target).unwrap_err();
        assert_eq!(
            error.status_code(),
            axum::http::StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn ip_attempt_limit_resets_at_the_five_minute_boundary() {
        let clock = TestClock::new();
        let limiter = limiter(&clock);
        let tenant_id = Uuid::new_v4();
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 17));

        for _ in 0..20 {
            limiter.begin_ip_attempt(tenant_id, ip).unwrap();
        }
        assert!(matches!(
            limiter.begin_ip_attempt(tenant_id, ip),
            Err(AppError::RateLimited {
                retry_after_seconds: 300
            })
        ));

        clock.advance(Duration::from_secs(5 * 60));
        limiter.begin_ip_attempt(tenant_id, ip).unwrap();
    }

    #[test]
    fn limits_six_failed_attempts_for_one_target_but_not_successes() {
        let clock = TestClock::new();
        let limiter = limiter(&clock);
        let tenant_id = Uuid::new_v4();
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 11));
        let target = CertificateVerificationLimiter::target_digest("2569-0001-000002-8");

        limiter.begin_attempt(tenant_id, ip, target).unwrap();
        limiter.record_success(tenant_id, ip, target);
        for _ in 0..6 {
            limiter.begin_attempt(tenant_id, ip, target).unwrap();
            limiter.record_failure(tenant_id, ip, target).unwrap();
        }
        assert!(matches!(
            limiter.begin_attempt(tenant_id, ip, target),
            Err(crate::error::AppError::RateLimited { .. })
        ));
    }

    #[test]
    fn failed_target_limit_resets_at_the_ten_minute_boundary() {
        let clock = TestClock::new();
        let limiter = limiter(&clock);
        let tenant_id = Uuid::new_v4();
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 18));
        let target = CertificateVerificationLimiter::target_digest("ten-minute-target");

        for _ in 0..6 {
            limiter.check_target(tenant_id, ip, target).unwrap();
            limiter.record_failure(tenant_id, ip, target).unwrap();
        }
        assert!(matches!(
            limiter.check_target(tenant_id, ip, target),
            Err(AppError::RateLimited {
                retry_after_seconds: 600
            })
        ));

        clock.advance(Duration::from_secs(10 * 60));
        limiter.check_target(tenant_id, ip, target).unwrap();
        limiter.record_failure(tenant_id, ip, target).unwrap();
        limiter.check_target(tenant_id, ip, target).unwrap();
    }

    #[test]
    fn split_ip_and_target_checks_count_each_public_render_attempt_once() {
        let clock = TestClock::new();
        let limiter = limiter(&clock);
        let tenant_id = Uuid::new_v4();
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 16));
        let certificate_target =
            CertificateVerificationLimiter::target_digest("certificate-id-constant");

        for _ in 0..6 {
            limiter.begin_ip_attempt(tenant_id, ip).unwrap();
            limiter
                .check_target(tenant_id, ip, certificate_target)
                .unwrap();
            limiter
                .record_failure(tenant_id, ip, certificate_target)
                .unwrap();
        }

        assert!(matches!(
            limiter.check_target(tenant_id, ip, certificate_target),
            Err(crate::error::AppError::RateLimited { .. })
        ));
        assert_eq!(limiter.entry_counts(), (1, 1));
    }

    #[test]
    fn stale_entries_are_removed_lazily_after_fifteen_minutes() {
        let clock = TestClock::new();
        let limiter = limiter(&clock);
        let tenant_id = Uuid::new_v4();
        let stale_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 12));
        let stale_target = CertificateVerificationLimiter::target_digest("stale-target");
        limiter
            .begin_attempt(tenant_id, stale_ip, stale_target)
            .unwrap();
        limiter
            .record_failure(tenant_id, stale_ip, stale_target)
            .unwrap();
        assert_eq!(limiter.entry_counts(), (1, 1));

        clock.advance(Duration::from_secs(15 * 60 + 1));
        let fresh_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 13));
        let fresh_target = CertificateVerificationLimiter::target_digest("fresh-target");
        limiter
            .begin_attempt(tenant_id, fresh_ip, fresh_target)
            .unwrap();
        assert_eq!(limiter.entry_counts(), (1, 0));
    }

    #[test]
    fn capacity_is_bounded_when_no_entry_is_stale() {
        let clock = TestClock::new();
        let now_clock = clock.clone();
        let limiter = CertificateVerificationLimiter::with_clock_and_capacity(
            Arc::new(move || now_clock.now()),
            1,
            1,
        );
        let tenant_id = Uuid::new_v4();
        let first_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 14));
        let second_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 15));
        let target = CertificateVerificationLimiter::target_digest("bounded-target");
        limiter.begin_attempt(tenant_id, first_ip, target).unwrap();

        assert!(matches!(
            limiter.begin_attempt(tenant_id, second_ip, target),
            Err(crate::error::AppError::RateLimited { .. })
        ));
        assert_eq!(limiter.entry_counts(), (1, 0));
    }
}
