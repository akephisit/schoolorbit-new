use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::admission::models::applications::*;
use crate::modules::admission::services::application_service;
use crate::permissions::registry::codes;
use crate::services::r2_client::R2Client;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
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
    let pool = get_pool(&state, &headers).await?;
    let (application_number, application) =
        application_service::submit_application(&pool, round_id, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": {
            "applicationNumber": application_number,
            "application": application,
        }, "message": "ยื่นใบสมัครสำเร็จ" })),
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
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_READ_ALL) {
        return Ok(response);
    }
    let applications = application_service::list_applications(&pool, round_id, filter).await?;
    Ok(Json(json!({ "success": true, "data": applications })).into_response())
}

pub async fn get_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_READ_ALL) {
        return Ok(response);
    }
    let (application, documents) =
        application_service::get_application_with_documents(&pool, id).await?;
    Ok(Json(
        json!({ "success": true, "data": { "application": application, "documents": documents } }),
    )
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
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_VERIFY) {
        return Ok(response);
    }
    let verifier_id = actor.user_id;
    application_service::verify_application(&pool, id, verifier_id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ยืนยันใบสมัครแล้ว" })).into_response())
}

pub async fn reject_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<RejectApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_VERIFY) {
        return Ok(response);
    }
    application_service::reject_application(&pool, id, &payload.rejection_reason).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ปฏิเสธใบสมัครแล้ว" })).into_response())
}

pub async fn mark_absent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<MarkAbsentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    application_service::mark_absent(&pool, id, payload.absent).await?;
    let msg = if payload.absent {
        "ทำเครื่องหมายขาดสอบแล้ว"
    } else {
        "ยกเลิกขาดสอบแล้ว"
    };
    Ok(Json(json!({ "success": true, "data": {}, "message": msg })).into_response())
}

pub async fn update_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_VERIFY) {
        return Ok(response);
    }
    application_service::update_application(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "แก้ไขใบสมัครแล้ว" })).into_response())
}

pub async fn unverify_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_VERIFY) {
        return Ok(response);
    }
    application_service::unverify_application(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ยกเลิกการอนุมัติแล้ว" })).into_response())
}

pub async fn delete_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_MANAGE_ALL) {
        return Ok(response);
    }

    let files_to_delete =
        application_service::fetch_application_files_then_delete(&pool, id).await?;

    // ลบไฟล์ใน R2 (best-effort, ไม่ blocking response)
    if !files_to_delete.is_empty() {
        if let Ok(r2) = R2Client::new().await {
            for (_, storage_path) in &files_to_delete {
                r2.delete_file(storage_path).await.ok();
            }
        }
    }

    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบใบสมัครแล้ว" })).into_response())
}

// ==========================================
// Enrollment
// ==========================================

pub async fn list_enrollment_pending(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_ENROLL) {
        return Ok(response);
    }
    let list = application_service::list_enrollment_pending(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": list })).into_response())
}

pub async fn complete_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<CompleteEnrollmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_ENROLL) {
        return Ok(response);
    }
    let enroller_id = actor.user_id;

    let result = application_service::complete_enrollment(&pool, id, payload, enroller_id).await?;
    Ok(Json(json!({ "success": true, "data": {
            "userId": result.user_id,
            "username": result.username,
            "studentCode": result.student_code,
        }, "message": "มอบตัวสำเร็จ สร้าง account แล้ว" }))
    .into_response())
}

pub async fn change_application_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(payload): Json<ChangeTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    application_service::change_application_track(&pool, application_id, payload.track_id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn update_admission_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(payload): Json<UpdateAdmissionTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_VERIFY) {
        return Ok(response);
    }
    application_service::update_admission_track(&pool, application_id, payload.track_id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

// ==========================================
// Documents
// ==========================================

pub async fn staff_upload_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let pool = get_pool(&state, &headers).await?;

    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_VERIFY) {
        return Ok(response);
    }

    // Parse multipart in handler (Multipart can't cross service boundary)
    let mut doc_type: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;
    let mut mime_type = "application/octet-stream".to_string();

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
                if let Some(ct) = field.content_type() {
                    mime_type = ct.to_string();
                }
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

    let ext = std::path::Path::new(&original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_lowercase();
    if !application_service::ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::BadRequest(format!(
            "File extension .{} not allowed. Use: jpg, png, pdf",
            ext
        )));
    }

    if file_data.len() > 20 * 1024 * 1024 {
        return Err(AppError::BadRequest("File size exceeds 20MB".to_string()));
    }

    let input = application_service::DocumentUploadInput {
        doc_type: doc_type.clone(),
        file_data: file_data.clone(),
        original_filename,
        mime_type: mime_type.clone(),
        ext,
    };

    // R2 ใส่ใน handler — service ทำแค่ DB
    let r2_client = R2Client::new()
        .await
        .map_err(|_| AppError::InternalServerError("Storage service unavailable".to_string()))?;

    // Upload R2 ก่อน (ถ้าพัง = old file ยังอยู่)
    // คำนวณ storage_path ใน service โดยใช้ Uuid::new_v4() — ต้อง call service ก่อน R2 upload
    // เพราะ service ต้องสร้าง file_id และ storage_path matching DB record
    // แต่ถ้าทำ DB ก่อนแล้ว R2 fail = orphan record. Better order:
    //   1. Generate paths in service (insert DB only after R2 success)
    // Simplification: trust ordering — DB insert first, then R2 (mirror original code)

    let result =
        application_service::save_document_record(&pool, &subdomain, application_id, input).await?;

    // Upload to R2 (DB record มีอยู่แล้ว — ถ้าพังต้อง manual cleanup)
    if let Err(_) = r2_client
        .upload_file(&result.storage_path, file_data, &mime_type)
        .await
    {
        return Err(AppError::InternalServerError(
            "Failed to upload file".to_string(),
        ));
    }

    // Delete old file from R2 (DB update succeeded)
    if let Some(old_path) = &result.old_storage_path {
        r2_client.delete_file(old_path).await.ok();
    }

    let response = application_service::document_upload_response(&result, &doc_type)?;
    Ok(Json(json!({ "success": true, "data": response })).into_response())
}

pub async fn staff_delete_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((application_id, doc_type)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_VERIFY) {
        return Ok(response);
    }

    if !application_service::VALID_DOC_TYPES.contains(&doc_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid doc_type: {}",
            doc_type
        )));
    }

    let storage_path =
        application_service::delete_document_record(&pool, application_id, &doc_type).await?;

    let r2_client = R2Client::new()
        .await
        .map_err(|_| AppError::InternalServerError("Storage service unavailable".to_string()))?;
    r2_client.delete_file(&storage_path).await.ok();

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

// ==========================================
// Student ID Assignment
// ==========================================

pub async fn sort_room_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_MANAGE_ALL) {
        return Ok(response);
    }
    let updated = application_service::sort_room_students(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": { "updated": updated } })).into_response())
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
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_MANAGE_ALL) {
        return Ok(response);
    }
    let assigned =
        application_service::auto_assign_student_ids(&pool, round_id, payload.start_number).await?;
    Ok(Json(json!({ "success": true, "data": { "assigned": assigned } })).into_response())
}

pub async fn list_student_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_MANAGE_ALL) {
        return Ok(response);
    }
    let rows = application_service::list_student_ids(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}

pub async fn move_application_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<MoveRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    application_service::move_application_room(&pool, id, payload.room_id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn batch_update_student_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<Vec<UpdateStudentIdItem>>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_MANAGE_ALL) {
        return Ok(response);
    }
    let updated = application_service::batch_update_student_ids(&pool, round_id, payload).await?;
    Ok(Json(json!({ "success": true, "data": { "updated": updated } })).into_response())
}
