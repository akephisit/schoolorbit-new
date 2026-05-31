use crate::models::{CreateSchool, School, UpdateSchool};
use crate::services::SchoolService;
use crate::types::ApiResponse;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_page() -> i64 {
    1
}

fn default_limit() -> i64 {
    10
}

#[derive(Debug, Serialize)]
pub struct SchoolListResponse {
    pub schools: Vec<School>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub total_pages: i64,
}

// Create school
pub async fn create_school(
    State(state): State<AppState>,
    Json(data): Json<CreateSchool>,
) -> Response {
    let service = SchoolService::new(state.pool.clone());

    match service.create_school(data).await {
        Ok(school) => {
            let response = ApiResponse::success(school);
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// List schools with pagination
pub async fn list_schools(
    State(state): State<AppState>,
    Query(params): Query<PaginationQuery>,
) -> Response {
    let service = SchoolService::new(state.pool.clone());

    match service.list_schools(params.page, params.limit).await {
        Ok((schools, total)) => {
            let total_pages = (total as f64 / params.limit as f64).ceil() as i64;
            let response = ApiResponse::success(SchoolListResponse {
                schools,
                total,
                page: params.page,
                limit: params.limit,
                total_pages,
            });
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// Get school by ID
pub async fn get_school(State(state): State<AppState>, Path(id): Path<Uuid>) -> Response {
    let service = SchoolService::new(state.pool.clone());

    match service.get_school(id).await {
        Ok(school) => {
            let response = ApiResponse::success(school);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "School not found"})),
        )
            .into_response(),
    }
}

// Update school
pub async fn update_school(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(data): Json<UpdateSchool>,
) -> Response {
    let service = SchoolService::new(state.pool.clone());

    match service.update_school(id, data).await {
        Ok(school) => {
            let response = ApiResponse::success(school);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// Delete school
pub async fn delete_school(State(state): State<AppState>, Path(id): Path<Uuid>) -> Response {
    let service = SchoolService::new(state.pool.clone());

    match service.delete_school(id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "message": "School deleted"})),
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "School not found"})),
        )
            .into_response(),
    }
}

// Deploy/Redeploy school frontend
pub async fn deploy_school(State(state): State<AppState>, Path(id): Path<Uuid>) -> Response {
    let service = SchoolService::new(state.pool.clone());

    match service.deploy_school(id).await {
        Ok(result) => {
            let response = ApiResponse::success(result);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct BulkDeployRequest {
    pub school_ids: Vec<Uuid>,
}

// Bulk deploy multiple schools
pub async fn bulk_deploy_schools(
    State(state): State<AppState>,
    Json(data): Json<BulkDeployRequest>,
) -> Response {
    let service = SchoolService::new(state.pool.clone());

    match service.bulk_deploy_schools(data.school_ids).await {
        Ok(results) => {
            let response = ApiResponse::success(results);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// Get deployment history for a school
pub async fn get_deployment_history(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    let service = SchoolService::new(state.pool.clone());

    match service.get_deployment_history(id).await {
        Ok(history) => {
            let response = ApiResponse::success(history);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
