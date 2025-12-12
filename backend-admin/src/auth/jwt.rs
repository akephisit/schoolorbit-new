use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use std::env;
use super::types::Claims;
use crate::error::AppError;

pub fn generate_token(claims: Claims) -> Result<String, AppError> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key".to_string());
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::InternalServerError(format!("JWT generation failed: {}", e)))?;
    
    Ok(token)
}

pub fn validate_token(token: &str) -> Result<Claims, AppError> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key".to_string());
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;
    
    Ok(token_data.claims)
}
