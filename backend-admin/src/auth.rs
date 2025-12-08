// Authentication utilities

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,       // Subject (user ID)
    pub email: Option<String>,
    pub role: String,
    pub school_id: Option<String>,
    pub exp: i64,          // Expiration time
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum UserRole {
    SuperAdmin,
    SchoolAdmin,
}

impl ToString for UserRole {
    fn to_string(&self) -> String {
        match self {
            UserRole::SuperAdmin => "super_admin".to_string(),
            UserRole::SchoolAdmin => "school_admin".to_string(),
        }
    }
}

pub fn generate_token(user_id: &str, role: &str) -> Result<String, String> {
    let secret = env::var("JWT_SECRET").map_err(|_| "JWT_SECRET not set".to_string())?;
    
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .ok_or("Failed to calculate expiration")?
        .timestamp();

    let claims = Claims {
        sub: user_id.to_string(),
        email: None,
        role: role.to_string(),
        school_id: None,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("Failed to generate token: {}", e))
}

pub fn validate_token(token: &str) -> Result<Claims, String> {
    let secret = env::var("JWT_SECRET").map_err(|_| "JWT_SECRET not set".to_string())?;
    
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| format!("Invalid token: {}", e))
}

pub fn hash_password(password: &str) -> Result<String, String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("Failed to hash password: {}", e))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    bcrypt::verify(password, hash)
        .map_err(|e| format!("Failed to verify password: {}", e))
}
