use crate::http::state::StaffHttpState;
use crate::personnel::*;
use crate::services::job_position_service;
#[cfg(feature = "api-docs")]
use crate::services::rank_milestone_service::RankMilestoneOverview;
use crate::services::rank_milestone_service::RankMilestoneOverviewQuery;
use crate::staff_access_policy;
use axum::{
    extract::{Extension, Query, State},
    response::IntoResponse,
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, HttpError as AppError};

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/staff/job-positions", operation_id="listStaffJobPositions", tag="staff", params(JobPositionListQuery), responses((status=200, description="Reference catalog", body=ApiResponse<JobPositionPage>),(status=401, description="Authentication required", body=ApiErrorResponse),(status=403, description="Permission denied", body=ApiErrorResponse))))]
pub async fn list_job_positions(
    State(state): State<StaffHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<JobPositionListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    staff_access_policy::require_job_position_read(&context.actor)?;
    Ok(Json(ApiResponse::ok(
        job_position_service::list_job_positions(&context.tenant.pool, query).await?,
    )))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/staff/personnel-overview",operation_id="getPersonnelOverview",tag="staff",params(PersonnelOverviewQuery),responses((status=200,description="Scoped personnel counts",body=ApiResponse<PersonnelOverview>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse))))]
pub async fn get_personnel_overview(
    State(state): State<StaffHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<PersonnelOverviewQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let access = super::staff::staff_list_access(
        staff_access_policy::resolve_staff_profile_list_access(&context.actor)?,
    );
    Ok(Json(ApiResponse::ok(
        crate::services::personnel_overview_service::get_personnel_overview(
            &context.tenant.pool,
            query,
            access,
        )
        .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/staff/personnel-rank-milestones",operation_id="getRankMilestoneOverview",tag="staff",params(RankMilestoneOverviewQuery),responses((status=200,description="Scoped rank tenure planning milestones",body=ApiResponse<RankMilestoneOverview>),(status=400,description="Invalid status, bucket or page",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse))))]
pub async fn get_rank_milestone_overview(
    State(state): State<StaffHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    query: Result<Query<RankMilestoneOverviewQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let access = super::staff::staff_list_access(
        staff_access_policy::resolve_staff_profile_list_access(&context.actor)?,
    );
    let Query(query) =
        query.map_err(|_| AppError::BadRequest("ตัวกรองกำหนดเวลาวิทยฐานะไม่ถูกต้อง".into()))?;
    Ok(Json(ApiResponse::ok(
        crate::services::rank_milestone_service::get_rank_milestone_overview(
            &context.tenant.pool,
            query,
            access,
        )
        .await?,
    )))
}
