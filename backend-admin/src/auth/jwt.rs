use super::types::AdminClaims;
use crate::error::AppError;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use std::env;

pub fn generate_token(claims: AdminClaims) -> Result<String, AppError> {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::InternalServerError(format!("JWT generation failed: {}", e)))?;

    Ok(token)
}

pub fn validate_token(token: &str) -> Result<AdminClaims, AppError> {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");
    let token_data = decode::<AdminClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;

    Ok(token_data.claims)
}
