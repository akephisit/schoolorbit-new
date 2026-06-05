use axum::{
    body::Body,
    http::{HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use std::env;
use subtle::ConstantTimeEq;

use crate::error::AppError;

pub const INTERNAL_SECRET_HEADER: &str = "X-Internal-Secret";
pub const INTERNAL_CALLER_HEADER: &str = "X-Internal-Caller";

/// Middleware to validate internal API requests using X-Internal-Secret header.
pub async fn validate_internal_secret(
    req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    if verify_internal_secret(req.headers())? {
        Ok(next.run(req).await)
    } else {
        Err(unauthorized_response())
    }
}

fn verify_internal_secret(headers: &HeaderMap) -> Result<bool, AppError> {
    let expected_secret = expected_internal_secret(headers)?;
    let provided_secret = headers
        .get(INTERNAL_SECRET_HEADER)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(unauthorized_response)?;

    Ok(secrets_match(provided_secret, &expected_secret))
}

fn expected_internal_secret(headers: &HeaderMap) -> Result<String, AppError> {
    if let Some(caller_secret) = internal_caller(headers).and_then(secret_for_caller) {
        return Ok(caller_secret);
    }

    env::var("INTERNAL_API_SECRET")
        .map_err(|_| AppError::ConfigError("INTERNAL_API_SECRET is not configured".to_string()))
}

fn internal_caller(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(INTERNAL_CALLER_HEADER)
        .and_then(|h| h.to_str().ok())
        .map(str::trim)
        .filter(|caller| !caller.is_empty())
}

fn secret_for_caller(caller: &str) -> Option<String> {
    let env_key = format!(
        "INTERNAL_API_SECRET_{}",
        caller.replace('-', "_").to_ascii_uppercase()
    );
    env::var(env_key).ok().filter(|secret| !secret.is_empty())
}

fn secrets_match(provided: &str, expected: &str) -> bool {
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

fn unauthorized_response() -> AppError {
    AppError::AuthError("Unauthorized - Invalid or missing internal secret".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn headers(secret: &str, caller: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            INTERNAL_SECRET_HEADER,
            HeaderValue::from_str(secret).unwrap(),
        );
        if let Some(caller) = caller {
            headers.insert(
                INTERNAL_CALLER_HEADER,
                HeaderValue::from_str(caller).unwrap(),
            );
        }
        headers
    }

    #[test]
    fn verifies_fallback_internal_secret() {
        let _guard = env_lock();
        env::set_var("INTERNAL_API_SECRET", "shared-secret");
        env::remove_var("INTERNAL_API_SECRET_BACKEND_ADMIN");

        assert!(verify_internal_secret(&headers("shared-secret", None)).unwrap());
        assert!(!verify_internal_secret(&headers("wrong-secret", None)).unwrap());
    }

    #[test]
    fn caller_secret_overrides_shared_secret() {
        let _guard = env_lock();
        env::set_var("INTERNAL_API_SECRET", "shared-secret");
        env::set_var("INTERNAL_API_SECRET_BACKEND_ADMIN", "admin-secret");

        assert!(verify_internal_secret(&headers("admin-secret", Some("backend-admin"))).unwrap());
        assert!(!verify_internal_secret(&headers("shared-secret", Some("backend-admin"))).unwrap());
    }

    #[test]
    fn caller_secret_falls_back_to_shared_secret_when_unset() {
        let _guard = env_lock();
        env::set_var("INTERNAL_API_SECRET", "shared-secret");
        env::remove_var("INTERNAL_API_SECRET_BACKEND_ADMIN");

        assert!(verify_internal_secret(&headers("shared-secret", Some("backend-admin"))).unwrap());
    }
}
