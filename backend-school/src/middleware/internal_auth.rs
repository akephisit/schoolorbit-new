use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::env;

/// Middleware to validate internal API requests using X-Internal-Secret header
pub async fn validate_internal_secret(
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let expected_secret = env::var("INTERNAL_API_SECRET")
        .expect("INTERNAL_API_SECRET must be set");

    let secret_header = req
        .headers()
        .get("X-Internal-Secret")
        .and_then(|h| h.to_str().ok());

    match secret_header {
        Some(secret) if secret == expected_secret => {
            Ok(next.run(req).await)
        }
        _ => {
            let error = serde_json::json!({
                "error": "Unauthorized - Invalid or missing internal secret"
            });
            Err((StatusCode::UNAUTHORIZED, Json(error)).into_response())
        }
    }
}

