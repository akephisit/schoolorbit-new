use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use shared::auth::validate_token;
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
        Ok(_claims) => {
            // Token valid, proceed to handler
            next.run(request).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Unauthorized - Invalid token"
            })),
        )
            .into_response(),
    }
}
