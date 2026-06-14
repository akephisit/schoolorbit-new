use axum::http::HeaderMap;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::work::services::{
    self, CreateWorkItemInput, WorkItem, WorkItemAssigneeTargetInput, WorkItemFilter,
    WorkItemMetadata, WorkItemState,
};
use crate::modules::workflow;
use crate::policies::workflow_access_policy;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkItemsQuery {
    pub module_code: Option<String>,
    pub state: Option<WorkItemState>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkItemRequest {
    pub workflow_window_id: Uuid,
    pub module_code: String,
    pub source_resource_type: String,
    pub source_resource_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub action_path: String,
    pub required_permission: Option<String>,
    pub metadata: Option<WorkItemMetadata>,
    pub assignees: Vec<WorkItemAssigneeTargetInput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkItemsData {
    items: Vec<WorkItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkItemIdData {
    id: Uuid,
}

pub async fn list_my_work_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListWorkItemsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let items = services::list_my_work_items(
        &context.tenant.pool,
        context.actor.user_id,
        WorkItemFilter {
            module_code: query.module_code,
            state: query.state,
        },
    )
    .await?;

    Ok(Json(ApiResponse::ok(WorkItemsData { items })).into_response())
}

pub async fn get_my_work_counts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let counts = services::get_my_work_counts(&context.tenant.pool, context.actor.user_id).await?;

    Ok(Json(ApiResponse::ok(counts)).into_response())
}

pub async fn create_work_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateWorkItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let window =
        workflow::services::get_workflow_window(&context.tenant.pool, payload.workflow_window_id)
            .await?;
    workflow_access_policy::require_workflow_window_manage_permission(
        &context.actor,
        &window.managed_by_permission,
    )?;

    let id = services::create_work_item(
        &context.tenant.pool,
        CreateWorkItemInput {
            workflow_window_id: payload.workflow_window_id,
            module_code: payload.module_code,
            source_resource_type: payload.source_resource_type,
            source_resource_id: payload.source_resource_id,
            title: payload.title,
            description: payload.description,
            action_path: payload.action_path,
            required_permission: payload.required_permission,
            metadata: payload.metadata.unwrap_or_default(),
            assignees: payload.assignees,
            created_by: Some(context.actor.user_id),
        },
    )
    .await?;
    state.notify_work_items_changed();

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::ok(WorkItemIdData { id })),
    )
        .into_response())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/work-items", get(list_my_work_items))
        .route("/me/work-items/counts", get(get_my_work_counts))
        .route("/work-items", post(create_work_item))
}
