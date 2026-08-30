use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::models::timetable_version::{
    CloneTimetableVersionRequest, ResolveTimetableVersionQuery, TimetableVersionQuery,
};
use crate::modules::academic::services::timetable_version_service;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::permissions::registry::codes;
use crate::policies::learning_offering_access_policy::{
    require_learning_offering_list_access, OfferingAction,
};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/academic/timetable-versions",
    operation_id = "listTimetableVersions",
    params(TimetableVersionQuery),
    responses(
        (status = 200, description = "Timetable versions for the selected term", body = ApiResponse<Vec<crate::modules::academic::models::timetable_version::TimetableVersion>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable read permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn list_versions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<TimetableVersionQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    let versions =
        timetable_version_service::list_versions(&context.tenant.pool, query.academic_term_id)
            .await?;
    Ok(Json(ApiResponse::ok(versions)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/timetable-versions/resolve",
    operation_id = "resolveTimetableVersion",
    params(ResolveTimetableVersionQuery),
    responses(
        (status = 200, description = "Published timetable version effective on the selected date", body = ApiResponse<crate::modules::academic::models::timetable_version::TimetableVersion>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable read permission denied", body = ApiErrorResponse),
        (status = 404, description = "No effective timetable version", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn resolve_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResolveTimetableVersionQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    let version = timetable_version_service::resolve_for_date(
        &context.tenant.pool,
        query.academic_term_id,
        query.date,
    )
    .await?;
    Ok(Json(ApiResponse::ok(version)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-versions/{source_id}/clone",
    operation_id = "cloneTimetableVersion",
    params(("source_id" = Uuid, Path, description = "Published timetable version ID")),
    request_body = CloneTimetableVersionRequest,
    responses(
        (status = 200, description = "Draft timetable version cloned from the published source", body = ApiResponse<crate::modules::academic::models::timetable_version::TimetableVersion>),
        (status = 400, description = "Invalid effective date", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse),
        (status = 404, description = "Source timetable version not found", body = ApiErrorResponse),
        (status = 409, description = "Timetable version conflict", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn clone_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(source_id): Path<Uuid>,
    Json(payload): Json<CloneTimetableVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    let version = timetable_version_service::clone_draft(
        &context.tenant.pool,
        context.actor.user_id,
        source_id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(version)).into_response())
}
