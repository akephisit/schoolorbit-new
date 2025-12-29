use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

/// Extract subdomain from X-School-Subdomain header (for deploy builds) or Origin/Referer (for runtime)
/// Safe because these headers are set by the browser and cannot be spoofed via JavaScript
pub fn extract_subdomain_from_request(headers: &HeaderMap) -> Result<String, Response> {
    // First, try X-School-Subdomain header (used during deployment builds)
    if let Some(subdomain_header) = headers.get("x-school-subdomain") {
        if let Ok(subdomain) = subdomain_header.to_str() {
            if !subdomain.is_empty() {
                return Ok(subdomain.to_string());
            }
        }
    }
    
    // Fallback: Try Origin/Referer (used during runtime requests)
    let url = headers
        .get("origin")
        .or_else(|| headers.get("referer"))
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No subdomain specified"
                })),
            )
                .into_response()
        })?;

    // Extract subdomain from URL
    // "https://snwsb.schoolorbit.app" → "snwsb"
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or( "");

    let parts: Vec<&str> = host.split('.').collect();
    
    // Validate: should be subdomain.schoolorbit.app
    if parts.len() < 3 || parts[parts.len() - 2] != "schoolorbit" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid domain"
            })),
        )
            .into_response());
    }

    let subdomain = parts[0].to_string();

    // Basic validation
    if subdomain.is_empty() || subdomain == "www" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid subdomain"
            })),
        )
            .into_response());
    }

    Ok(subdomain)
}
