//! Bounded, single-process identity cache. Only hashes and session metadata are stored.
//! Permission and session mutations synchronously invalidate it before returning.
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    hash::{Hash, Hasher},
    sync::{Mutex, MutexGuard},
};

use chrono::{DateTime, Duration, Utc};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use super::{
    session_crypto::TokenHash,
    session_policy::{ROTATION_INTERVAL, TOUCH_INTERVAL},
    session_repository::{
        MaintainedSession, PresentedTokenKind, SessionMaintenanceMode, SessionValidity,
    },
};
use crate::error::AppError;

const AUTH_TTL: Duration = Duration::seconds(60);
const REALTIME_TTL: Duration = Duration::minutes(5);
const CAPACITY: usize = 8192;
const LOCK_STRIPES: usize = 256;

type AuthKey = (String, [u8; 32]);
type ValidationKey = (String, Uuid, Uuid);

#[derive(Clone)]
struct CachedAuthentication {
    session_id: Uuid,
    user_id: Uuid,
    username: String,
    user_type: String,
    remember_me: bool,
    rotated_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    validity: SessionValidity,
    checked_at: DateTime<Utc>,
}

impl CachedAuthentication {
    fn from_row(row: &MaintainedSession, now: DateTime<Utc>) -> Self {
        Self {
            session_id: row.session_id,
            user_id: row.user_id,
            username: row.username.clone(),
            user_type: row.user_type.clone(),
            remember_me: row.remember_me,
            rotated_at: row.rotated_at,
            last_seen_at: row.last_seen_at,
            validity: SessionValidity {
                idle_expires_at: row.idle_expires_at,
                absolute_expires_at: row.absolute_expires_at,
            },
            checked_at: now,
        }
    }

    fn fresh(&self, now: DateTime<Utc>, mode: SessionMaintenanceMode) -> bool {
        now >= self.checked_at
            && now < self.checked_at + AUTH_TTL
            && self.validity.is_valid_at(now)
            && now < self.last_seen_at + TOUCH_INTERVAL
            && (mode == SessionMaintenanceMode::TouchOnly
                || now < self.rotated_at + ROTATION_INTERVAL)
    }

    fn into_row(self) -> MaintainedSession {
        MaintainedSession {
            session_id: self.session_id,
            user_id: self.user_id,
            username: self.username,
            user_type: self.user_type,
            presented_as: PresentedTokenKind::Current,
            remember_me: self.remember_me,
            rotated_at: self.rotated_at,
            last_seen_at: self.last_seen_at,
            idle_expires_at: self.validity.idle_expires_at,
            absolute_expires_at: self.validity.absolute_expires_at,
            replacement: None,
        }
    }
}

#[derive(Clone)]
struct CachedValidation {
    validity: SessionValidity,
    checked_at: DateTime<Utc>,
}

#[derive(Default)]
struct CacheState {
    revision: u64,
    invalidation_floor: u64,
    invalidations: VecDeque<Invalidation>,
    auth: HashMap<AuthKey, CachedAuthentication>,
    validation: HashMap<ValidationKey, CachedValidation>,
}

struct Invalidation {
    revision: u64,
    tenant: String,
    user_id: Option<Uuid>,
}

impl CacheState {
    fn store_validation(&mut self, key: ValidationKey, incoming: CachedValidation) -> bool {
        // Authentication and realtime reads use different stripes. A late read
        // must not overwrite evidence of a newer read or a committed idle touch.
        if let Some(existing) = self.validation.get(&key) {
            if existing.validity.idle_expires_at > incoming.validity.idle_expires_at
                || (existing.validity.idle_expires_at == incoming.validity.idle_expires_at
                    && existing.checked_at > incoming.checked_at)
            {
                return existing.validity.is_valid_at(incoming.checked_at);
            }
        }
        let valid = incoming.validity.is_valid_at(incoming.checked_at);
        if self.validation.len() >= CAPACITY && !self.validation.contains_key(&key) {
            self.validation.clear();
        }
        self.validation.insert(key, incoming);
        valid
    }

    fn invalidate(&mut self, tenant: &str, user_id: Option<Uuid>) {
        self.revision = self.revision.wrapping_add(1);
        self.invalidations.push_back(Invalidation {
            revision: self.revision,
            tenant: tenant.into(),
            user_id,
        });
        if self.invalidations.len() > CAPACITY {
            if let Some(removed) = self.invalidations.pop_front() {
                self.invalidation_floor = removed.revision;
            }
        }
    }

    fn changed_since(&self, revision: u64, tenant: &str, user_id: Uuid) -> bool {
        // An exceptionally slow query that outlives the bounded journal fails
        // closed. Ordinary unrelated changes never discard a committed rotation.
        revision < self.invalidation_floor
            || revision > self.revision
            || self
                .invalidations
                .iter()
                .rev()
                .take_while(|event| event.revision > revision)
                .any(|event| {
                    event.tenant == tenant && event.user_id.is_none_or(|user| user == user_id)
                })
    }
}

pub struct SessionCache {
    state: Mutex<CacheState>,
    // Fixed stripes bound memory even for untrusted, invalid credentials.
    locks: [AsyncMutex<()>; LOCK_STRIPES],
}

impl SessionCache {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CacheState::default()),
            locks: std::array::from_fn(|_| AsyncMutex::new(())),
        }
    }

    fn state(&self) -> MutexGuard<'_, CacheState> {
        self.state.lock().unwrap_or_else(|poisoned| {
            let mut state = poisoned.into_inner();
            state.revision = state.revision.wrapping_add(1);
            state.invalidation_floor = state.revision;
            state.invalidations.clear();
            state.auth.clear();
            state.validation.clear();
            state
        })
    }

    fn lock_for(&self, key: &impl Hash) -> &AsyncMutex<()> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hash);
        &self.locks[hash.finish() as usize % LOCK_STRIPES]
    }

    pub fn invalidate_identity_user(&self, tenant: &str, user_id: Uuid) {
        let mut state = self.state();
        state.invalidate(tenant, Some(user_id));
        state
            .auth
            .retain(|(school, _), row| school != tenant || row.user_id != user_id);
        state
            .validation
            .retain(|(school, _, user), _| school != tenant || *user != user_id);
    }

    pub fn invalidate_identity_tenant(&self, tenant: &str) {
        let mut state = self.state();
        state.invalidate(tenant, None);
        state.auth.retain(|(school, _), _| school != tenant);
        state
            .validation
            .retain(|(school, _, _), _| school != tenant);
    }

    /// `fetch(None)` authenticates normally. A recovery hash must be checked
    /// directly against the database with TouchOnly maintenance, never cached.
    pub async fn authenticate<F, Fut>(
        &self,
        tenant: &str,
        hash: TokenHash,
        now: DateTime<Utc>,
        mode: SessionMaintenanceMode,
        mut fetch: F,
    ) -> Result<Option<MaintainedSession>, AppError>
    where
        F: FnMut(Option<TokenHash>) -> Fut,
        Fut: Future<Output = Result<Option<MaintainedSession>, AppError>>,
    {
        let key = (tenant.to_string(), *hash.as_bytes());
        let _guard = self.lock_for(&key).lock().await;
        let revision = {
            let state = self.state();
            if let Some(row) = state.auth.get(&key).filter(|row| row.fresh(now, mode)) {
                return Ok(Some(row.clone().into_row()));
            }
            state.revision
        };
        let mut result = fetch(None).await?;
        let recovery_revision = {
            let mut state = self.state();
            if result
                .as_ref()
                .is_some_and(|row| state.changed_since(revision, tenant, row.user_id))
            {
                // Keep a committed rotation alive across permission edits, but
                // verify its replacement credential against fresh database state.
                state.revision
            } else {
                state.auth.remove(&key);
                if let Some(row) = &result {
                    // Rotation invalidates any cached use of the old current token. Never
                    // cache a replacement credential or a previous-token grace acceptance.
                    if row.replacement.is_some() {
                        state.auth.retain(|(school, _), cached| {
                            school != tenant || cached.session_id != row.session_id
                        });
                    }
                    if state.auth.len() >= CAPACITY {
                        state.auth.clear();
                    }
                    let cached = CachedAuthentication::from_row(row, now);
                    state.store_validation(
                        (tenant.to_string(), row.session_id, row.user_id),
                        CachedValidation {
                            validity: cached.validity.clone(),
                            checked_at: now,
                        },
                    );
                    if row.presented_as == PresentedTokenKind::Current && row.replacement.is_none()
                    {
                        state.auth.insert(key, cached);
                    }
                }
                return Ok(result);
            }
        };
        let original = result.as_mut().ok_or_else(unavailable)?;
        let replacement = original.replacement.take().ok_or_else(unavailable)?;
        let mut refreshed = fetch(Some(replacement.token_hash()))
            .await?
            .ok_or_else(unavailable)?;
        if refreshed.session_id != original.session_id
            || refreshed.user_id != original.user_id
            || refreshed.presented_as != PresentedTokenKind::Current
            || refreshed.replacement.is_some()
            || now >= refreshed.idle_expires_at
            || now >= refreshed.absolute_expires_at
            || self
                .state()
                .changed_since(recovery_revision, tenant, refreshed.user_id)
        {
            return Err(unavailable());
        }
        // This raced result is deliberately not cached. Only the verified
        // response owns the raw replacement, just like an ordinary rotation.
        refreshed.replacement = Some(replacement);
        Ok(Some(refreshed))
    }

    pub async fn revalidate<F, Fut>(
        &self,
        tenant: &str,
        session_id: Uuid,
        user_id: Uuid,
        now: DateTime<Utc>,
        fetch: F,
    ) -> Result<bool, AppError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<SessionValidity>, AppError>>,
    {
        let key = (tenant.to_string(), session_id, user_id);
        let _guard = self.lock_for(&key).lock().await;
        let revision = {
            let state = self.state();
            if let Some(row) = state.validation.get(&key) {
                if now >= row.validity.absolute_expires_at {
                    return Ok(false);
                }
                if now >= row.checked_at
                    && now < row.checked_at + REALTIME_TTL
                    && row.validity.is_valid_at(now)
                {
                    return Ok(true);
                }
            }
            state.revision
        };
        let result = fetch().await?;
        let mut state = self.state();
        if state.changed_since(revision, tenant, user_id) {
            return Err(unavailable());
        }
        match result {
            Some(validity) => {
                let valid = validity.is_valid_at(now);
                if valid {
                    return Ok(state.store_validation(
                        key,
                        CachedValidation {
                            validity,
                            checked_at: now,
                        },
                    ));
                }
                state.validation.remove(&key);
                Ok(valid)
            }
            None => {
                state.validation.remove(&key);
                Ok(false)
            }
        }
    }
}

fn unavailable() -> AppError {
    AppError::ServiceUnavailable("session_state_changed".into())
}
