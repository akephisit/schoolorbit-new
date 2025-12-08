use crate::models::{CreateSchool, School, UpdateSchool};
use crate::services::SchoolService;
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use shared::types::ApiResponse;
use sqlx::PgPool;
use std::sync::OnceLock;
use uuid::Uuid;

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

pub fn init_pool(pool: PgPool) {
    DB_POOL.set(pool).ok();
}

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
pub async fn create_school(Json(data): Json<CreateSchool>) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database not initialized"})),
            )
                .into_response();
        }
    };

    let service = SchoolService::new(pool.clone());

    match service.create_school(data).await {
        Ok(school) => {
            // Trigger auto-deployment in background
            let school_clone = school.clone();
            let pool_clone = pool.clone();
            
            tokio::spawn(async move {
                use crate::services::DeploymentService;
                
                match DeploymentService::new(pool_clone) {
                    Ok(deployment_service) => {
                        if let Err(e) = deployment_service.deploy_school(&school_clone).await {
                            eprintln!("⚠️  Auto-deployment failed for {}: {}", school_clone.name, e);
                            eprintln!("   You may need to deploy manually");
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  Deployment service initialization failed: {}", e);
                        eprintln!("   Auto-deployment disabled. Deploy manually.");
                    }
                }
            });

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
pub async fn list_schools(Query(params): Query<PaginationQuery>) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database not initialized"})),
            )
                .into_response();
        }
    };

    let service = SchoolService::new(pool);

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
pub async fn get_school(Path(id): Path<Uuid>) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database not initialized"})),
            )
                .into_response();
        }
    };

    let service = SchoolService::new(pool);

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
pub async fn update_school(Path(id): Path<Uuid>, Json(data): Json<UpdateSchool>) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database not initialized"})),
            )
                .into_response();
        }
    };

    let service = SchoolService::new(pool);

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
pub async fn delete_school(Path(id): Path<Uuid>) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database not initialized"})),
            )
                .into_response();
        }
    };

    let service = SchoolService::new(pool);

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
