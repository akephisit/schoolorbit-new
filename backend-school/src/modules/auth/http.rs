use axum::{
    http::{header, HeaderMap, HeaderValue},
    response::Response,
};

use crate::error::AppError;

use super::{
    config::{CSRF_HEADER_NAME, LEGACY_COOKIE_NAME, SESSION_COOKIE_NAME},
    session_crypto::{CsrfToken, RawSessionToken},
};

const SESSION_EXPIRY_FALLBACK: &str =
    "__Host-schoolorbit_session=; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=0";

pub fn set_session_cookie(encoded: &str, max_age_seconds: Option<u64>) -> HeaderValue {
    if !encoded
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return HeaderValue::from_static(SESSION_EXPIRY_FALLBACK);
    }

    let mut value =
        format!("{SESSION_COOKIE_NAME}={encoded}; HttpOnly; SameSite=Lax; Secure; Path=/");
    if let Some(seconds) = max_age_seconds {
        value.push_str(&format!("; Max-Age={seconds}"));
    }

    HeaderValue::from_str(&value)
        .unwrap_or_else(|_| HeaderValue::from_static(SESSION_EXPIRY_FALLBACK))
}

pub fn expire_auth_cookies() -> Vec<HeaderValue> {
    [SESSION_COOKIE_NAME, LEGACY_COOKIE_NAME]
        .into_iter()
        .filter_map(|name| {
            HeaderValue::from_str(&format!(
                "{name}=; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=0"
            ))
            .ok()
        })
        .collect()
}

pub fn expire_legacy_cookie() -> HeaderValue {
    HeaderValue::from_static("auth_token=; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=0")
}

pub fn append_response_cookie(response: &mut Response, cookie: HeaderValue) {
    response.headers_mut().append(header::SET_COOKIE, cookie);
}

pub fn append_expired_auth_cookies(response: &mut Response) {
    for cookie in expire_auth_cookies() {
        append_response_cookie(response, cookie);
    }
}

pub fn csrf_response_header(token: &CsrfToken) -> HeaderValue {
    HeaderValue::from_str(&token.expose_for_header())
        .unwrap_or_else(|_| HeaderValue::from_static("invalid"))
}

pub fn presented_session_token(headers: &HeaderMap) -> Result<Option<RawSessionToken>, AppError> {
    let mut presented = None;
    let mut matching_names = 0_u8;

    for header_value in headers.get_all(header::COOKIE).iter() {
        let Ok(raw_header) = header_value.to_str() else {
            return Err(authentication_required());
        };

        for pair in raw_header.split(';') {
            let pair = pair.trim();
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            if name.trim() != SESSION_COOKIE_NAME {
                continue;
            }

            matching_names = matching_names.saturating_add(1);
            if matching_names > 1 {
                return Err(authentication_required());
            }
            if name != SESSION_COOKIE_NAME || value.trim() != value {
                presented = None;
                continue;
            }
            presented = RawSessionToken::parse(value).ok();
        }
    }

    Ok(presented)
}

pub fn validate_csrf(headers: &HeaderMap, expected: &CsrfToken) -> Result<(), AppError> {
    let mut values = headers.get_all(CSRF_HEADER_NAME).iter();
    let Some(value) = values.next() else {
        return Err(csrf_rejected());
    };
    if values.next().is_some() {
        return Err(csrf_rejected());
    }
    let value = value.to_str().map_err(|_| csrf_rejected())?;
    let presented = CsrfToken::parse(value).map_err(|_| csrf_rejected())?;
    if &presented == expected {
        Ok(())
    } else {
        Err(csrf_rejected())
    }
}

fn authentication_required() -> AppError {
    AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string())
}

fn csrf_rejected() -> AppError {
    AppError::Forbidden("csrf_rejected".to_string())
}

#[cfg(test)]
mod tests {
    use axum::http::{header, HeaderMap, HeaderValue};

    use super::presented_session_token;

    #[test]
    fn malformed_session_cookie_is_treated_as_absent() {
        let headers = HeaderMap::from_iter([(
            header::COOKIE,
            HeaderValue::from_static("__Host-schoolorbit_session=not-an-opaque-token"),
        )]);

        assert!(presented_session_token(&headers)
            .expect("one malformed cookie must not panic")
            .is_none());
    }
}
