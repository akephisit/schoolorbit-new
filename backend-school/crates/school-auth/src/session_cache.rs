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
    session_repository::{MaintainedSession, PresentedTokenKind, SessionValidity},
};
use school_errors::AppError;

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
    user_type: String,
    remember_me: bool,
    rotated_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    validity: SessionValidity,
    checked_at: DateTime<Utc>,
    has_pending: bool,
}

impl CachedAuthentication {
    fn from_row(row: &MaintainedSession, now: DateTime<Utc>) -> Self {
        Self {
            session_id: row.session_id,
            user_id: row.user_id,
            user_type: row.user_type.clone(),
            remember_me: row.remember_me,
            rotated_at: row.rotated_at,
            last_seen_at: row.last_seen_at,
            validity: SessionValidity {
                idle_expires_at: row.idle_expires_at,
                absolute_expires_at: row.absolute_expires_at,
            },
            checked_at: now,
            has_pending: row.has_pending,
        }
    }

    fn fresh(&self, now: DateTime<Utc>) -> bool {
        now >= self.checked_at && now < self.checked_at + AUTH_TTL && self.validity.is_valid_at(now)
    }

    fn into_row(self) -> MaintainedSession {
        MaintainedSession {
            session_id: self.session_id,
            user_id: self.user_id,
            user_type: self.user_type,
            presented_as: PresentedTokenKind::Current,
            remember_me: self.remember_me,
            rotated_at: self.rotated_at,
            last_seen_at: self.last_seen_at,
            idle_expires_at: self.validity.idle_expires_at,
            absolute_expires_at: self.validity.absolute_expires_at,
            replacement: None,
            has_pending: self.has_pending,
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
    session_id: Option<Uuid>,
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

    fn invalidate(&mut self, tenant: &str, user_id: Option<Uuid>, session_id: Option<Uuid>) {
        self.revision = self.revision.wrapping_add(1);
        self.invalidations.push_back(Invalidation {
            revision: self.revision,
            tenant: tenant.into(),
            user_id,
            session_id,
        });
        if self.invalidations.len() > CAPACITY {
            if let Some(removed) = self.invalidations.pop_front() {
                self.invalidation_floor = removed.revision;
            }
        }
    }

    fn changed_since(&self, revision: u64, tenant: &str, user_id: Uuid, session_id: Uuid) -> bool {
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
                    event.tenant == tenant
                        && event.user_id.is_none_or(|user| user == user_id)
                        && event.session_id.is_none_or(|session| session == session_id)
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
        state.invalidate(tenant, Some(user_id), None);
        state
            .auth
            .retain(|(school, _), row| school != tenant || row.user_id != user_id);
        state
            .validation
            .retain(|(school, _, user), _| school != tenant || *user != user_id);
    }

    pub fn invalidate_identity_tenant(&self, tenant: &str) {
        let mut state = self.state();
        state.invalidate(tenant, None, None);
        state.auth.retain(|(school, _), _| school != tenant);
        state
            .validation
            .retain(|(school, _, _), _| school != tenant);
    }

    pub(crate) fn maintenance_revision(&self) -> u64 {
        self.state().revision
    }

    /// Publish authoritative locked metadata while fencing every older read for this session.
    /// A concurrent account/revocation fence suppresses publication, never invalidation.
    pub(crate) fn publish_maintenance(
        &self,
        tenant: &str,
        hash: TokenHash,
        row: &MaintainedSession,
        now: DateTime<Utc>,
        revision: u64,
        cache_current: bool,
    ) {
        let mut state = self.state();
        let changed = state.changed_since(revision, tenant, row.user_id, row.session_id);
        state.invalidate(tenant, Some(row.user_id), Some(row.session_id));
        state
            .auth
            .retain(|(school, _), cached| school != tenant || cached.session_id != row.session_id);
        state
            .validation
            .retain(|(school, session, _), _| school != tenant || *session != row.session_id);
        if changed {
            return;
        }
        let cached = CachedAuthentication::from_row(row, now);
        state.store_validation(
            (tenant.to_string(), row.session_id, row.user_id),
            CachedValidation {
                validity: cached.validity.clone(),
                checked_at: now,
            },
        );
        if cache_current && row.presented_as == PresentedTokenKind::Current {
            if state.auth.len() >= CAPACITY {
                state.auth.clear();
            }
            // CachedAuthentication copies hashes/metadata only, never row.replacement.
            state
                .auth
                .insert((tenant.to_string(), *hash.as_bytes()), cached);
        }
    }

    /// Cache only read-only current-credential validation. Maintenance is a separate
    /// guarded operation; no invalidation recovery is allowed to issue credentials.
    pub async fn authenticate<F, Fut>(
        &self,
        tenant: &str,
        hash: TokenHash,
        now: DateTime<Utc>,
        fetch: F,
    ) -> Result<Option<MaintainedSession>, AppError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<MaintainedSession>, AppError>>,
    {
        let key = (tenant.to_string(), *hash.as_bytes());
        let _guard = self.lock_for(&key).lock().await;
        let revision = {
            let state = self.state();
            if let Some(row) = state.auth.get(&key).filter(|row| row.fresh(now)) {
                return Ok(Some(row.clone().into_row()));
            }
            state.revision
        };
        let result = fetch().await?;
        let mut state = self.state();
        if result
            .as_ref()
            .is_some_and(|row| state.changed_since(revision, tenant, row.user_id, row.session_id))
        {
            return Err(unavailable());
        }
        state.auth.remove(&key);
        if let Some(row) = &result {
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
            if row.presented_as == PresentedTokenKind::Current && row.replacement.is_none() {
                state.auth.insert(key, cached);
            }
        }
        Ok(result)
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
        if state.changed_since(revision, tenant, user_id, session_id) {
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
