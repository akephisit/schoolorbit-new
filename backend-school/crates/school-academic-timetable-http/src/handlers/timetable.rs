#[cfg(feature = "api-docs")]
use school_academic_timetable::models::timetable::Timetable;
use crate::state::AcademicTimetableHttpState;
use axum::{
    extract::{Extension, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use school_academic_realtime::TimetableEvent;
use school_academic_timetable::{
    models::timetable::{CreateTimetableRequest, TimetableQuery},
    policy::{require_timetable_list_access, TimetableAction},
    services::timetable_service,
};
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, HttpError as AppError};
use school_permissions::registry::codes;

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/timetable",operation_id="getTimetable",params(TimetableQuery),responses((status=200,description="Current timetable for the explicit term, or null when not created",body=ApiResponse<Option<Timetable>>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Timetable read permission denied",body=ApiErrorResponse)),tag="academic"))]
pub async fn get_timetable(
    State(state): State<AcademicTimetableHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<TimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    require_timetable_list_access(&context.tenant.pool, &context.actor, TimetableAction::Read)
        .await?;
    Ok(Json(ApiResponse::ok(
        timetable_service::get_for_term(&context.tenant.pool, query.academic_term_id).await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/timetable",operation_id="createTimetable",params(crate::mutation::TimetableMutationHeaders),request_body=CreateTimetableRequest,responses((status=200,description="Current timetable created or existing singleton returned",body=ApiResponse<Timetable>),(status=400,description="Invalid term or bell schedule",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="School-wide timetable management required",body=ApiErrorResponse),(status=409,description="Term is not writable",body=ApiErrorResponse)),tag="academic"))]
pub async fn create_timetable(
    State(state): State<AcademicTimetableHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(request): Json<CreateTimetableRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL)?;
    let mutation = crate::mutation::from_session(&state, &headers, &session, &context)?;
    let current = timetable_service::create(&context.tenant.pool, &mutation, request).await?;
    state.websocket_manager.broadcast_mutation(
        context.tenant.subdomain.clone(),
        current.academic_term_id,
        TimetableEvent::TimetableChanged {
            user_id: context.actor.user_id,
            academic_term_id: current.academic_term_id,
            timetable_id: current.id,
            block_id: None,
            revision: current.row_version,
        },
    );
    Ok(Json(ApiResponse::ok(current)))
}
