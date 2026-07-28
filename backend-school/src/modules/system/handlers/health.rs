use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct LivenessResponse {
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadinessResponse {
    status: &'static str,
    control_plane: &'static str,
    file_platform: &'static str,
    timestamp: String,
}

fn liveness_response(timestamp: String) -> (StatusCode, LivenessResponse) {
    (
        StatusCode::OK,
        LivenessResponse {
            status: "healthy",
            timestamp,
        },
    )
}

fn readiness_response(
    timestamp: String,
    control_plane_result: Result<(), String>,
    file_platform_result: Result<(), String>,
) -> (StatusCode, ReadinessResponse) {
    match (control_plane_result, file_platform_result) {
        (Ok(()), Ok(())) => (
            StatusCode::OK,
            ReadinessResponse {
                status: "ready",
                control_plane: "connected",
                file_platform: "ready",
                timestamp,
            },
        ),
        (control_plane, file_platform) => (
            StatusCode::SERVICE_UNAVAILABLE,
            ReadinessResponse {
                status: "not_ready",
                control_plane: if control_plane.is_ok() {
                    "connected"
                } else {
                    "unavailable"
                },
                file_platform: if file_platform.is_ok() {
                    "ready"
                } else {
                    "unavailable"
                },
                timestamp,
            },
        ),
    }
}

pub async fn health_check() -> impl IntoResponse {
    let (status, response) = liveness_response(chrono::Utc::now().to_rfc3339());
    (status, Json(response))
}

pub async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    let (control_plane, file_platform) = tokio::join!(
        state.admin_client.check_readiness(),
        state.file_platform.check_readiness()
    );
    if let Err(error) = &control_plane {
        tracing::warn!(error = %error, "Backend-school readiness check failed");
    }
    if let Err(error) = &file_platform {
        tracing::warn!(
            error_code = error.log_safe_code(),
            "File Platform readiness check failed"
        );
    }
    let (status, response) = readiness_response(
        chrono::Utc::now().to_rfc3339(),
        control_plane,
        file_platform.map_err(|error| error.log_safe_code().to_string()),
    );
    (status, Json(response))
}

#[cfg(test)]
mod tests {
    use super::{liveness_response, readiness_response};
    use axum::http::StatusCode;

    #[test]
    fn liveness_is_dependency_free() {
        let (status, response) = liveness_response("2026-07-23T00:00:00Z".to_string());

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, "healthy");
    }

    #[test]
    fn available_control_plane_is_ready() {
        let (status, response) =
            readiness_response("2026-07-23T00:00:00Z".to_string(), Ok(()), Ok(()));

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, "ready");
        assert_eq!(response.control_plane, "connected");
    }

    #[test]
    fn unavailable_control_plane_fails_closed_without_internal_error() {
        let (status, response) = readiness_response(
            "2026-07-23T00:00:00Z".to_string(),
            Err("secret internal detail".to_string()),
            Ok(()),
        );

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.status, "not_ready");
        assert_eq!(response.control_plane, "unavailable");
        let json = serde_json::to_value(response).expect("readiness response must serialize");
        assert!(json.get("error").is_none());
        assert!(json.get("controlPlane").is_some());
        assert_eq!(json["filePlatform"], "ready");
    }

    #[test]
    fn unavailable_file_platform_fails_closed_without_internal_error() {
        let (status, response) = readiness_response(
            "2026-07-23T00:00:00Z".to_string(),
            Ok(()),
            Err("secret scanner endpoint".to_string()),
        );

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let json = serde_json::to_value(response).expect("readiness response must serialize");
        assert_eq!(json["controlPlane"], "connected");
        assert_eq!(json["filePlatform"], "unavailable");
        assert!(json.get("error").is_none());
    }
}
