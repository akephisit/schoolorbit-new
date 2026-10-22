use crate::http::state::AttendanceHttpState;
use crate::{application as services, runtime};
use crate::{
    models::*,
    services::{faces, reports, sessions, settings, specials},
};
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, EmptyData, HttpError as AppError};
use school_permissions::registry::codes;
use uuid::Uuid;
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/settings/{term}",operation_id="attendance_get_settings",tag="attendance",params(("term"=Uuid,Path)),responses((status=200, description = "Attendance settings returned",body=ApiResponse<AttendanceSettings>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn get_settings(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
) -> Result<Json<ApiResponse<AttendanceSettings>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    crate::policy::require_settings_read(&context.actor)?;
    let result = settings::get(pool, term).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/attendance/settings/{term}",operation_id="attendance_save_settings",tag="attendance",params(("term"=Uuid,Path)),request_body=SaveAttendanceSettings,responses((status=200, description = "Attendance settings saved",body=ApiResponse<AttendanceSettings>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn save_settings(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
    Json(payload): Json<SaveAttendanceSettings>,
) -> Result<Json<ApiResponse<AttendanceSettings>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    context
        .actor
        .require_permission(codes::ATTENDANCE_MANAGE_SCHOOL)?;
    let result = settings::save(pool, context.actor.user_id, term, payload).await?;
    runtime::refresh_after_commit(
        &context.tenant.subdomain,
        pool.as_ref().clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/days",operation_id="attendance_list_days",tag="attendance",params(AttendanceMonthQuery),responses((status=200, description = "Attendance calendar days returned",body=ApiResponse<Vec<AttendanceDay>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn list_days(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AttendanceMonthQuery>,
) -> Result<Json<ApiResponse<Vec<AttendanceDay>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    context
        .actor
        .require_permission(codes::ATTENDANCE_MANAGE_SCHOOL)?;
    let result = settings::days(pool, query.academic_term_id, query.start, query.end).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/attendance/days/{term}",operation_id="attendance_save_days",tag="attendance",params(("term"=Uuid,Path)),request_body=SaveAttendanceDays,responses((status=200, description = "Attendance calendar days saved",body=ApiResponse<AttendanceSettings>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn save_days(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
    Json(payload): Json<SaveAttendanceDays>,
) -> Result<Json<ApiResponse<AttendanceSettings>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    context
        .actor
        .require_permission(codes::ATTENDANCE_MANAGE_SCHOOL)?;
    let result = settings::save_days(pool, context.actor.user_id, term, payload).await?;
    runtime::refresh_after_commit(
        &context.tenant.subdomain,
        pool.as_ref().clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/workspace",operation_id="attendance_workspace",tag="attendance",params(AttendanceQuery),responses((status=200, description = "Attendance workspace returned",body=ApiResponse<AttendanceWorkspace>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn workspace(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AttendanceQuery>,
) -> Result<Json<ApiResponse<AttendanceWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::workspace(pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/preview",operation_id="attendance_open_session",tag="attendance",params(OpenAttendanceSession),responses((status=200, description = "Attendance preview returned",body=ApiResponse<AttendanceDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn open_session(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(payload): Query<OpenAttendanceSession>,
) -> Result<Json<ApiResponse<AttendanceDetail>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::open(pool, &context.actor, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/sessions/{id}",operation_id="attendance_get_session",tag="attendance",params(("id"=Uuid,Path)),responses((status=200, description = "Attendance session detail returned",body=ApiResponse<AttendanceDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn get_session(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AttendanceDetail>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = sessions::detail(pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/attendance/sessions/{id}",operation_id="attendance_save_results",tag="attendance",params(("id"=Uuid,Path)),request_body=SaveAttendanceResults,responses((status=200, description = "Attendance results saved",body=ApiResponse<AttendanceDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn save_results(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SaveAttendanceResults>,
) -> Result<Json<ApiResponse<AttendanceDetail>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::save(pool, &context.actor, id, payload).await?;
    runtime::wake_after_commit(
        &context.tenant.subdomain,
        pool.as_ref().clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/attendance/sessions/{id}/cancellation",operation_id="attendance_cancel_session",tag="attendance",params(("id"=Uuid,Path)),request_body=AttendanceCancellation,responses((status=200, description = "Attendance session cancellation updated",body=ApiResponse<AttendanceDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn cancel_session(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AttendanceCancellation>,
) -> Result<Json<ApiResponse<AttendanceDetail>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::cancel(pool, &context.actor, id, payload).await?;
    runtime::wake_after_commit(
        &context.tenant.subdomain,
        pool.as_ref().clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/options",operation_id="attendance_get_options",tag="attendance",params(AttendanceTermQuery),responses((status=200, description = "Attendance options returned",body=ApiResponse<AttendanceOptions>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn get_options(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AttendanceTermQuery>,
) -> Result<Json<ApiResponse<AttendanceOptions>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::options(pool, &context.actor, query.academic_term_id).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/attendance/audiences/{term}",operation_id="attendance_create_audience",tag="attendance",params(("term"=Uuid,Path)),request_body=SaveAttendanceAudience,responses((status=200, description = "Attendance audience group created",body=ApiResponse<AttendanceAudienceGroup>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn create_audience(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
    Json(payload): Json<SaveAttendanceAudience>,
) -> Result<Json<ApiResponse<AttendanceAudienceGroup>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    context
        .actor
        .require_permission(codes::ATTENDANCE_MANAGE_SCHOOL)?;
    let result = specials::save_audience(
        pool,
        context.actor.user_id,
        term,
        payload.id.unwrap_or_else(Uuid::new_v4),
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/attendance/specials/{term}",operation_id="attendance_create_special",tag="attendance",params(("term"=Uuid,Path)),request_body=SpecialAttendanceDefinition,responses((status=200, description = "Special attendance template created",body=ApiResponse<SpecialAttendanceTemplate>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn create_special(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
    Json(payload): Json<SpecialAttendanceDefinition>,
) -> Result<Json<ApiResponse<SpecialAttendanceTemplate>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    if payload.scoring.is_some() {
        context
            .actor
            .require_permission(codes::BEHAVIOR_MANAGE_SCHOOL)?;
    }
    let pool = &context.tenant.pool;
    let result = services::create_special(pool, &context.actor, term, payload).await?;
    runtime::refresh_after_commit(
        &context.tenant.subdomain,
        pool.as_ref().clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/attendance/devices/{id}",operation_id="attendance_save_device",tag="attendance",params(("id"=Uuid,Path)),request_body=SaveAttendanceDevice,responses((status=200, description = "Attendance device saved",body=ApiResponse<AttendanceDevice>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn save_device(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SaveAttendanceDevice>,
) -> Result<Json<ApiResponse<AttendanceDevice>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    context
        .actor
        .require_permission(codes::ATTENDANCE_MANAGE_SCHOOL)?;
    let result = specials::save_device(pool, context.actor.user_id, id, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/faces",operation_id="attendance_face_enrollments",tag="attendance",params(AttendanceTermQuery),responses((status=200, description = "Attendance face enrollments returned",body=ApiResponse<Vec<AttendanceFaceEnrollment>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse))))]
pub async fn face_enrollments(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AttendanceTermQuery>,
) -> Result<Json<ApiResponse<Vec<AttendanceFaceEnrollment>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::face_enrollments(&context.tenant.pool, &context.actor, query.academic_term_id)
            .await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/attendance/faces/{student}",operation_id="attendance_enroll_face",tag="attendance",params(("student"=Uuid,Path)),request_body=EnrollAttendanceFace,responses((status=200, description = "Attendance face enrollment saved",body=ApiResponse<EmptyData>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn enroll_face(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student): Path<Uuid>,
    Json(payload): Json<EnrollAttendanceFace>,
) -> Result<Json<ApiResponse<EmptyData>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    services::require_enrollment(pool, &context.actor, student).await?;
    let result = {
        faces::invalidate_gallery(&context.tenant.subdomain)?;
        faces::enroll(pool, context.actor.user_id, student, payload).await?;
        faces::invalidate_gallery(&context.tenant.subdomain)?;
        EmptyData {}
    };
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(delete,path="/api/attendance/faces/{student}",operation_id="attendance_delete_face",tag="attendance",params(("student"=Uuid,Path)),responses((status=200, description = "Attendance face enrollment removed",body=ApiResponse<EmptyData>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn delete_face(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student): Path<Uuid>,
) -> Result<Json<ApiResponse<EmptyData>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    services::require_enrollment(pool, &context.actor, student).await?;
    let result = {
        faces::invalidate_gallery(&context.tenant.subdomain)?;
        faces::remove(pool, context.actor.user_id, student).await?;
        faces::invalidate_gallery(&context.tenant.subdomain)?;
        EmptyData {}
    };
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/attendance/kiosk/open",operation_id="attendance_open_kiosk",tag="attendance",request_body=OpenAttendanceKiosk,responses((status=200, description = "Attendance kiosk workspace returned",body=ApiResponse<AttendanceKioskWorkspace>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn open_kiosk(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<OpenAttendanceKiosk>,
) -> Result<Json<ApiResponse<AttendanceKioskWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::prepare_kiosk(pool, &context.actor, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/attendance/scans",operation_id="attendance_scan",tag="attendance",request_body=AttendanceScan,responses((status=200, description = "Attendance scan outcome returned",body=ApiResponse<AttendanceScanOutcome>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn scan(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<AttendanceScan>,
) -> Result<Json<ApiResponse<AttendanceScanOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::scan(&context, payload).await?;
    runtime::wake_after_commit(
        &context.tenant.subdomain,
        pool.as_ref().clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/report",operation_id="attendance_report",tag="attendance",params(AttendanceReportQuery),responses((status=200, description = "Attendance report returned",body=ApiResponse<AttendanceReport>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn report(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AttendanceReportQuery>,
) -> Result<Json<ApiResponse<AttendanceReport>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::report(pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/history",operation_id="attendance_history",tag="attendance",params(AttendanceHistoryQuery),responses((status=200, description = "Attendance history returned",body=ApiResponse<Vec<AttendanceHistoryItem>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn history(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AttendanceHistoryQuery>,
) -> Result<Json<ApiResponse<Vec<AttendanceHistoryItem>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    let result = services::history(pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/terms/{term}/purge",operation_id="attendance_purge_impact",tag="attendance",params(("term"=Uuid,Path)),responses((status=200, description = "Attendance term purge impact returned",body=ApiResponse<AttendancePurgeImpact>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn purge_impact(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
) -> Result<Json<ApiResponse<AttendancePurgeImpact>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    context
        .actor
        .require_permission(codes::ATTENDANCE_DELETE_SCHOOL)?;
    let result = reports::impact(pool, term).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/attendance/terms/{term}/purge",operation_id="attendance_purge_term",tag="attendance",params(("term"=Uuid,Path)),request_body=PurgeAttendanceTerm,responses((status=200, description = "Attendance term purge result returned",body=ApiResponse<AttendancePurgeImpact>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn purge_term(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
    Json(payload): Json<PurgeAttendanceTerm>,
) -> Result<Json<ApiResponse<AttendancePurgeImpact>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = &context.tenant.pool;
    context
        .actor
        .require_permission(codes::ATTENDANCE_DELETE_SCHOOL)?;
    let result = services::purge(pool, context.actor.user_id, term, payload).await?;
    runtime::refresh_after_commit(
        &context.tenant.subdomain,
        pool.as_ref().clone(),
        state.notification_channel.clone(),
    )
    .await;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/attendance/reset",operation_id="attendance_reset_impact",tag="attendance",responses((status=200, description = "Attendance module reset impact returned",body=ApiResponse<AttendanceResetImpact>),(status=403, description = "Permission denied",body=ApiErrorResponse))))]
pub async fn reset_impact(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<AttendanceResetImpact>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    crate::policy::require_reset(&context.actor)?;
    Ok(Json(ApiResponse::ok(
        crate::services::reset::impact(&context.tenant.pool).await?,
    )))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/attendance/reset",operation_id="attendance_reset_module",tag="attendance",request_body=ResetAttendanceModule,responses((status=200, description = "Attendance module reset result returned",body=ApiResponse<AttendanceResetImpact>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn reset_module(
    State(state): State<AttendanceHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<ResetAttendanceModule>,
) -> Result<Json<ApiResponse<AttendanceResetImpact>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    crate::policy::require_reset(&context.actor)?;
    let result = services::reset(&context.tenant.pool, context.actor.user_id, payload).await?;
    if crate::services::faces::invalidate_gallery(&context.tenant.subdomain).is_err() {
        tracing::warn!(error_code = "attendance_reset_gallery_invalidation_failed");
    }
    runtime::refresh_after_commit(
        &context.tenant.subdomain,
        context.tenant.pool.as_ref().clone(),
        state.notification_channel.clone(),
    )
    .await;
    let impact = services::complete_reset_cleanup(
        &context.tenant.pool,
        state.file_platform.as_ref(),
        result,
    )
    .await;
    Ok(Json(ApiResponse::ok(impact)))
}
pub fn routes() -> axum::Router<AttendanceHttpState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route(
            "/api/attendance/reset",
            get(reset_impact).post(reset_module),
        )
        .route(
            "/api/attendance/settings/{term}",
            get(get_settings).put(save_settings),
        )
        .route("/api/attendance/days", get(list_days))
        .route("/api/attendance/days/{term}", put(save_days))
        .route("/api/attendance/workspace", get(workspace))
        .route("/api/attendance/preview", get(open_session))
        .route(
            "/api/attendance/sessions/{id}",
            get(get_session).put(save_results),
        )
        .route(
            "/api/attendance/sessions/{id}/cancellation",
            put(cancel_session),
        )
        .route("/api/attendance/options", get(get_options))
        .route("/api/attendance/audiences/{term}", post(create_audience))
        .route("/api/attendance/specials/{term}", post(create_special))
        .route("/api/attendance/devices/{id}", put(save_device))
        .route("/api/attendance/faces", get(face_enrollments))
        .route(
            "/api/attendance/faces/{student}",
            put(enroll_face).delete(delete_face),
        )
        .route("/api/attendance/kiosk/open", post(open_kiosk))
        .route("/api/attendance/scans", post(scan))
        .route("/api/attendance/report", get(report))
        .route("/api/attendance/history", get(history))
        .route(
            "/api/attendance/terms/{term}/purge",
            get(purge_impact).post(purge_term),
        )
}
