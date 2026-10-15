use crate::{
    policies::behavior_access_policy as policy,
    utils::request_context::actor_tenant_context_from_session, AppState,
};
use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use school_auth::session_service::AuthenticatedSession;
use school_behavior::{
    models::*,
    services::{catalog, cleanup, manual, report as reports, schedules},
};
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, HttpError as AppError};
use school_permissions::registry::codes;
use uuid::Uuid;

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/overview",operation_id="behavior_overview",tag="behavior",responses((status=200,body=ApiResponse<BehaviorOverview>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn overview(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_catalog(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = catalog::overview(pool, policy::access(&context.actor)).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/items",operation_id="behavior_list_items",tag="behavior",params(BehaviorItemsQuery),responses((status=200,body=ApiResponse<Vec<BehaviorItem>>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_items(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(payload): Query<BehaviorItemsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_catalog(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = catalog::list_items(pool, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/items",operation_id="behavior_save_item",tag="behavior",request_body=SaveBehaviorItem,responses((status=200,body=ApiResponse<BehaviorItem>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn save_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SaveBehaviorItem>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_manage(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = catalog::save_item(pool, context.actor.user_id, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/rules",operation_id="behavior_list_rules",tag="behavior",responses((status=200,body=ApiResponse<Vec<BehaviorAttendanceRule>>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_rules(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_manage(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = catalog::list_rules(pool).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/rules",operation_id="behavior_save_rule",tag="behavior",request_body=SaveBehaviorAttendanceRule,responses((status=200,body=ApiResponse<BehaviorAttendanceRule>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn save_rule(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SaveBehaviorAttendanceRule>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_manage(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = catalog::save_rule(pool, context.actor.user_id, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/settings",operation_id="behavior_get_settings",tag="behavior",responses((status=200,body=ApiResponse<BehaviorSettings>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn get_settings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_manage(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = catalog::settings(pool).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/behavior/settings",operation_id="behavior_save_settings",tag="behavior",request_body=SaveBehaviorSettings,responses((status=200,body=ApiResponse<BehaviorSettings>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn save_settings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SaveBehaviorSettings>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_manage(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = catalog::save_settings(pool, payload).await?;
    crate::modules::attendance::runtime::refresh_after_commit(
        &context.tenant.subdomain,
        pool.clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/students",operation_id="behavior_students",tag="behavior",params(BehaviorStudentsQuery),responses((status=200,body=ApiResponse<BehaviorStudentPage>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn students(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(payload): Query<BehaviorStudentsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = &context.tenant.pool;
    let result = manual::students(pool, policy::access(&context.actor), payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/options",operation_id="behavior_selector_options",tag="behavior",params(BehaviorStudentsQuery),responses((status=200,body=ApiResponse<BehaviorSelectorOptions>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn selector_options(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(payload): Query<BehaviorStudentsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = &context.tenant.pool;
    let result = manual::selector_options(
        pool,
        policy::access(&context.actor),
        payload,
        context.actor.has_permission(codes::BEHAVIOR_MANAGE_SCHOOL),
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/batches",operation_id="behavior_create_batch",tag="behavior",request_body=CreateBehaviorBatch,responses((status=200,body=ApiResponse<BehaviorBatchReceipt>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn create_batch(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateBehaviorBatch>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = &context.tenant.pool;
    let result = manual::create_batch(pool, policy::access(&context.actor), payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/entries",operation_id="behavior_report",tag="behavior",params(BehaviorReportQuery),responses((status=200,body=ApiResponse<BehaviorReportPage>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn report(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(payload): Query<BehaviorReportQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = &context.tenant.pool;
    let result = reports::report(pool, policy::access(&context.actor), payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/entries/cancel",operation_id="behavior_cancel_entries",tag="behavior",request_body=CancelBehaviorEntries,responses((status=200,body=ApiResponse<BehaviorCancellation>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn cancel_entries(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CancelBehaviorEntries>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = &context.tenant.pool;
    let result = manual::cancel_entries(pool, policy::access(&context.actor), payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/cycles",operation_id="behavior_cycles",tag="behavior",responses((status=200,body=ApiResponse<Vec<BehaviorCycle>>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn cycles(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_catalog(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = catalog::cycles(pool).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/cleanup/preview",operation_id="behavior_preview_cleanup",tag="behavior",request_body=PreviewBehaviorCleanup,responses((status=200,body=ApiResponse<BehaviorCleanupPreview>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn preview_cleanup(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<PreviewBehaviorCleanup>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_cleanup(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = cleanup::preview(pool, context.actor.user_id, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/cleanup",operation_id="behavior_execute_cleanup",tag="behavior",request_body=ExecuteBehaviorCleanup,responses((status=200,body=ApiResponse<BehaviorCleanupOutcome>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn execute_cleanup(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<ExecuteBehaviorCleanup>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_cleanup(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = cleanup::execute(pool, context.actor.user_id, payload).await?;
    crate::modules::attendance::runtime::refresh_after_commit(
        &context.tenant.subdomain,
        pool.clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/history/adjustments",operation_id="behavior_adjust_history",tag="behavior",request_body=AdjustBehaviorHistory,responses((status=200,body=ApiResponse<BehaviorHistoryAdjustment>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn adjust_history(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<AdjustBehaviorHistory>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_history(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = cleanup::adjust_history(pool, context.actor.user_id, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/reset/preview",operation_id="behavior_preview_reset",tag="behavior",request_body=PreviewBehaviorReset,responses((status=200,body=ApiResponse<BehaviorCleanupPreview>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn preview_reset(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<PreviewBehaviorReset>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_cleanup(&context.actor)?;
    let pool = &context.tenant.pool;
    let result = schedules::preview_reset(pool, context.actor.user_id, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/behavior/reset",operation_id="behavior_reset",tag="behavior",request_body=ResetBehaviorScores,responses((status=200,body=ApiResponse<BehaviorCleanupOutcome>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn reset(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<ResetBehaviorScores>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_cleanup(&context.actor)?;
    let pool = &context.tenant.pool;
    let result =
        crate::behavior_provider_adapter::reset_scores(pool, context.actor.user_id, payload)
            .await?;
    crate::modules::attendance::runtime::refresh_after_commit(
        &context.tenant.subdomain,
        pool.clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/students/{id}/summary",operation_id="behavior_student_summary",tag="behavior",params(("id"=Uuid,Path),BehaviorSummaryQuery),responses((status=200,body=ApiResponse<BehaviorStudentSummary>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse))))]
pub async fn student_summary(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(payload): Query<BehaviorSummaryQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = reports::student_summary(
        &context.tenant.pool,
        policy::access(&context.actor),
        id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/behavior/cleanup/{id}",operation_id="behavior_cleanup_job",tag="behavior",params(("id"=Uuid,Path)),responses((status=200,body=ApiResponse<BehaviorCleanupOutcome>),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse))))]
pub async fn cleanup_job(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    policy::require_cleanup(&context.actor)?;
    let result = cleanup::job(&context.tenant.pool, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/behavior/overview", get(overview))
        .route("/api/behavior/items", get(list_items).post(save_item))
        .route("/api/behavior/rules", get(list_rules).post(save_rule))
        .route(
            "/api/behavior/settings",
            get(get_settings).put(save_settings),
        )
        .route("/api/behavior/students", get(students))
        .route("/api/behavior/options", get(selector_options))
        .route("/api/behavior/batches", post(create_batch))
        .route("/api/behavior/entries", get(report))
        .route("/api/behavior/entries/cancel", post(cancel_entries))
        .route("/api/behavior/cycles", get(cycles))
        .route("/api/behavior/cleanup/preview", post(preview_cleanup))
        .route("/api/behavior/cleanup", post(execute_cleanup))
        .route("/api/behavior/cleanup/{id}", get(cleanup_job))
        .route("/api/behavior/history/adjustments", post(adjust_history))
        .route("/api/behavior/reset/preview", post(preview_reset))
        .route("/api/behavior/reset", post(reset))
        .route("/api/behavior/students/{id}/summary", get(student_summary))
}
