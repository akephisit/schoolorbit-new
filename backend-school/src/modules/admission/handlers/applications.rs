use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::admission::models::applications::*;
use crate::modules::admission::services::application_service;
use crate::modules::files::{
    consumer_service::{map_platform_error, request_deletions},
    platform_service::UploadCommand,
    platform_types::FilePurpose,
    repository::SqlFileRepository,
};
use crate::permissions::registry::codes;
use crate::policies::file_access_policy;
use crate::utils::request_context::{actor_tenant_context, tenant_pool};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitApplicationData {
    application_number: String,
    application: AdmissionApplication,
}

#[derive(Debug, Serialize)]
struct ApplicationWithDocumentsData {
    application: AdmissionApplication,
    documents: Vec<ApplicationDocument>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompleteEnrollmentData {
    user_id: Uuid,
    username: String,
    student_code: String,
}

#[derive(Debug, Serialize)]
struct UpdatedData<T> {
    updated: T,
}

#[derive(Debug, Serialize)]
struct AssignedData<T> {
    assigned: T,
}

#[derive(Debug, ToSchema)]
#[allow(dead_code)]
pub struct StaffDocumentMultipart {
    pub doc_type: String,
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

// ==========================================
// Public submit
// ==========================================

pub async fn submit_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<SubmitApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let (application_number, application) =
        application_service::submit_application(&pool, round_id, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            SubmitApplicationData {
                application_number,
                application,
            },
            "ยื่นใบสมัครสำเร็จ",
        )),
    )
        .into_response())
}

// ==========================================
// Staff: List / Get
// ==========================================

pub async fn list_applications(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Query(filter): Query<ApplicationFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_READ_ALL)?;
    let applications = application_service::list_applications(&pool, round_id, filter).await?;
    Ok(Json(ApiResponse::ok(applications)).into_response())
}

pub async fn get_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_READ_ALL)?;
    let (application, documents) =
        application_service::get_application_with_documents(&pool, id).await?;
    Ok(Json(ApiResponse::ok(ApplicationWithDocumentsData {
        application,
        documents,
    }))
    .into_response())
}

// ==========================================
// Verify / Reject / Absent / Update / Unverify / Delete
// ==========================================

pub async fn verify_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_VERIFY_ALL)?;
    let verifier_id = actor.user_id;
    application_service::verify_application(&pool, id, verifier_id).await?;
    Ok(Json(ApiResponse::empty_with_message("ยืนยันใบสมัครแล้ว")).into_response())
}

pub async fn reject_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<RejectApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_VERIFY_ALL)?;
    application_service::reject_application(&pool, id, &payload.rejection_reason).await?;
    Ok(Json(ApiResponse::empty_with_message("ปฏิเสธใบสมัครแล้ว")).into_response())
}

pub async fn mark_absent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<MarkAbsentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    application_service::mark_absent(&pool, id, payload.absent).await?;
    let msg = if payload.absent {
        "ทำเครื่องหมายขาดสอบแล้ว"
    } else {
        "ยกเลิกขาดสอบแล้ว"
    };
    Ok(Json(ApiResponse::empty_with_message(msg)).into_response())
}

pub async fn update_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_VERIFY_ALL)?;
    application_service::update_application(&pool, id, payload).await?;
    Ok(Json(ApiResponse::empty_with_message("แก้ไขใบสมัครแล้ว")).into_response())
}

pub async fn unverify_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_VERIFY_ALL)?;
    application_service::unverify_application(&pool, id).await?;
    Ok(Json(ApiResponse::empty_with_message("ยกเลิกการอนุมัติแล้ว")).into_response())
}

pub async fn delete_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;

    let file_ids = application_service::fetch_application_files_then_delete(&pool, id).await?;
    request_deletions(state.file_platform.as_ref(), &pool, file_ids).await?;

    Ok(Json(ApiResponse::empty_with_message("ลบใบสมัครแล้ว")).into_response())
}

// ==========================================
// Enrollment
// ==========================================

pub async fn list_enrollment_pending(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_ENROLL_ALL)?;
    let list = application_service::list_enrollment_pending(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(list)).into_response())
}

pub async fn complete_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<CompleteEnrollmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_ENROLL_ALL)?;
    let enroller_id = actor.user_id;

    let result = application_service::complete_enrollment(&pool, id, payload, enroller_id).await?;
    Ok(Json(ApiResponse::with_message(
        CompleteEnrollmentData {
            user_id: result.user_id,
            username: result.username,
            student_code: result.student_code,
        },
        "มอบตัวสำเร็จ สร้าง account แล้ว",
    ))
    .into_response())
}

pub async fn change_application_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(payload): Json<ChangeTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    application_service::change_application_track(&pool, application_id, payload.track_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn update_admission_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(payload): Json<UpdateAdmissionTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_VERIFY_ALL)?;
    application_service::update_admission_track(&pool, application_id, payload.track_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

// ==========================================
// Documents
// ==========================================

#[utoipa::path(
    post,
    path = "/api/admission/applications/{application_id}/documents",
    operation_id = "staffUploadAdmissionDocument",
    tag = "admission",
    params(("application_id" = Uuid, Path, description = "Application ID")),
    request_body(content = StaffDocumentMultipart, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Document attached by file ID", body = ApiResponse<application_service::DocumentUploadResponse>),
        (status = 400, description = "Invalid document or multipart payload", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Admission verification permission required", body = ApiErrorResponse),
        (status = 404, description = "Application not found", body = ApiErrorResponse),
        (status = 503, description = "Scanner or storage unavailable", body = ApiErrorResponse)
    )
)]
pub async fn staff_upload_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_VERIFY_ALL)?;
    file_access_policy::authorize_create(
        &pool,
        &actor,
        FilePurpose::AdmissionApplicationDocument,
        Some(application_id),
    )
    .await?;

    // Parse multipart in handler (Multipart can't cross service boundary)
    let mut doc_type: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("Invalid multipart data".to_string()))?
    {
        match field.name().unwrap_or("") {
            "doc_type" => {
                doc_type = Some(
                    String::from_utf8_lossy(&field.bytes().await.unwrap_or_default()).to_string(),
                );
            }
            "file" => {
                original_filename = field
                    .file_name()
                    .map(|s| s.to_string())
                    .or(Some("document".to_string()));
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| AppError::BadRequest("Failed to read file".to_string()))?
                        .to_vec(),
                );
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let doc_type = doc_type.ok_or_else(|| AppError::BadRequest("Missing doc_type".to_string()))?;
    let file_data = file_data.ok_or_else(|| AppError::BadRequest("Missing file".to_string()))?;
    let original_filename = original_filename.unwrap_or_else(|| "document".to_string());

    if !application_service::VALID_DOC_TYPES.contains(&doc_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid doc_type: {}",
            doc_type
        )));
    }

    let repository = SqlFileRepository::new(pool.clone());
    let file = state
        .file_platform
        .upload(
            &repository,
            UploadCommand {
                tenant_id: context.tenant.tenant_id,
                actor_user_id: Some(actor.user_id),
                owner_user_id: Some(actor.user_id),
                purpose: FilePurpose::AdmissionApplicationDocument,
                display_filename: original_filename,
                bytes: file_data.into(),
            },
        )
        .await
        .map_err(map_platform_error)?;
    let result =
        match application_service::attach_document(&pool, application_id, &doc_type, &file).await {
            Ok(result) => result,
            Err(error) => {
                request_deletions(state.file_platform.as_ref(), &pool, [file.id]).await?;
                return Err(error);
            }
        };
    if let Some(old_file_id) = result.replaced_file_id {
        request_deletions(state.file_platform.as_ref(), &pool, [old_file_id]).await?;
    }

    let response = application_service::document_upload_response(&result, &doc_type);
    Ok(Json(ApiResponse::ok(response)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/admission/applications/{application_id}/documents/{doc_type}",
    operation_id = "staffDeleteAdmissionDocument",
    tag = "admission",
    params(
        ("application_id" = Uuid, Path, description = "Application ID"),
        ("doc_type" = String, Path, description = "Admission document type")
    ),
    responses(
        (status = 200, description = "Document detached and deletion requested", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Invalid document type", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Admission verification permission required", body = ApiErrorResponse),
        (status = 404, description = "Application document not found", body = ApiErrorResponse)
    )
)]
pub async fn staff_delete_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((application_id, doc_type)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_VERIFY_ALL)?;

    if !application_service::VALID_DOC_TYPES.contains(&doc_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid doc_type: {}",
            doc_type
        )));
    }

    let file_id =
        application_service::delete_document_record(&pool, application_id, &doc_type).await?;
    request_deletions(state.file_platform.as_ref(), &pool, [file_id]).await?;

    Ok(Json(ApiResponse::empty()).into_response())
}

// ==========================================
// Student ID Assignment
// ==========================================

pub async fn sort_room_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let updated = application_service::sort_room_students(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(UpdatedData { updated })).into_response())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAssignStudentIdsRequest {
    pub start_number: i64,
}

pub async fn auto_assign_student_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AutoAssignStudentIdsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let assigned =
        application_service::auto_assign_student_ids(&pool, round_id, payload.start_number).await?;
    Ok(Json(ApiResponse::ok(AssignedData { assigned })).into_response())
}

pub async fn list_student_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let rows = application_service::list_student_ids(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn move_application_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<MoveRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    application_service::move_application_room(&pool, id, payload.room_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn batch_update_student_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<Vec<UpdateStudentIdItem>>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let updated = application_service::batch_update_student_ids(&pool, round_id, payload).await?;
    Ok(Json(ApiResponse::ok(UpdatedData { updated })).into_response())
}
