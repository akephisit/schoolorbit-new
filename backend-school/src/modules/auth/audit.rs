use uuid::Uuid;

use super::session_repository::SessionRevocationReason;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginRejectionReason {
    InvalidCredentials,
    RateLimited,
}

impl LoginRejectionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid_credentials",
            Self::RateLimited => "rate_limited",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionFailureReason {
    CredentialGeneration,
    InvalidCsrf,
    InvalidOrigin,
    SessionStore,
}

impl SessionFailureReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::CredentialGeneration => "credential_generation",
            Self::InvalidCsrf => "invalid_csrf",
            Self::InvalidOrigin => "invalid_origin",
            Self::SessionStore => "session_store",
        }
    }
}

pub fn login_rejected(tenant_id: Uuid, reason: LoginRejectionReason) {
    tracing::warn!(
        event = "login_rejected",
        tenant_id = %tenant_id,
        reason = reason.as_str()
    );
}

pub fn login_succeeded(tenant_id: Uuid, user_id: Uuid, session_id: Uuid) {
    tracing::info!(
        event = "login_succeeded",
        tenant_id = %tenant_id,
        user_id = %user_id,
        session_id = %session_id
    );
}

pub fn session_created(tenant_id: Uuid, user_id: Uuid, session_id: Uuid) {
    tracing::info!(
        event = "session_created",
        tenant_id = %tenant_id,
        user_id = %user_id,
        session_id = %session_id
    );
}

pub fn session_revoked(
    tenant_id: Uuid,
    user_id: Uuid,
    session_id: Uuid,
    reason: SessionRevocationReason,
) {
    tracing::info!(
        event = "session_revoked",
        tenant_id = %tenant_id,
        user_id = %user_id,
        session_id = %session_id,
        reason = reason.as_str()
    );
}

pub fn password_sessions_revoked(tenant_id: Uuid, user_id: Uuid, session_id: Uuid) {
    tracing::info!(
        event = "password_sessions_revoked",
        tenant_id = %tenant_id,
        user_id = %user_id,
        session_id = %session_id,
        reason = SessionRevocationReason::PasswordChanged.as_str()
    );
}

pub fn session_rotation_failed(tenant_id: Uuid, reason: SessionFailureReason) {
    tracing::warn!(
        event = "session_rotation_failed",
        tenant_id = %tenant_id,
        reason = reason.as_str()
    );
}

pub fn cleanup_failed() {
    tracing::warn!(reason = "auth_cleanup_failed");
}

pub fn origin_rejected(reason: SessionFailureReason) {
    tracing::warn!(event = "origin_rejected", reason = reason.as_str());
}

pub fn csrf_rejected(tenant_id: Uuid, reason: SessionFailureReason) {
    tracing::warn!(
        event = "csrf_rejected",
        tenant_id = %tenant_id,
        reason = reason.as_str()
    );
}

pub fn session_realtime_disconnect(
    tenant_id: Uuid,
    user_id: Uuid,
    session_id: Uuid,
    reason: SessionFailureReason,
) {
    tracing::info!(
        event = "session_realtime_disconnect",
        tenant_id = %tenant_id,
        user_id = %user_id,
        session_id = %session_id,
        reason = reason.as_str()
    );
}
