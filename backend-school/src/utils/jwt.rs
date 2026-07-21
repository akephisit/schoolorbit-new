use crate::error::AppError;
use crate::modules::auth::models::Claims;
use crate::utils::subdomain::extract_subdomain_from_request;
use axum::http::{header, HeaderMap};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use std::env;
use uuid::Uuid;

const TOKEN_EXPIRY_DAYS: i64 = 7;
pub const JWT_ISSUER: &str = "schoolorbit-backend-school";
pub const JWT_AUDIENCE: &str = "schoolorbit-school-app";
pub const TOKEN_VERSION: u8 = 1;

#[derive(Clone, Debug)]
pub struct AuthenticatedRequest {
    pub claims: Claims,
    pub user_id: Uuid,
    pub tenant: String,
}

pub fn extract_token_from_headers(headers: &HeaderMap) -> Result<String, AppError> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned);
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| JwtService::extract_token_from_cookie(Some(value)));
    bearer
        .or(cookie)
        .ok_or_else(|| AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))
}

pub fn authenticate_request(headers: &HeaderMap) -> Result<AuthenticatedRequest, AppError> {
    let token = extract_token_from_headers(headers)?;
    let claims = JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;
    let tenant = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;
    if claims.tenant != tenant {
        return Err(AppError::AuthError("Token ไม่ถูกต้อง".to_string()));
    }
    Ok(AuthenticatedRequest {
        claims,
        user_id,
        tenant,
    })
}

pub fn authenticate_for_tenant(
    headers: &HeaderMap,
    expected_tenant: &str,
) -> Result<AuthenticatedRequest, AppError> {
    let authenticated = authenticate_request(headers)?;
    if authenticated.tenant != expected_tenant {
        return Err(AppError::AuthError("Token ไม่ถูกต้อง".to_string()));
    }
    Ok(authenticated)
}

pub struct JwtService;

impl JwtService {
    /// Generate JWT token from claims
    pub fn generate_token(
        user_id: &str,
        username: &str,
        user_type: &str,
        tenant: &str,
    ) -> Result<String, String> {
        let secret = env::var("JWT_SECRET")
            .map_err(|_| "JWT_SECRET environment variable must be set".to_string())?;

        let now = Utc::now().timestamp();
        let exp = now + (TOKEN_EXPIRY_DAYS * 24 * 60 * 60);

        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            user_type: user_type.to_string(),
            tenant: tenant.to_string(),
            iss: JWT_ISSUER.to_string(),
            aud: JWT_AUDIENCE.to_string(),
            token_version: TOKEN_VERSION,
            exp,
            iat: now,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|e| format!("Failed to generate token: {}", e))
    }

    /// Verify and decode JWT token
    pub fn verify_token(token: &str) -> Result<Claims, String> {
        let secret = env::var("JWT_SECRET")
            .map_err(|_| "JWT_SECRET environment variable must be set".to_string())?;

        let mut validation = Validation::default();
        validation.leeway = 0;
        validation.set_issuer(&[JWT_ISSUER]);
        validation.set_audience(&[JWT_AUDIENCE]);
        validation.set_required_spec_claims(&["exp", "sub", "iss", "aud"]);
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        )
        .map_err(|_| "Invalid token".to_string())?
        .claims;
        if claims.token_version != TOKEN_VERSION {
            return Err("Invalid token version".to_string());
        }
        Ok(claims)
    }

    /// Extract token from cookie header
    pub fn extract_token_from_cookie(cookie_header: Option<&str>) -> Option<String> {
        cookie_header.and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
                if parts.len() == 2 && parts[0] == "auth_token" {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header, HeaderMap, HeaderValue};

    fn configure_secret() {
        std::env::set_var("JWT_SECRET", "test-only-secret-at-least-32-bytes");
    }

    #[test]
    fn generated_token_has_strict_tenant_contract() {
        configure_secret();
        let token = JwtService::generate_token(
            "8b391685-4a1c-4f25-a544-b1c5bd0d457e",
            "teacher.one",
            "staff",
            "tenant-a",
        )
        .unwrap();
        let claims = JwtService::verify_token(&token).unwrap();
        assert_eq!(claims.tenant, "tenant-a");
        assert_eq!(claims.iss, JWT_ISSUER);
        assert_eq!(claims.aud, JWT_AUDIENCE);
        assert_eq!(claims.token_version, TOKEN_VERSION);
    }

    #[test]
    fn bearer_token_takes_precedence_over_cookie() {
        configure_secret();
        let bearer = JwtService::generate_token(
            "8b391685-4a1c-4f25-a544-b1c5bd0d457e",
            "bearer",
            "staff",
            "tenant-a",
        )
        .unwrap();
        let cookie = JwtService::generate_token(
            "eb22ab8e-4382-4ddb-bcbb-8833b788e362",
            "cookie",
            "staff",
            "tenant-a",
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {bearer}")).unwrap(),
        );
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("auth_token={cookie}")).unwrap(),
        );
        assert_eq!(extract_token_from_headers(&headers).unwrap(), bearer);
    }

    fn encode_claims(claims: Claims) -> String {
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"test-only-secret-at-least-32-bytes"),
        )
        .unwrap()
    }

    fn strict_claims() -> Claims {
        let now = Utc::now().timestamp();
        Claims {
            sub: "8b391685-4a1c-4f25-a544-b1c5bd0d457e".into(),
            username: "teacher.one".into(),
            user_type: "staff".into(),
            tenant: "tenant-a".into(),
            iss: JWT_ISSUER.into(),
            aud: JWT_AUDIENCE.into(),
            token_version: TOKEN_VERSION,
            exp: now + 300,
            iat: now,
        }
    }

    #[test]
    fn wrong_registered_claims_version_and_expiry_are_rejected() {
        configure_secret();
        let mut wrong_issuer = strict_claims();
        wrong_issuer.iss = "other-service".into();
        assert!(JwtService::verify_token(&encode_claims(wrong_issuer)).is_err());

        let mut wrong_audience = strict_claims();
        wrong_audience.aud = "other-app".into();
        assert!(JwtService::verify_token(&encode_claims(wrong_audience)).is_err());

        let mut wrong_version = strict_claims();
        wrong_version.token_version = 2;
        assert!(JwtService::verify_token(&encode_claims(wrong_version)).is_err());

        let mut expired = strict_claims();
        expired.exp = Utc::now().timestamp() - 1;
        assert!(JwtService::verify_token(&encode_claims(expired)).is_err());

        let mut missing_issuer = serde_json::to_value(strict_claims()).unwrap();
        missing_issuer.as_object_mut().unwrap().remove("iss");
        let token = encode(
            &Header::default(),
            &missing_issuer,
            &EncodingKey::from_secret(b"test-only-secret-at-least-32-bytes"),
        )
        .unwrap();
        assert!(JwtService::verify_token(&token).is_err());
    }

    #[test]
    fn request_tenant_must_match_token_tenant() {
        configure_secret();
        let token = encode_claims(strict_claims());
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        headers.insert("x-school-subdomain", HeaderValue::from_static("tenant-b"));
        assert!(matches!(
            authenticate_request(&headers),
            Err(AppError::AuthError(_))
        ));
    }
}
