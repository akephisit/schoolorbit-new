use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::question_bank::models::{QuestionBankListQuery, UpsertQuestionRequest};
use crate::modules::question_bank::services as question_bank_service;
use crate::policies::question_bank_access_policy;
use crate::services::r2_client::R2Client;
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
    let source = question_bank_service::get_question_file_source(
        &context.tenant.pool,
        &context.actor,
        question_id,
        file_id,
    )
    .await?;
    let storage = R2Client::new().await.map_err(|error| {
        tracing::error!("Failed to initialize question image storage: {}", error);
        AppError::InternalServerError("ไม่สามารถเชื่อมต่อพื้นที่เก็บรูปได้".to_string())
    })?;
    let data = storage
        .download_file(&source.storage_path)
        .await
        .map_err(|error| {
            tracing::error!("Failed to download question image: {}", error);
            AppError::InternalServerError("ไม่สามารถดาวน์โหลดรูปประกอบข้อสอบได้".to_string())
        })?;
    let content_type = HeaderValue::from_str(&source.mime_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=300"),
            ),
        ],
        data,
    )
        .into_response())
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
    let question = question_bank_service::update_question(
        &context.tenant.pool,
        &context.actor,
        id,
        context.actor.user_id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(question)).into_response())
}

pub async fn delete_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    question_bank_service::delete_question(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}
