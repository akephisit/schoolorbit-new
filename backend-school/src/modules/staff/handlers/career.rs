use crate::{
    policies::staff_access_policy, utils::request_context::actor_tenant_context_from_session,
    AppState,
};
use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_http::{ApiErrorResponse, ApiResponse, HttpError as AppError};
use school_permissions::registry::codes;
use school_staff::{career::*, services::staff_career_service};
use uuid::Uuid;

#[utoipa::path(get,path="/api/staff/{id}/career-history",operation_id="listStaffCareerHistory",tag="staff",params(("id"=Uuid,Path,description="Staff user ID"),StaffCareerHistoryQuery),responses((status=200,description="Scoped history and current entries",body=ApiResponse<StaffCareerHistoryPage>),(status=400,description="Invalid cursor or page size",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Profile read permission denied",body=ApiErrorResponse),(status=404,description="Staff member not found",body=ApiErrorResponse)))]
pub async fn list_career_history(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(staff_id): Path<Uuid>,
    Query(query): Query<StaffCareerHistoryQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    staff_access_policy::can_read_staff_profile(&context.tenant.pool, &context.actor, staff_id)
        .await?;
    Ok(Json(ApiResponse::ok(
        staff_career_service::list_staff_career_history(&context.tenant.pool, staff_id, query)
            .await?,
    )))
}
#[utoipa::path(post,path="/api/staff/{id}/career-history",operation_id="appendStaffCareerHistory",tag="staff",params(("id"=Uuid,Path,description="Staff user ID")),request_body=CreateStaffCareerHistoryRequest,responses((status=200,description="Entry acknowledgement, including safe retries",body=ApiResponse<StaffCareerMutationAck>),(status=400,description="Invalid historical entry",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Staff update permission denied",body=ApiErrorResponse),(status=404,description="Staff member not found",body=ApiErrorResponse),(status=409,description="ID or payload conflict",body=ApiErrorResponse)))]
pub async fn append_career_history(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(staff_id): Path<Uuid>,
    Json(input): Json<CreateStaffCareerHistoryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context.actor.require_permission(codes::STAFF_UPDATE_ALL)?;
    Ok(Json(ApiResponse::ok(
        staff_career_service::append_staff_career_history(
            &context.tenant.pool,
            staff_id,
            context.actor.user_id,
            input,
        )
        .await?,
    )))
}
#[utoipa::path(patch,path="/api/staff/{id}/career-history/{entryId}",operation_id="correctStaffCareerHistory",tag="staff",params(("id"=Uuid,Path,description="Staff user ID"),("entryId"=Uuid,Path,description="History entry ID")),request_body=CorrectStaffCareerHistoryRequest,responses((status=200,description="Correction acknowledgement",body=ApiResponse<StaffCareerMutationAck>),(status=400,description="Invalid correction or missing reason",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Staff update permission denied",body=ApiErrorResponse),(status=404,description="Staff member or entry not found",body=ApiErrorResponse),(status=409,description="Revision or current status changed",body=ApiErrorResponse)))]
pub async fn correct_career_history(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((staff_id, entry_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CorrectStaffCareerHistoryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context.actor.require_permission(codes::STAFF_UPDATE_ALL)?;
    Ok(Json(ApiResponse::ok(
        staff_career_service::correct_staff_career_history(
            &context.tenant.pool,
            staff_id,
            entry_id,
            context.actor.user_id,
            input,
        )
        .await?,
    )))
}
