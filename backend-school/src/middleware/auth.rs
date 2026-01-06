use crate::utils::jwt::JwtService;
use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

/// Middleware to verify JWT token and inject user claims
/// Supports both Authorization header (Bearer token) and Cookie
pub async fn auth_middleware(mut req: Request, next: Next) -> Response {
    // Try to extract token from Authorization header first (for cross-origin requests)
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    
    let token_from_header = auth_header
        .and_then(|h| {
            if h.starts_with("Bearer ") {
                Some(h[7..].to_string())
            } else {
                None
            }
        });

    // Fallback to cookie (for same-origin requests)
    let token_from_cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| JwtService::extract_token_from_cookie(Some(cookie)));

    // Use Authorization header first, then cookie
    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "No authentication token found"
                })),
            )
                .into_response();
        }
    };

    // Verify token
    let claims = match JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": format!("Invalid token: {}", e)
                })),
            )
                .into_response();
        }
    };

    // Insert claims into request extensions
    req.extensions_mut().insert(claims);

    next.run(req).await
}

