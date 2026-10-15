use crate::http::state::AdministrationHttpState;
use crate::supply_catalog::{models::*, services};
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
use school_errors::AppError;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, EmptyData, HttpError};
use uuid::Uuid;

fn require_staff(session: &AuthenticatedSession) -> Result<(), AppError> {
    if session.user_type != "staff" {
        return Err(AppError::Forbidden("ทะเบียนวัสดุสำหรับบุคลากรเท่านั้น".into()));
    }
    Ok(())
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/supply-catalog/categories", operation_id="supplyListCategories", tag="supply-catalog",responses((status=200,description="Supply catalog result",body=ApiResponse<Vec<SupplyCategory>>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn list_categories(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<Vec<SupplyCategory>>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::list_categories(&context.tenant.pool, &context.actor).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/supply-catalog/categories", operation_id="supplyCreateCategory", tag="supply-catalog",request_body=SupplyCategoryWrite,responses((status=201,description="Supply catalog result",body=ApiResponse<SupplyCategory>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn create_category(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SupplyCategoryWrite>,
) -> Result<(StatusCode, Json<ApiResponse<SupplyCategory>>), HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::write_category(&context.tenant.pool, &context.actor, None, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(result))))
}

#[cfg_attr(feature = "api-docs", utoipa::path(put, path="/api/supply-catalog/categories/{id}", operation_id="supplyUpdateCategory", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyCategoryWrite,responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyCategory>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn update_category(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SupplyCategoryWrite>,
) -> Result<Json<ApiResponse<SupplyCategory>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::write_category(&context.tenant.pool, &context.actor, Some(id), payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/supply-catalog/items", operation_id="supplyListItems", tag="supply-catalog",params(SupplyItemQuery),responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyItemPage>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn list_items(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<SupplyItemQuery>,
) -> Result<Json<ApiResponse<SupplyItemPage>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::list_items(&context.tenant.pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/supply-catalog/duplicates", operation_id="supplyDuplicates", tag="supply-catalog",params(SupplyDuplicateQuery),responses((status=200,description="Supply catalog result",body=ApiResponse<Vec<SupplyItem>>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn duplicates(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<SupplyDuplicateQuery>,
) -> Result<Json<ApiResponse<Vec<SupplyItem>>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::duplicates(&context.tenant.pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/supply-catalog/items", operation_id="supplyCreateItem", tag="supply-catalog",request_body=SupplyItemWrite,responses((status=201,description="Supply catalog result",body=ApiResponse<SupplyItem>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn create_item(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SupplyItemWrite>,
) -> Result<(StatusCode, Json<ApiResponse<SupplyItem>>), HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::create_item(&context.tenant.pool, &context.actor, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(result))))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/supply-catalog/items/{id}", operation_id="supplyGetItem", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyItemDetail>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn get_item(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SupplyItemDetail>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::get_item(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(put, path="/api/supply-catalog/items/{id}", operation_id="supplyUpdateItem", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyItemUpdate,responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyItem>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn update_item(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SupplyItemUpdate>,
) -> Result<Json<ApiResponse<SupplyItem>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::update_item(
        &context.tenant.pool,
        state.file_platform.as_ref(),
        &context.actor,
        id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/supply-catalog/items/{id}/versions", operation_id="supplyAppendVersion", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyVersionAppend,responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyItemDetail>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn append_version(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SupplyVersionAppend>,
) -> Result<Json<ApiResponse<SupplyItemDetail>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::append_version(
        &context.tenant.pool,
        state.file_platform.as_ref(),
        &context.actor,
        id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(put, path="/api/supply-catalog/items/{id}/versions/{version_id}", operation_id="supplyUpdateVersion", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID"),("version_id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyVersionWrite,responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyItemDetail>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn update_version(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((id, version_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<SupplyVersionWrite>,
) -> Result<Json<ApiResponse<SupplyItemDetail>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::write_version(
        &context.tenant.pool,
        &context.actor,
        id,
        Some(version_id),
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(put, path="/api/supply-catalog/items/{id}/favorite", operation_id="supplySetFavorite", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyFavoriteWrite,responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyItem>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn set_favorite(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SupplyFavoriteWrite>,
) -> Result<Json<ApiResponse<SupplyItem>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::set_favorite(&context.tenant.pool, &context.actor, id, payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/supply-catalog/bundles", operation_id="supplyListBundles", tag="supply-catalog",params(SupplyPageQuery),responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyBundlePage>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn list_bundles(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<SupplyPageQuery>,
) -> Result<Json<ApiResponse<SupplyBundlePage>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::list_bundles(&context.tenant.pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/supply-catalog/bundles", operation_id="supplyCreateBundle", tag="supply-catalog",request_body=SupplyBundleWrite,responses((status=201,description="Supply catalog result",body=ApiResponse<SupplyBundle>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn create_bundle(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SupplyBundleWrite>,
) -> Result<(StatusCode, Json<ApiResponse<SupplyBundle>>), HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::write_bundle(&context.tenant.pool, &context.actor, None, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(result))))
}

#[cfg_attr(feature = "api-docs", utoipa::path(put, path="/api/supply-catalog/bundles/{id}", operation_id="supplyUpdateBundle", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyBundleWrite,responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyBundle>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn update_bundle(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SupplyBundleWrite>,
) -> Result<Json<ApiResponse<SupplyBundle>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::write_bundle(&context.tenant.pool, &context.actor, Some(id), payload).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(delete, path="/api/supply-catalog/bundles/{id}", operation_id="supplyDeleteBundle", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyRevision,responses((status=200,description="Supply catalog result",body=ApiResponse<EmptyData>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn delete_bundle(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SupplyRevision>,
) -> Result<Json<ApiResponse<EmptyData>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    services::delete_bundle(&context.tenant.pool, &context.actor, id, payload).await?;
    Ok(Json(ApiResponse::empty()))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/supply-catalog/proposals", operation_id="supplyListProposals", tag="supply-catalog",params(SupplyProposalQuery),responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyProposalPage>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn list_proposals(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<SupplyProposalQuery>,
) -> Result<Json<ApiResponse<SupplyProposalPage>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::list_proposals(&context.tenant.pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/supply-catalog/proposals", operation_id="supplyCreateProposal", tag="supply-catalog",request_body=SupplyProposalWrite,responses((status=201,description="Supply catalog result",body=ApiResponse<SupplyProposal>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn create_proposal(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SupplyProposalWrite>,
) -> Result<(StatusCode, Json<ApiResponse<SupplyProposal>>), HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::create_proposal(&context.tenant.pool, &context.actor, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(result))))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/supply-catalog/proposals/{id}", operation_id="supplyGetProposal", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyProposal>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn get_proposal(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SupplyProposal>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::get_proposal(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/supply-catalog/proposals/{id}/approve", operation_id="supplyApproveProposal", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyProposalApprove,responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyItemDetail>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn approve_proposal(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SupplyProposalApprove>,
) -> Result<Json<ApiResponse<SupplyItemDetail>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::approve_proposal(
        &context.tenant.pool,
        state.file_platform.as_ref(),
        &context.actor,
        id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/supply-catalog/proposals/{id}/reject", operation_id="supplyRejectProposal", tag="supply-catalog",params(("id" = Uuid, Path, description = "Supply resource ID")),request_body=SupplyProposalReject,responses((status=200,description="Supply catalog result",body=ApiResponse<SupplyProposal>),(status=400,description="Invalid input",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission required",body=ApiErrorResponse),(status=404,description="Resource missing",body=ApiErrorResponse),(status=409,description="Duplicate or stale revision",body=ApiErrorResponse))))]
pub async fn reject_proposal(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SupplyProposalReject>,
) -> Result<Json<ApiResponse<SupplyProposal>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::reject_proposal(
        &context.tenant.pool,
        state.file_platform.as_ref(),
        &context.actor,
        id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/supply-catalog/items/{id}/versions", operation_id="supplyListVersions", tag="supply-catalog", params(("id"=Uuid, Path, description="Supply item ID"), SupplyPageQuery), responses((status=200,body=ApiResponse<SupplyVersionPage>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse))))]
pub async fn list_versions(
    State(state): State<AdministrationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(query): Query<SupplyPageQuery>,
) -> Result<Json<ApiResponse<SupplyVersionPage>>, HttpError> {
    require_staff(&session)?;
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_versions(&context.tenant.pool, &context.actor, id, query).await?,
    )))
}
