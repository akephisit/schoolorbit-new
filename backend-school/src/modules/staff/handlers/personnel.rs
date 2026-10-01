use crate::{
    policies::staff_access_policy, utils::request_context::actor_tenant_context_from_session,
    AppState,
};
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_http::{ApiErrorResponse, ApiResponse, HttpError as AppError};
use school_staff::{personnel::*, services::reference_service};
use uuid::Uuid;

#[utoipa::path(get, path="/api/staff/reference-items", operation_id="listStaffReferenceItems", tag="staff", params(ReferenceListQuery), responses((status=200, description="Reference catalog", body=ApiResponse<ReferencePage>),(status=401, description="Authentication required", body=ApiErrorResponse),(status=403, description="Permission denied", body=ApiErrorResponse)))]
pub async fn list_reference_items(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ReferenceListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    staff_access_policy::require_reference_read(&context.actor)?;
    Ok(Json(ApiResponse::ok(
        reference_service::list_reference_items(&context.tenant.pool, query).await?,
    )))
}
#[utoipa::path(post, path="/api/staff/reference-items", operation_id="createStaffReferenceItem", tag="staff", request_body=CreateReferenceRequest, responses((status=201, description="Reference created", body=ApiResponse<StaffReferenceItem>),(status=400, description="Invalid name", body=ApiErrorResponse),(status=401, description="Authentication required", body=ApiErrorResponse),(status=403, description="Permission denied", body=ApiErrorResponse),(status=409, description="Duplicate name", body=ApiErrorResponse)))]
pub async fn create_reference_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<CreateReferenceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    staff_access_policy::require_reference_write(&context.actor)?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::ok(
            reference_service::create_reference_item(&context.tenant.pool, input).await?,
        )),
    ))
}
#[utoipa::path(patch, path="/api/staff/reference-items/{id}", operation_id="updateStaffReferenceItem", tag="staff", params(("id"=Uuid, Path, description="Reference ID")), request_body=UpdateReferenceRequest, responses((status=200, description="Reference updated", body=ApiResponse<StaffReferenceItem>),(status=400, description="Invalid name", body=ApiErrorResponse),(status=401, description="Authentication required", body=ApiErrorResponse),(status=403, description="Permission denied", body=ApiErrorResponse),(status=404, description="Missing reference", body=ApiErrorResponse),(status=409, description="Duplicate name", body=ApiErrorResponse)))]
pub async fn update_reference_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateReferenceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    staff_access_policy::require_reference_write(&context.actor)?;
    Ok(Json(ApiResponse::ok(
        reference_service::update_reference_item(&context.tenant.pool, id, input).await?,
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
