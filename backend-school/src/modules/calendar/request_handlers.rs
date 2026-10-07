use crate::{utils::request_context::actor_tenant_context_from_session, AppState};
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_calendar::{
    models::UpsertCalendarEventRequest,
    requests::{
        self, CalendarEventRequest, CalendarRequestApproval, CalendarRequestPage,
        CalendarRequestQuery, CreateCalendarRequest, PendingCalendarPage, PendingCalendarQuery,
        RejectCalendarRequest,
    },
    target_options::{self, CalendarTargetOptions, CalendarTargetOptionsQuery},
};
use school_http::{ApiErrorResponse, ApiResponse, HttpError as AppError};
use school_permissions::registry::codes;
use uuid::Uuid;

fn require_staff(session: &AuthenticatedSession) -> Result<(), AppError> {
    if session.user_type != "staff" {
        return Err(AppError::Forbidden("คำร้องปฏิทินสำหรับบุคลากรเท่านั้น".into()));
    }
    Ok(())
}

#[utoipa::path(get,path="/api/calendar/requests/calendar",operation_id="listPendingCalendarRequests",tag="calendar",params(PendingCalendarQuery),responses(
    (status=200,description="Pending request dates; managers see the school, requesters see their own. Up to 500 records with explicit overflow.",body=ApiResponse<PendingCalendarPage>),
    (status=400,description="Invalid or excessive date range",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),
    (status=403,description="Staff calendar request access required",body=ApiErrorResponse)
))]
pub async fn list_pending_calendar(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<PendingCalendarQuery>,
) -> Result<impl IntoResponse, AppError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        requests::list_pending_calendar(&context.tenant.pool, &context.actor, query).await?,
    )))
}

#[utoipa::path(get,path="/api/calendar/requests",operation_id="listCalendarRequests",tag="calendar",params(CalendarRequestQuery),responses(
    (status=200,description="Own request history or authorized oldest-first school review queue excluding approved requests",body=ApiResponse<CalendarRequestPage>),
    (status=400,description="Invalid page",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),
    (status=403,description="Request or review access denied",body=ApiErrorResponse)
))]
pub async fn list_requests(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<CalendarRequestQuery>,
) -> Result<impl IntoResponse, AppError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        requests::list_requests(&context.tenant.pool, &context.actor, query).await?,
    )))
}

#[utoipa::path(get,path="/api/calendar/requests/{id}",operation_id="getCalendarRequestForReview",tag="calendar",params(("id"=Uuid,Path,description="Request ID")),responses(
    (status=200,description="Full request details loaded when a calendar manager opens review",body=ApiResponse<CalendarEventRequest>),
    (status=401,description="Authentication required",body=ApiErrorResponse),
    (status=403,description="Staff calendar manager required",body=ApiErrorResponse),
    (status=404,description="Request missing",body=ApiErrorResponse)
))]
pub async fn get_request_for_review(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        requests::get_request_for_review(&context.tenant.pool, &context.actor, id).await?,
    )))
}

#[utoipa::path(post,path="/api/calendar/requests",operation_id="createCalendarRequest",tag="calendar",request_body=CreateCalendarRequest,responses(
    (status=201,description="Pending request submitted; no calendar event created",body=ApiResponse<CalendarEventRequest>),
    (status=400,description="Invalid request",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),
    (status=403,description="Request capability required",body=ApiErrorResponse)
))]
pub async fn create_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateCalendarRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::ok(
            requests::create_request(&context.tenant.pool, &context.actor, payload).await?,
        )),
    ))
}

#[utoipa::path(post,path="/api/calendar/requests/{id}/approve",operation_id="approveCalendarRequest",tag="calendar",params(("id"=Uuid,Path,description="Request ID")),request_body=UpsertCalendarEventRequest,responses(
    (status=200,description="Request approved and one event published atomically",body=ApiResponse<CalendarRequestApproval>),
    (status=400,description="Invalid event configuration",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),
    (status=403,description="Calendar manager required",body=ApiErrorResponse),(status=404,description="Request missing",body=ApiErrorResponse),
    (status=409,description="Request already decided",body=ApiErrorResponse)
))]
pub async fn approve_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpsertCalendarEventRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let notify = payload.notify_audience;
    let outcome =
        requests::approve_request(&context.tenant.pool, &context.actor, id, payload).await?;
    if notify {
        if let Err(error) = super::services::send_event_notification(
            &context.tenant.pool,
            &state.notification_channel,
            &context.tenant.subdomain,
            &outcome.event,
            school_calendar::services::CalendarNotificationKind::Created,
        )
        .await
        {
            tracing::error!(event_id=%outcome.event.id,error=%error,"Calendar notification failed after approval");
        }
    }
    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(post,path="/api/calendar/requests/{id}/reject",operation_id="rejectCalendarRequest",tag="calendar",params(("id"=Uuid,Path,description="Request ID")),request_body=RejectCalendarRequest,responses(
    (status=200,description="Request rejected with reason",body=ApiResponse<CalendarEventRequest>),
    (status=400,description="Reason required",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),
    (status=403,description="Calendar manager required",body=ApiErrorResponse),(status=404,description="Request missing",body=ApiErrorResponse),
    (status=409,description="Request already decided",body=ApiErrorResponse)
))]
pub async fn reject_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RejectCalendarRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        requests::reject_request(&context.tenant.pool, &context.actor, id, &payload.reason).await?,
    )))
}

#[utoipa::path(get,path="/api/calendar/target-options",operation_id="listCalendarTargetOptions",tag="calendar",params(CalendarTargetOptionsQuery),responses(
    (status=200,description="Classroom audience options for the event date",body=ApiResponse<CalendarTargetOptions>),
    (status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Calendar manager required",body=ApiErrorResponse)
))]
pub async fn list_target_options(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<CalendarTargetOptionsQuery>,
) -> Result<impl IntoResponse, AppError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    Ok(Json(ApiResponse::ok(
        target_options::list_target_options(&context.tenant.pool, query.date).await?,
    )))
}
