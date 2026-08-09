use std::{fmt, sync::Arc};

use chrono::{DateTime, Utc};
use subtle::ConstantTimeEq;
use tokio::sync::broadcast;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    db::permission_cache::PermissionCache, error::AppError,
    middleware::permission::get_cached_user_permissions, utils::tenant::TenantContext,
};

use super::{
    audit::{self, LoginRejectionReason, SessionFailureReason},
    config::SessionConfig,
    events::SessionRevocationEvent,
    services::{
        find_active_user_shell_by_id, find_session_login_user_by_username, get_primary_role_name,
        ActiveUserShell, SessionLoginUser,
    },
    session_crypto::{
        identifier_bucket, session_csrf_token, source_bucket, CsrfToken, EncodedSessionToken,
        RawSessionToken, TokenHash,
    },
    session_policy::{
        cookie_max_age_seconds, device_label, normalize_login_identifier, retry_after_seconds,
        validate_login_input, validate_new_password, SessionLifetime, SessionTimes,
    },
    session_repository::{
        apply_password_change, authenticate_and_maintain, cleanup_auth_state, list_user_sessions,
        load_password_change_snapshot, lock_password_change, revalidate_session, revoke_sessions,
        NewSession, SessionMaintenanceMode, SessionRevocationReason, SessionRevocationTarget,
        SessionRow,
    },
    throttle_repository::{check_login_throttles, record_login_failure, ThrottleState},
};

const DUMMY_BCRYPT_HASH: &str = "$2b$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy";

#[derive(Clone)]
pub struct SessionServiceContext {
    tenant: TenantContext,
    permission_cache: Arc<PermissionCache>,
    config: Arc<SessionConfig>,
    session_events: broadcast::Sender<SessionRevocationEvent>,
}

impl SessionServiceContext {
    pub fn new(
        tenant: TenantContext,
        permission_cache: Arc<PermissionCache>,
        config: Arc<SessionConfig>,
        session_events: broadcast::Sender<SessionRevocationEvent>,
    ) -> Self {
        Self {
            tenant,
            permission_cache,
            config,
            session_events,
        }
    }

    pub fn tenant(&self) -> &TenantContext {
        &self.tenant
    }

    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    pub fn session_events(&self) -> &broadcast::Sender<SessionRevocationEvent> {
        &self.session_events
    }
}

#[derive(Clone)]
pub struct AuthenticatedSession {
    pub tenant: TenantContext,
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub user_type: String,
}

impl fmt::Debug for AuthenticatedSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedSession")
            .field("tenant_id", &self.tenant.tenant_id)
            .field("session_id", &self.session_id)
            .field("user_id", &self.user_id)
            .field("username", &"[REDACTED]")
            .field("user_type", &self.user_type)
            .finish()
    }
}

pub struct SessionAuthentication {
    pub authenticated: AuthenticatedSession,
    pub csrf_token: CsrfToken,
    pub replacement: Option<SessionCredential>,
}

impl fmt::Debug for SessionAuthentication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionAuthentication")
            .field("authenticated", &self.authenticated)
            .field("csrf_token", &self.csrf_token)
            .field("replacement", &self.replacement)
            .finish()
    }
}

pub struct SessionCredential {
    raw: RawSessionToken,
    pub cookie_max_age_seconds: Option<u64>,
}

impl SessionCredential {
    pub fn encoded(&self) -> EncodedSessionToken {
        self.raw.encode()
    }

    pub fn token_hash(&self) -> TokenHash {
        self.raw.token_hash()
    }
}

impl fmt::Debug for SessionCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionCredential")
            .field("raw", &"[REDACTED]")
            .field("cookie_max_age_seconds", &self.cookie_max_age_seconds)
            .finish()
    }
}

pub struct LoginCommand<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub remember_me: bool,
    pub source: std::net::IpAddr,
    pub user_agent: Option<&'a str>,
    pub now: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoginUserSnapshot {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    pub status: String,
    pub primary_role_name: Option<String>,
    pub profile_image_file_id: Option<Uuid>,
    pub permissions: Vec<String>,
}

pub struct LoginResult {
    pub user: LoginUserSnapshot,
    pub authenticated: AuthenticatedSession,
    pub credential: SessionCredential,
    pub csrf_token: CsrfToken,
}

impl fmt::Debug for LoginResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoginResult")
            .field("user_id", &self.user.id)
            .field("authenticated", &self.authenticated)
            .field("credential", &self.credential)
            .field("csrf_token", &self.csrf_token)
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SessionRevocationResult {
    pub revoked_session_ids: Vec<Uuid>,
    pub current_revoked: bool,
}

#[derive(Debug)]
pub struct PasswordChangeResult {
    pub credential: SessionCredential,
    pub csrf_token: CsrfToken,
    pub revoked_session_ids: Vec<Uuid>,
}

pub async fn login<G>(
    context: &SessionServiceContext,
    command: LoginCommand<'_>,
    generate: G,
) -> Result<LoginResult, AppError>
where
    G: FnOnce() -> Result<RawSessionToken, AppError>,
{
    if let Err(error) = validate_login_input(command.username, command.password) {
        audit::login_rejected(
            context.tenant.tenant_id,
            LoginRejectionReason::InvalidCredentials,
        );
        return Err(error);
    }

    let normalized_username = normalize_login_identifier(command.username);
    let identifier_hash = identifier_bucket(
        context.config.hmac_key(),
        context.tenant.tenant_id,
        &normalized_username,
    );
    let source_hash = source_bucket(
        context.config.hmac_key(),
        context.tenant.tenant_id,
        command.source,
    );

    let initial_throttle = check_login_throttles(
        &context.tenant.pool,
        identifier_hash,
        source_hash,
        command.now,
    )
    .await?;
    if let Some(error) = throttle_error(initial_throttle, command.now) {
        audit::login_rejected(context.tenant.tenant_id, LoginRejectionReason::RateLimited);
        return Err(error);
    }

    let user = find_session_login_user_by_username(&context.tenant.pool, &normalized_username)
        .await
        .map_err(|_| session_store_unavailable())?;
    let password_hash = user
        .as_ref()
        .map(|user| user.password_hash.as_str())
        .unwrap_or(DUMMY_BCRYPT_HASH);
    let password_matches = verify_password(command.password, password_hash).await?;
    let valid_user = user
        .as_ref()
        .is_some_and(|user| password_matches && user.status == "active");

    if !valid_user {
        let throttle = record_login_failure(
            &context.tenant.pool,
            identifier_hash,
            source_hash,
            command.now,
        )
        .await?;
        let blocked = throttle_error(throttle, command.now);
        audit::login_rejected(
            context.tenant.tenant_id,
            if blocked.is_some() {
                LoginRejectionReason::RateLimited
            } else {
                LoginRejectionReason::InvalidCredentials
            },
        );
        return Err(blocked.unwrap_or_else(invalid_login));
    }

    let Some(user) = user else {
        return Err(invalid_login());
    };
    let user_snapshot = load_login_snapshot(context, &user).await?;

    let final_throttle = check_login_throttles(
        &context.tenant.pool,
        identifier_hash,
        source_hash,
        command.now,
    )
    .await?;
    if let Some(error) = throttle_error(final_throttle, command.now) {
        audit::login_rejected(context.tenant.tenant_id, LoginRejectionReason::RateLimited);
        return Err(error);
    }

    let raw = generate()?;
    let lifetime = if command.remember_me {
        SessionLifetime::remembered()
    } else {
        SessionLifetime::normal()
    };
    let session_id = Uuid::new_v4();
    let absolute_expires_at = command.now + lifetime.absolute;
    let new_session = NewSession {
        id: session_id,
        user_id: user.id,
        current_token_hash: raw.token_hash(),
        remember_me: command.remember_me,
        device_label: device_label(command.user_agent),
        times: SessionTimes {
            created_at: command.now,
            last_seen_at: command.now,
            idle_expires_at: command.now + lifetime.idle,
            absolute_expires_at,
            rotated_at: command.now,
        },
    };
    super::session_repository::create_login_session(
        &context.tenant.pool,
        &new_session,
        identifier_hash,
    )
    .await?;

    let authenticated = authenticated(context, session_id, &user);
    let csrf_token = session_csrf_token(
        context.config.hmac_key(),
        context.tenant.tenant_id,
        session_id,
    );
    let credential = credential(raw, command.remember_me, absolute_expires_at, command.now);

    audit::login_succeeded(context.tenant.tenant_id, user.id, session_id);
    audit::session_created(context.tenant.tenant_id, user.id, session_id);
    cleanup_after_operation(&context.tenant.pool, command.now).await;

    Ok(LoginResult {
        user: user_snapshot,
        authenticated,
        credential,
        csrf_token,
    })
}

pub async fn authenticate<G>(
    context: &SessionServiceContext,
    presented_hash: TokenHash,
    now: DateTime<Utc>,
    maintenance: SessionMaintenanceMode,
    generate: G,
) -> Result<Option<SessionAuthentication>, AppError>
where
    G: FnOnce() -> Result<RawSessionToken, AppError>,
{
    let maintained = match authenticate_and_maintain(
        &context.tenant.pool,
        presented_hash,
        now,
        maintenance,
        generate,
    )
    .await
    {
        Ok(maintained) => maintained,
        Err(error) => {
            let reason = match &error {
                AppError::ServiceUnavailable(reason) if reason == "session_rng" => {
                    SessionFailureReason::CredentialGeneration
                }
                _ => SessionFailureReason::SessionStore,
            };
            audit::session_rotation_failed(context.tenant.tenant_id, reason);
            return Err(error);
        }
    };
    let Some(maintained) = maintained else {
        return Ok(None);
    };

    let authenticated = AuthenticatedSession {
        tenant: context.tenant.clone(),
        session_id: maintained.session_id,
        user_id: maintained.user_id,
        username: maintained.username,
        user_type: maintained.user_type,
    };
    let csrf_token = session_csrf_token(
        context.config.hmac_key(),
        context.tenant.tenant_id,
        maintained.session_id,
    );
    let replacement = maintained.replacement.map(|raw| {
        credential(
            raw,
            maintained.remember_me,
            maintained.absolute_expires_at,
            now,
        )
    });

    Ok(Some(SessionAuthentication {
        authenticated,
        csrf_token,
        replacement,
    }))
}

pub async fn load_current_user(
    context: &SessionServiceContext,
    session: &AuthenticatedSession,
) -> Result<LoginUserSnapshot, AppError> {
    ensure_same_tenant(context, session)?;
    let user = find_active_user_shell_by_id(&session.tenant.pool, session.user_id)
        .await
        .map_err(|_| session_store_unavailable())?
        .ok_or_else(authentication_required)?;
    load_active_shell_snapshot(context, user).await
}

pub async fn list_sessions(
    session: &AuthenticatedSession,
    now: DateTime<Utc>,
) -> Result<Vec<SessionRow>, AppError> {
    let sessions = list_user_sessions(&session.tenant.pool, session.user_id, now).await?;
    cleanup_after_operation(&session.tenant.pool, now).await;
    Ok(sessions)
}

pub async fn logout(
    context: &SessionServiceContext,
    session: &AuthenticatedSession,
    now: DateTime<Utc>,
) -> Result<SessionRevocationResult, AppError> {
    ensure_same_tenant(context, session)?;
    revoke(
        context,
        session,
        SessionRevocationTarget::Session(session.session_id),
        SessionRevocationReason::Logout,
        now,
    )
    .await
}

pub async fn revoke_selected(
    context: &SessionServiceContext,
    session: &AuthenticatedSession,
    selected_session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<SessionRevocationResult, AppError> {
    ensure_same_tenant(context, session)?;
    let result = revoke(
        context,
        session,
        SessionRevocationTarget::Session(selected_session_id),
        SessionRevocationReason::UserSelected,
        now,
    )
    .await?;
    if result.revoked_session_ids.is_empty() {
        Err(AppError::NotFound("ไม่พบเซสชัน".to_string()))
    } else {
        Ok(result)
    }
}

pub async fn logout_all(
    context: &SessionServiceContext,
    session: &AuthenticatedSession,
    now: DateTime<Utc>,
) -> Result<SessionRevocationResult, AppError> {
    ensure_same_tenant(context, session)?;
    revoke(
        context,
        session,
        SessionRevocationTarget::User {
            except_session_id: None,
        },
        SessionRevocationReason::LogoutAll,
        now,
    )
    .await
}

pub async fn change_password<G>(
    context: &SessionServiceContext,
    session: &AuthenticatedSession,
    current_password: &str,
    new_password: &str,
    now: DateTime<Utc>,
    generate: G,
) -> Result<PasswordChangeResult, AppError>
where
    G: FnOnce() -> Result<RawSessionToken, AppError>,
{
    ensure_same_tenant(context, session)?;
    validate_new_password(new_password)?;
    let snapshot = load_password_change_snapshot(
        &session.tenant.pool,
        session.user_id,
        session.session_id,
        now,
    )
    .await?;
    if !verify_password(current_password, &snapshot.password_hash).await? {
        return Err(AppError::BadRequest("รหัสผ่านปัจจุบันไม่ถูกต้อง".to_string()));
    }

    let new_password_hash = hash_password(new_password).await?;
    let replacement = generate()?;
    let replacement_hash = replacement.token_hash();
    let mut transaction = session
        .tenant
        .pool
        .begin()
        .await
        .map_err(|_| session_store_unavailable())?;
    let locked =
        lock_password_change(&mut transaction, session.user_id, session.session_id, now).await?;
    if !constant_time_bytes_equal(
        snapshot.password_hash.as_bytes(),
        locked.password_hash.as_bytes(),
    ) {
        transaction
            .rollback()
            .await
            .map_err(|_| session_store_unavailable())?;
        return Err(AppError::Conflict(
            "ข้อมูลรหัสผ่านมีการเปลี่ยนแปลง กรุณาลองใหม่".to_string(),
        ));
    }

    let revoked_session_ids = apply_password_change(
        &mut transaction,
        session.user_id,
        session.session_id,
        &new_password_hash,
        replacement_hash,
        now,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| session_store_unavailable())?;

    if !revoked_session_ids.is_empty() {
        publish(
            &context.session_events,
            SessionRevocationEvent::user(
                &context.tenant.subdomain,
                session.user_id,
                Some(session.session_id),
            ),
        );
        audit::password_sessions_revoked(
            context.tenant.tenant_id,
            session.user_id,
            session.session_id,
        );
        for revoked_session_id in &revoked_session_ids {
            audit::session_revoked(
                context.tenant.tenant_id,
                session.user_id,
                *revoked_session_id,
                SessionRevocationReason::PasswordChanged,
            );
        }
    }

    let csrf_token = session_csrf_token(
        context.config.hmac_key(),
        context.tenant.tenant_id,
        session.session_id,
    );
    Ok(PasswordChangeResult {
        credential: credential(
            replacement,
            locked.remember_me,
            locked.absolute_expires_at,
            now,
        ),
        csrf_token,
        revoked_session_ids,
    })
}

pub async fn revalidate(
    session: &AuthenticatedSession,
    now: DateTime<Utc>,
) -> Result<bool, AppError> {
    revalidate_session(
        &session.tenant.pool,
        session.session_id,
        session.user_id,
        now,
    )
    .await
}

async fn revoke(
    context: &SessionServiceContext,
    session: &AuthenticatedSession,
    target: SessionRevocationTarget,
    reason: SessionRevocationReason,
    now: DateTime<Utc>,
) -> Result<SessionRevocationResult, AppError> {
    let mut revoked_session_ids =
        revoke_sessions(&session.tenant.pool, session.user_id, target, reason, now).await?;
    revoked_session_ids.sort_unstable();
    let current_revoked = revoked_session_ids.contains(&session.session_id);

    if !revoked_session_ids.is_empty() {
        let event = match target {
            SessionRevocationTarget::Session(session_id) => SessionRevocationEvent::session(
                &context.tenant.subdomain,
                session.user_id,
                session_id,
            ),
            SessionRevocationTarget::User { except_session_id } => SessionRevocationEvent::user(
                &context.tenant.subdomain,
                session.user_id,
                except_session_id,
            ),
        };
        publish(&context.session_events, event);
        for revoked_session_id in &revoked_session_ids {
            audit::session_revoked(
                context.tenant.tenant_id,
                session.user_id,
                *revoked_session_id,
                reason,
            );
        }
    }

    Ok(SessionRevocationResult {
        revoked_session_ids,
        current_revoked,
    })
}

async fn load_login_snapshot(
    context: &SessionServiceContext,
    user: &SessionLoginUser,
) -> Result<LoginUserSnapshot, AppError> {
    let primary_role_name = get_primary_role_name(&context.tenant.pool, user.id)
        .await
        .map_err(|_| permission_store_unavailable())?;
    let permissions = get_cached_user_permissions(
        &context.tenant.subdomain,
        user.id,
        &context.tenant.pool,
        &context.permission_cache,
    )
    .await
    .map_err(|_| permission_store_unavailable())?;

    Ok(LoginUserSnapshot {
        id: user.id,
        username: user.username.clone(),
        first_name: user.first_name.clone(),
        last_name: user.last_name.clone(),
        user_type: user.user_type.clone(),
        status: user.status.clone(),
        primary_role_name,
        profile_image_file_id: user.profile_image_file_id,
        permissions,
    })
}

async fn load_active_shell_snapshot(
    context: &SessionServiceContext,
    user: ActiveUserShell,
) -> Result<LoginUserSnapshot, AppError> {
    let primary_role_name = get_primary_role_name(&context.tenant.pool, user.id)
        .await
        .map_err(|_| permission_store_unavailable())?;
    let permissions = get_cached_user_permissions(
        &context.tenant.subdomain,
        user.id,
        &context.tenant.pool,
        &context.permission_cache,
    )
    .await
    .map_err(|_| permission_store_unavailable())?;

    Ok(LoginUserSnapshot {
        id: user.id,
        username: user.username,
        first_name: user.first_name,
        last_name: user.last_name,
        user_type: user.user_type,
        status: user.status,
        primary_role_name,
        profile_image_file_id: user.profile_image_file_id,
        permissions,
    })
}

fn authenticated(
    context: &SessionServiceContext,
    session_id: Uuid,
    user: &SessionLoginUser,
) -> AuthenticatedSession {
    AuthenticatedSession {
        tenant: context.tenant.clone(),
        session_id,
        user_id: user.id,
        username: user.username.clone(),
        user_type: user.user_type.clone(),
    }
}

fn credential(
    raw: RawSessionToken,
    remember_me: bool,
    absolute_expires_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> SessionCredential {
    SessionCredential {
        raw,
        cookie_max_age_seconds: cookie_max_age_seconds(remember_me, absolute_expires_at, now),
    }
}

fn throttle_error(state: ThrottleState, now: DateTime<Utc>) -> Option<AppError> {
    state
        .blocked_until()
        .and_then(|blocked_until| retry_after_seconds(blocked_until, now))
        .map(|retry_after_seconds| AppError::RateLimited {
            retry_after_seconds,
        })
}

async fn verify_password(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let password = Zeroizing::new(password.to_string());
    let password_hash = password_hash.to_string();
    tokio::task::spawn_blocking(move || bcrypt::verify(password.as_bytes(), &password_hash))
        .await
        .map_err(|_| AppError::ServiceUnavailable("password_worker".to_string()))?
        .map_err(|_| AppError::ServiceUnavailable("password_hash".to_string()))
}

async fn hash_password(password: &str) -> Result<String, AppError> {
    let password = Zeroizing::new(password.to_string());
    tokio::task::spawn_blocking(move || {
        bcrypt::non_truncating_hash(password.as_bytes(), bcrypt::DEFAULT_COST)
    })
    .await
    .map_err(|_| AppError::ServiceUnavailable("password_worker".to_string()))?
    .map_err(|_| AppError::ServiceUnavailable("password_hash".to_string()))
}

async fn cleanup_after_operation(pool: &sqlx::PgPool, now: DateTime<Utc>) {
    if cleanup_auth_state(pool, now).await.is_err() {
        audit::cleanup_failed();
    }
}

fn ensure_same_tenant(
    context: &SessionServiceContext,
    session: &AuthenticatedSession,
) -> Result<(), AppError> {
    if context.tenant.tenant_id == session.tenant.tenant_id
        && context.tenant.subdomain == session.tenant.subdomain
    {
        Ok(())
    } else {
        Err(authentication_required())
    }
}

fn constant_time_bytes_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && bool::from(left.ct_eq(right))
}

fn publish(sender: &broadcast::Sender<SessionRevocationEvent>, event: SessionRevocationEvent) {
    match sender.send(event) {
        Ok(_) | Err(_) => {}
    }
}

fn invalid_login() -> AppError {
    AppError::AuthError("ชื่อผู้ใช้หรือรหัสผ่านไม่ถูกต้อง".to_string())
}

fn authentication_required() -> AppError {
    AppError::AuthError("กรุณาเข้าสู่ระบบอีกครั้ง".to_string())
}

fn session_store_unavailable() -> AppError {
    AppError::ServiceUnavailable("session_store".to_string())
}

fn permission_store_unavailable() -> AppError {
    AppError::ServiceUnavailable("permission_store".to_string())
}
