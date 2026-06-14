use axum::http::HeaderMap;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::workflow::models::{
    WorkflowWindow, WorkflowWindowMetadata, WorkflowWindowStatus,
};
use crate::modules::workflow::services::{
    self, CreateWorkflowWindowInput, WorkflowWindowFilter, WorkflowWindowSchedule,
    WorkflowWindowTimeState,
};
use crate::policies::workflow_access_policy;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkflowWindowsQuery {
    pub module_code: Option<String>,
    pub status: Option<WorkflowWindowStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowWindowRequest {
    pub module_code: String,
    pub workflow_code: String,
    pub title: String,
    pub description: Option<String>,
    pub organization_unit_id: Option<Uuid>,
    pub managed_by_permission: String,
    pub opens_at: Option<chrono::DateTime<chrono::Utc>>,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub closes_at: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: Option<WorkflowWindowMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkflowWindowRequest {
    pub status: WorkflowWindowStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowWindowsData {
    items: Vec<WorkflowWindowResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowWindowResponse {
    id: Uuid,
    module_code: String,
    workflow_code: String,
    title: String,
    description: Option<String>,
    organization_unit_id: Option<Uuid>,
    managed_by_permission: String,
    opens_at: Option<DateTime<Utc>>,
    due_at: Option<DateTime<Utc>>,
    closes_at: Option<DateTime<Utc>>,
    status: WorkflowWindowStatus,
    time_state: WorkflowWindowTimeState,
    metadata: WorkflowWindowMetadata,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl WorkflowWindowResponse {
    fn from_window(window: WorkflowWindow, now: DateTime<Utc>) -> Result<Self, AppError> {
        let status = WorkflowWindowStatus::from_code(&window.status)
            .ok_or_else(|| AppError::InternalServerError("สถานะรอบงานไม่ถูกต้อง".to_string()))?;
        let time_state = services::workflow_window_time_state(
            status,
            window.opens_at,
            window.due_at,
            window.closes_at,
            now,
        );

        Ok(Self {
            id: window.id,
            module_code: window.module_code,
            workflow_code: window.workflow_code,
            title: window.title,
            description: window.description,
            organization_unit_id: window.organization_unit_id,
            managed_by_permission: window.managed_by_permission,
            opens_at: window.opens_at,
            due_at: window.due_at,
            closes_at: window.closes_at,
            status,
            time_state,
            metadata: window.metadata.0,
            created_by: window.created_by,
            created_at: window.created_at,
            updated_at: window.updated_at,
        })
    }
}

pub async fn list_manageable_workflow_windows(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListWorkflowWindowsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let access = workflow_access_policy::resolve_workflow_window_manage_access(&context.actor);
    let items = services::list_manageable_workflow_windows(
        &context.tenant.pool,
        access,
        WorkflowWindowFilter {
            module_code: query.module_code,
            status: query.status,
        },
    )
    .await?;
    let now = Utc::now();
    let items = items
        .into_iter()
        .map(|window| WorkflowWindowResponse::from_window(window, now))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(ApiResponse::ok(WorkflowWindowsData { items })).into_response())
}

pub async fn create_workflow_window(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateWorkflowWindowRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    workflow_access_policy::require_workflow_window_manage_permission(
        &context.actor,
        &payload.managed_by_permission,
    )?;

    let window = services::create_workflow_window(
        &context.tenant.pool,
        CreateWorkflowWindowInput {
            module_code: payload.module_code,
            workflow_code: payload.workflow_code,
            title: payload.title,
            description: payload.description,
            organization_unit_id: payload.organization_unit_id,
            managed_by_permission: payload.managed_by_permission,
            schedule: WorkflowWindowSchedule {
                opens_at: payload.opens_at,
                due_at: payload.due_at,
                closes_at: payload.closes_at,
            },
            metadata: payload.metadata.unwrap_or_default(),
            created_by: Some(context.actor.user_id),
        },
    )
    .await?;
    let window = WorkflowWindowResponse::from_window(window, Utc::now())?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(window))).into_response())
}

pub async fn update_workflow_window(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateWorkflowWindowRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let existing = services::get_workflow_window(&context.tenant.pool, id).await?;
    workflow_access_policy::require_workflow_window_manage_permission(
        &context.actor,
        &existing.managed_by_permission,
    )?;

    let window = match payload.status {
        WorkflowWindowStatus::Open => {
            services::open_workflow_window(&context.tenant.pool, id).await?
        }
        WorkflowWindowStatus::Closed => {
            services::close_workflow_window(&context.tenant.pool, id).await?
        }
        WorkflowWindowStatus::Draft | WorkflowWindowStatus::Archived => {
            services::set_workflow_window_status(&context.tenant.pool, id, payload.status).await?
        }
    };
    let window = WorkflowWindowResponse::from_window(window, Utc::now())?;

    Ok(Json(ApiResponse::ok(window)).into_response())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/me/workflow-windows/manageable",
            get(list_manageable_workflow_windows),
        )
        .route("/workflow-windows", post(create_workflow_window))
        .route("/workflow-windows/{id}", patch(update_workflow_window))
}
