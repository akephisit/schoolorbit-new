use axum::{
    extract::{Multipart, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::admission::models::applications::*;
use crate::modules::admission::services::{application_service, portal_service};
use crate::modules::files::{
    consumer_service::{map_platform_error, request_deletions},
    models::FileDownloadGrantResponse,
    platform_service::UploadCommand,
    platform_types::FilePurpose,
    repository::SqlFileRepository,
};
use crate::policies::file_access_policy;
use crate::utils::request_context::{tenant_context, tenant_pool};
use crate::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortalUploadDocumentData {
    pub file_id: Uuid,
    pub original_filename: String,
    pub file_size: i64,
    pub doc_type: String,
}

#[derive(Debug, ToSchema)]
#[allow(dead_code)]
pub struct PortalDocumentMultipart {
    pub doc_type: String,
    pub national_id: String,
    pub date_of_birth: String,
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

pub async fn check_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let info = portal_service::check_application(&pool, payload).await?;
    Ok(Json(ApiResponse::with_message(info, "ตรวจสอบสำเร็จ")).into_response())
}

pub async fn get_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let data = portal_service::get_status(&pool, payload).await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}

pub async fn confirm_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalConfirmRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    portal_service::confirm_enrollment(&pool, payload).await?;
    Ok(Json(ApiResponse::empty_with_message(
        "ยืนยันเข้าเรียนแล้ว กรุณากรอกแบบฟอร์มมอบตัวด้านล่าง",
    ))
    .into_response())
}

pub async fn get_enrollment_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let form = portal_service::get_enrollment_form(&pool, payload).await?;
    Ok(Json(ApiResponse::ok(form)).into_response())
}

pub async fn submit_enrollment_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalFormRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    portal_service::submit_enrollment_form(&pool, payload).await?;
    Ok(Json(ApiResponse::empty_with_message("ยืนยันมอบตัวและบันทึกข้อมูลแล้ว")).into_response())
}

pub async fn update_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdatePortalApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    portal_service::update_application(&pool, payload).await?;
    Ok(Json(ApiResponse::empty_with_message(
        "แก้ไขและอัปเดตใบสมัครเรียบร้อยแล้ว",
    ))
    .into_response())
}

#[utoipa::path(
    post,
    path = "/api/admission/portal/upload",
    operation_id = "portalUploadAdmissionDocument",
    tag = "admission",
    request_body(content = PortalDocumentMultipart, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Applicant document attached by file ID", body = ApiResponse<PortalUploadDocumentData>),
        (status = 400, description = "Invalid credentials, document, or multipart payload", body = ApiErrorResponse),
        (status = 401, description = "Applicant credentials rejected", body = ApiErrorResponse),
        (status = 404, description = "Application not found", body = ApiErrorResponse),
        (status = 503, description = "Scanner or storage unavailable", body = ApiErrorResponse)
    )
)]
pub async fn portal_upload_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    let pool = tenant.pool;

    let mut doc_type: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;
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
    let national_id =
        national_id.ok_or_else(|| AppError::BadRequest("Missing national_id".to_string()))?;
    let date_of_birth =
        date_of_birth.ok_or_else(|| AppError::BadRequest("Missing date_of_birth".to_string()))?;

    if !portal_service::VALID_DOC_TYPES.contains(&doc_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid doc_type: {}",
            doc_type
        )));
    }

    let application_id =
        portal_service::authorize_document_change(&pool, &national_id, &date_of_birth).await?;
    file_access_policy::authorize_portal_application(&pool, application_id, application_id, None)
        .await?;

    let repository = SqlFileRepository::new(pool.clone());
    let file = state
        .file_platform
        .upload(
            &repository,
            UploadCommand {
                tenant_id: tenant.tenant_id,
                actor_user_id: None,
                owner_user_id: None,
                purpose: FilePurpose::AdmissionApplicationDocument,
                display_filename: original_filename.clone(),
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

    Ok(Json(ApiResponse::ok(PortalUploadDocumentData {
        file_id: result.file_id,
        original_filename,
        file_size: result.file_size,
        doc_type,
    }))
    .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/admission/portal/documents/{doc_type}",
    operation_id = "portalDeleteAdmissionDocument",
    tag = "admission",
    params(("doc_type" = String, Path, description = "Admission document type")),
    request_body = PortalCredentials,
    responses(
        (status = 200, description = "Applicant document detached and deletion requested", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Invalid document state or type", body = ApiErrorResponse),
        (status = 401, description = "Applicant credentials rejected", body = ApiErrorResponse),
        (status = 404, description = "Application document not found", body = ApiErrorResponse)
    )
)]
pub async fn portal_delete_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(doc_type): Path<String>,
    Json(credentials): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;

    if !portal_service::VALID_DOC_TYPES.contains(&doc_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid doc_type: {}",
            doc_type
        )));
    }

    let application_id = portal_service::authorize_document_change(
        &pool,
        &credentials.national_id,
        &credentials.date_of_birth,
    )
    .await?;
    file_access_policy::authorize_portal_application(&pool, application_id, application_id, None)
        .await?;
    let file_id =
        application_service::delete_document_record(&pool, application_id, &doc_type).await?;
    request_deletions(state.file_platform.as_ref(), &pool, [file_id]).await?;

    Ok(Json(ApiResponse::empty_with_message("ลบเอกสารเรียบร้อยแล้ว")).into_response())
}

#[utoipa::path(
    post,
    path = "/api/admission/portal/documents/{file_id}/download",
    operation_id = "portalDownloadAdmissionDocument",
    tag = "admission",
    params(("file_id" = Uuid, Path, description = "Logical file ID")),
    request_body = PortalCredentials,
    responses(
        (status = 200, description = "Short-lived private document grant", body = ApiResponse<FileDownloadGrantResponse>),
        (status = 400, description = "Invalid portal credential format", body = ApiErrorResponse),
        (status = 401, description = "Applicant credentials rejected", body = ApiErrorResponse),
        (status = 404, description = "Application document not found", body = ApiErrorResponse),
        (status = 409, description = "File is not ready", body = ApiErrorResponse)
    )
)]
pub async fn portal_download_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
    Json(credentials): Json<PortalCredentials>,
) -> Result<Response, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let application_id = portal_service::authorize_document_download(
        &pool,
        &credentials.national_id,
        &credentials.date_of_birth,
        file_id,
    )
    .await?;
    file_access_policy::authorize_portal_application(
        &pool,
        application_id,
        application_id,
        Some(file_id),
    )
    .await?;
    let repository = SqlFileRepository::new(pool);
    let grant = state
        .file_platform
        .private_download(&repository, file_id)
        .await
        .map_err(map_platform_error)?;
    let response = FileDownloadGrantResponse::try_from(grant).map_err(|()| {
        AppError::InternalServerError("file_stream_grant_not_supported".to_string())
    })?;
    Ok(Json(ApiResponse::ok(response)).into_response())
}

pub async fn portal_get_exam_seat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<portal_service::PortalExamSeatRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let seat =
        portal_service::get_exam_seat(&pool, &payload.national_id, &payload.date_of_birth).await?;
    Ok(Json(ApiResponse::ok(seat)).into_response())
}
