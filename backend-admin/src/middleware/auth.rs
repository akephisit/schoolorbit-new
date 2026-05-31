use crate::auth::validate_token;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use tower_cookies::Cookies;

pub async fn require_auth(cookies: Cookies, request: Request, next: Next) -> Response {
    // Get auth_token from cookies
    let token = match cookies.get("auth_token") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Unauthorized - No auth token"
                })),
            )
                .into_response();
        }
    };

    // Validate token
    match validate_token(&token) {
        Ok(claims) if claims.role.can_access_admin_backend() => next.run(request).await,
        Ok(_) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Forbidden - Admin role required"
            })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Unauthorized - Invalid token"
            })),
        )
            .into_response(),
    }
}
