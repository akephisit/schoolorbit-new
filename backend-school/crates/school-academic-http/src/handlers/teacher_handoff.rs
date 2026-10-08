use crate::delivery::handlers::require_term_change_set_access;
use crate::policies::learning_offering_access_policy::OfferingAction;
use crate::state::AcademicHttpState;
use axum::{
    extract::{Extension, Path, State},
    response::{IntoResponse, Response},
    Json,
};
use school_academic_delivery::models::{
    ApplyTeacherHandoffRequest, ApplyTeacherHandoffResponse, PreviewTeacherHandoffRequest,
    TeacherHandoffPreview,
};
use school_academic_timetable::services::teacher_handoff;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
use school_http::{ApiErrorResponse, ApiResponse, HttpError as AppError};
use uuid::Uuid;
fn ok<T: serde::Serialize>(data: T) -> Response {
    Json(ApiResponse::ok(data)).into_response()
}

#[utoipa::path(
    post,
    path = "/api/academic/teacher-handoffs/{id}/preview",
    operation_id = "previewTeacherHandoff",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Published opening revision ID")),
    request_body = PreviewTeacherHandoffRequest,
    responses(
        (status = 200, description = "Teacher timetable handoff preview", body = ApiResponse<TeacherHandoffPreview>),
        (status = 400, description = "Invalid teacher handoff", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Teacher change item not found", body = ApiErrorResponse),
        (status = 409, description = "Teacher handoff conflict", body = ApiErrorResponse)
    )
)]
pub async fn preview_teacher_handoff(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PreviewTeacherHandoffRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    school_academic_timetable::policy::require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        school_academic_timetable::policy::TimetableAction::Manage,
        &school_academic_timetable::policy::TimetableResourceSet {
            timetable_version_ids: vec![request.timetable_version_id],
            requires_school_scope: true,
            ..Default::default()
        },
    )
    .await?;
    Ok(ok(teacher_handoff::preview(
        &context.tenant.pool,
        id,
        request,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/teacher-handoffs/{id}/apply",
    operation_id = "applyTeacherHandoff",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Published opening revision ID")),
    request_body = ApplyTeacherHandoffRequest,
    responses(
        (status = 200, description = "Teacher timetable handoff applied", body = ApiResponse<ApplyTeacherHandoffResponse>),
        (status = 400, description = "Invalid teacher handoff", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Teacher change item not found", body = ApiErrorResponse),
        (status = 409, description = "Teacher handoff conflict or stale preview", body = ApiErrorResponse)
    )
)]
pub async fn apply_teacher_handoff(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ApplyTeacherHandoffRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    school_academic_timetable::policy::require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        school_academic_timetable::policy::TimetableAction::Manage,
        &school_academic_timetable::policy::TimetableResourceSet {
            timetable_version_ids: vec![request.timetable_version_id],
            requires_school_scope: true,
            ..Default::default()
        },
    )
    .await?;
    let outcome =
        teacher_handoff::apply(&context.tenant.pool, context.actor.user_id, id, request).await?;
    for entry in &outcome.response.handoff.proposed_entries {
        state.websocket_manager.broadcast_mutation(
            session.tenant.subdomain.clone(),
            outcome.academic_term_id,
            crate::realtime::TimetableEvent::TimetableChanged {
                user_id: context.actor.user_id,
                academic_term_id: outcome.academic_term_id,
                timetable_version_id: outcome.response.handoff.target_timetable_version_id,
                block_id: None,
                revision: entry.row_version + 1,
            },
        );
    }
    Ok(ok(outcome.response))
}
