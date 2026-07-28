use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::files::{
    consumer_service::{map_platform_error, request_deletions},
    platform_types::DownloadGrant,
    repository::SqlFileRepository,
};
use crate::modules::question_bank::models::{QuestionBankListQuery, UpsertQuestionRequest};
use crate::modules::question_bank::services as question_bank_service;
use crate::policies::{file_access_policy, question_bank_access_policy};
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

pub async fn list_questions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<QuestionBankListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let access =
        question_bank_access_policy::resolve_access(&context.tenant.pool, &context.actor).await?;
    let questions =
        question_bank_service::list_questions(&context.tenant.pool, &query, &access).await?;
    Ok(Json(ApiResponse::ok(questions)).into_response())
}

pub async fn list_options(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let access =
        question_bank_access_policy::resolve_access(&context.tenant.pool, &context.actor).await?;
    let options = question_bank_service::list_options(&context.tenant.pool, &access).await?;
    Ok(Json(ApiResponse::ok(options)).into_response())
}

pub async fn get_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let question =
        question_bank_service::get_question(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(question)).into_response())
}

pub async fn get_question_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((question_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let repository = SqlFileRepository::new(context.tenant.pool);
    let file = state
        .file_platform
        .metadata(&repository, file_id)
        .await
        .map_err(map_platform_error)?;
    file_access_policy::authorize_existing(
        repository.pool(),
        &context.actor,
        &file,
        file_access_policy::FilePolicyAction::Read,
        Some(question_id),
    )
    .await?;
    match state
        .file_platform
        .private_download(&repository, file_id)
        .await
        .map_err(map_platform_error)?
    {
        DownloadGrant::Redirect { location, .. } => Ok(Redirect::to(&location).into_response()),
        DownloadGrant::Stream { .. } => Err(AppError::InternalServerError(
            "file_stream_grant_not_supported".to_string(),
        )),
    }
}

pub async fn create_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertQuestionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let question = question_bank_service::create_question(
        &context.tenant.pool,
        &context.actor,
        context.actor.user_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(question))).into_response())
}

pub async fn update_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpsertQuestionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let result = question_bank_service::update_question(
        &context.tenant.pool,
        &context.actor,
        id,
        context.actor.user_id,
        payload,
    )
    .await?;
    request_deletions(
        state.file_platform.as_ref(),
        &context.tenant.pool,
        result.detached_file_ids,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result.question)).into_response())
}

pub async fn delete_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let file_ids =
        question_bank_service::delete_question(&context.tenant.pool, &context.actor, id).await?;
    request_deletions(state.file_platform.as_ref(), &context.tenant.pool, file_ids).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}
