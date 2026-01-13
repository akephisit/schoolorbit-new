use crate::models::auth::Claims;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use std::env;

const DEFAULT_JWT_SECRET: &str = "your-super-secret-jwt-key-change-in-production";
const TOKEN_EXPIRY_DAYS: i64 = 7;

pub struct JwtService;

impl JwtService {
    /// Generate JWT token from claims
    pub fn generate_token(
        user_id: &str,
        username: &str,
        user_type: &str,
    ) -> Result<String, String> {
        let secret = env::var("JWT_SECRET").unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_string());
        
        let now = Utc::now().timestamp();
        let exp = now + (TOKEN_EXPIRY_DAYS * 24 * 60 * 60);

        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            user_type: user_type.to_string(),
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
        let secret = env::var("JWT_SECRET").unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_string());

        decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(|e| format!("Invalid token: {}", e))
    }

    /// Extract token from cookie header
    pub fn extract_token_from_cookie(cookie_header: Option<&str>) -> Option<String> {
        cookie_header.and_then(|cookies| {
            cookies
                .split(';')
                .find_map(|cookie| {
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
