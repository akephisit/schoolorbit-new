use crate::models::auth::Claims;
use crate::utils::jwt::JwtService;
use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

/// Middleware to verify JWT token and inject user claims
pub async fn auth_middleware(mut req: Request, next: Next) -> Response {
    // Extract token from cookie
    let cookie_header = req
        .headers()
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok());

    let token = match JwtService::extract_token_from_cookie(cookie_header) {
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

/// Extract claims from request extensions (use in handlers)
pub fn extract_claims(req: &Request) -> Option<Claims> {
    req.extensions().get::<Claims>().cloned()
}
