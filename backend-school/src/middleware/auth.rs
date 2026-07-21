use crate::error::AppError;
use crate::modules::auth::models::User;
use crate::utils::field_encryption;
use crate::utils::jwt::authenticate_request;
use axum::{
    extract::Request,
    http::HeaderMap,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Middleware to verify JWT token and inject user claims
/// Supports both Authorization header (Bearer token) and Cookie
pub async fn auth_middleware(mut req: Request, next: Next) -> Response {
    let authenticated = match authenticate_request(req.headers()) {
        Ok(authenticated) => authenticated,
        Err(error) => return error.into_response(),
    };

    // Insert claims into request extensions
    req.extensions_mut().insert(authenticated.claims);

    next.run(req).await
}

/// Helper function to extract user ID from request headers
pub async fn extract_user_id(
    headers: &HeaderMap,
    _pool: &sqlx::PgPool,
) -> Result<uuid::Uuid, String> {
    authenticate_request(headers)
        .map(|authenticated| authenticated.user_id)
        .map_err(|_| "Invalid user authentication".to_string())
}

/// Helper function to get current user info with database query
pub async fn get_current_user(headers: &HeaderMap, pool: &sqlx::PgPool) -> Result<User, AppError> {
    let user_id = extract_user_id(headers, pool)
        .await
        .map_err(AppError::AuthError)?;

    let mut user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching user: {}", e);
            AppError::InternalServerError("Database error".to_string())
        })?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    // Decrypt sensitive fields
    if let Some(nid) = &user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    Ok(user)
}
