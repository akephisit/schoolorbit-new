use axum::{
    extract::{Multipart, Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::error::AppError;
use crate::modules::admission::models::applications::*;
use crate::modules::admission::services::portal_service;
use crate::services::r2_client::R2Client;
use crate::utils::request_context::{tenant_context, tenant_pool};
use crate::AppState;

pub async fn check_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let info = portal_service::check_application(&pool, payload).await?;
    Ok(Json(json!({ "success": true, "data": info, "message": "ตรวจสอบสำเร็จ" })).into_response())
}

pub async fn get_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let data = portal_service::get_status(&pool, payload).await?;
    Ok(Json(json!({ "success": true, "data": data })).into_response())
}

pub async fn confirm_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalConfirmRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    portal_service::confirm_enrollment(&pool, payload).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ยืนยันเข้าเรียนแล้ว กรุณากรอกแบบฟอร์มมอบตัวด้านล่าง" })).into_response())
}

pub async fn get_enrollment_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let form = portal_service::get_enrollment_form(&pool, payload).await?;
    Ok(Json(json!({ "success": true, "data": form })).into_response())
}

pub async fn submit_enrollment_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalFormRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    portal_service::submit_enrollment_form(&pool, payload).await?;
    Ok(
        Json(json!({ "success": true, "data": {}, "message": "ยืนยันมอบตัวและบันทึกข้อมูลแล้ว" }))
            .into_response(),
    )
}

pub async fn update_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdatePortalApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    portal_service::update_application(&pool, payload).await?;
    Ok(
        Json(json!({ "success": true, "data": {}, "message": "แก้ไขและอัปเดตใบสมัครเรียบร้อยแล้ว" }))
            .into_response(),
    )
}

pub async fn portal_upload_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    let pool = tenant.pool;
    let subdomain = tenant.subdomain;

    let mut doc_type: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;
    let mut mime_type = "application/octet-stream".to_string();
    let mut national_id: Option<String> = None;
    let mut date_of_birth: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("Invalid multipart data".to_string()))?
    {
        match field.name().unwrap_or("") {
            "doc_type" => {
                doc_type = Some(
                    String::from_utf8_lossy(&field.bytes().await.unwrap_or_default()).to_string(),
                )
            }
            "national_id" => {
                national_id = Some(
                    String::from_utf8_lossy(&field.bytes().await.unwrap_or_default()).to_string(),
                )
            }
            "date_of_birth" => {
                date_of_birth = Some(
                    String::from_utf8_lossy(&field.bytes().await.unwrap_or_default()).to_string(),
                )
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

    if !portal_service::VALID_DOC_TYPES.contains(&doc_type.as_str()) {
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
    if !portal_service::ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::BadRequest(format!(
            "File extension .{} not allowed. Use: jpg, png, pdf",
            ext
        )));
    }
    if file_data.len() > 20 * 1024 * 1024 {
        return Err(AppError::BadRequest("File size exceeds 20MB".to_string()));
    }

    let r2_client = R2Client::new()
        .await
        .map_err(|_| AppError::InternalServerError("Storage service unavailable".to_string()))?;

    let input = portal_service::PortalUploadInput {
        doc_type: doc_type.clone(),
        file_data: file_data.clone(),
        original_filename: original_filename.clone(),
        mime_type: mime_type.clone(),
        ext,
        national_id,
        date_of_birth,
    };

    let (result, storage_path) =
        portal_service::save_portal_upload(&pool, &subdomain, &input).await?;

    // R2 upload (after DB success — handler scope)
    r2_client
        .upload_file(&storage_path, file_data, &mime_type)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to upload file".to_string()))?;

    if let Some(old) = &result.old_storage_path {
        r2_client.delete_file(old).await.ok();
    }

    let file_url = portal_service::build_file_url_full(&result.storage_path)?;
    Ok(Json(json!({ "success": true, "data": {
            "fileId": result.file_id,
            "originalFilename": original_filename,
            "fileSize": result.file_size,
            "docType": doc_type,
            "fileUrl": file_url,
        } }))
    .into_response())
}

pub async fn portal_delete_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(doc_type): Path<String>,
    Query(query): Query<PortalDeleteDocumentQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;

    if !portal_service::VALID_DOC_TYPES.contains(&doc_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid doc_type: {}",
            doc_type
        )));
    }

    let storage_path = portal_service::delete_portal_document(&pool, &doc_type, query).await?;

    let r2_client = R2Client::new()
        .await
        .map_err(|_| AppError::InternalServerError("Storage service unavailable".to_string()))?;
    r2_client.delete_file(&storage_path).await.ok();

    Ok(
        Json(json!({ "success": true, "data": {}, "message": "ลบเอกสารเรียบร้อยแล้ว" }))
            .into_response(),
    )
}

pub async fn portal_get_exam_seat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<portal_service::PortalExamSeatRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let seat =
        portal_service::get_exam_seat(&pool, &payload.national_id, &payload.date_of_birth).await?;
    Ok(Json(json!({ "success": true, "data": seat })).into_response())
}
