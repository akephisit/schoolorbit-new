use crate::{
    policies::staff_access_policy, utils::request_context::actor_tenant_context_from_session,
    AppState,
};
use axum::{
    extract::{Extension, Query, State},
    response::IntoResponse,
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_http::{ApiErrorResponse, ApiResponse, HttpError as AppError};
use school_staff::{personnel::*, services::job_position_service};

#[utoipa::path(get, path="/api/staff/job-positions", operation_id="listStaffJobPositions", tag="staff", params(JobPositionListQuery), responses((status=200, description="Reference catalog", body=ApiResponse<JobPositionPage>),(status=401, description="Authentication required", body=ApiErrorResponse),(status=403, description="Permission denied", body=ApiErrorResponse)))]
pub async fn list_job_positions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<JobPositionListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    staff_access_policy::require_job_position_read(&context.actor)?;
    Ok(Json(ApiResponse::ok(
        job_position_service::list_job_positions(&context.tenant.pool, query).await?,
    )))
}
#[utoipa::path(get,path="/api/staff/personnel-overview",operation_id="getPersonnelOverview",tag="staff",params(PersonnelOverviewQuery),responses((status=200,description="Scoped personnel counts",body=ApiResponse<PersonnelOverview>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse)))]
pub async fn get_personnel_overview(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<PersonnelOverviewQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access = super::staff::staff_list_access(
        staff_access_policy::resolve_staff_profile_list_access(&context.actor)?,
    );
    Ok(Json(ApiResponse::ok(
        school_staff::services::personnel_overview_service::get_personnel_overview(
            &context.tenant.pool,
            query,
            access,
        )
        .await?,
    )))
}
