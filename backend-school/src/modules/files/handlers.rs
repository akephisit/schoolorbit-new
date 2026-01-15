use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    Extension,
};
use serde_json::json;
use std::str::FromStr;

use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    db::school_mapping::get_school_database_url,
    modules::files::models::{
        DeleteFileResponse, File, FileListResponse, FileResponse, FileType,
        FileValidationConfig,
    },
    modules::auth::models::Claims,
    services::r2_client::R2Client,
    utils::{
        file_hash::FileHasher, file_processor::ImageProcessor, file_url::FileUrlBuilder,
        subdomain::extract_subdomain_from_request,
    },
    AppState,
    error::AppError,
};

/// Upload a file
///
/// POST /api/files/upload
///
/// Accepts multipart/form-data with:
/// - file: The file to upload
/// - file_type: Type classification (profile_image, document, etc.)
/// - is_temporary: Optional, mark as temporary file
pub async fn upload_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    subdomain_header: axum::http::HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        error!("Invalid user ID in claims: {}", claims.sub);
        AppError::AuthError("Invalid user authentication".to_string())
    })?;

    info!("Uploading file for user: {}", user_id);

    // Extract subdomain
    let subdomain = extract_subdomain_from_request(&subdomain_header)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    // Get validation config
    let validation_config = FileValidationConfig::from_env();

    // Initialize R2 client
    let r2_client = R2Client::new().await.map_err(|e| {
        error!("Failed to initialize R2 client: {}", e);
        AppError::InternalServerError("Storage service unavailable".to_string())
    })?;

    // Parse multipart form
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut file_type_str = String::from("other");
    let mut is_temporary = false;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to read multipart field: {}", e);
        AppError::BadRequest("Invalid multipart data".to_string())
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                file_name = field
                    .file_name()
                    .map(|s| s.to_string())
                    .or(Some("unnamed".to_string()));
                content_type = field
                    .content_type()
                    .map(|s| s.to_string())
                    .or(Some("application/octet-stream".to_string()));

                let data = field.bytes().await.map_err(|e| {
                    error!("Failed to read file data: {}", e);
                    AppError::BadRequest("Failed to read file".to_string())
                })?;

                file_data = Some(data.to_vec());
            }
            "file_type" => {
                let data = field.bytes().await.map_err(|_| {
                     AppError::BadRequest("Invalid file_type".to_string())
                })?;
                file_type_str = String::from_utf8_lossy(&data).to_string();
            }
            "is_temporary" => {
                let data = field.bytes().await.map_err(|_| {
                     AppError::BadRequest("Invalid is_temporary".to_string())
                })?;
                let value = String::from_utf8_lossy(&data).to_string();
                is_temporary = value == "true" || value == "1";
            }
            _ => {
                warn!("Unknown multipart field: {}", field_name);
            }
        }
    }

    // Validate required fields
    let file_data = file_data.ok_or_else(|| AppError::BadRequest("No file provided".to_string()))?;
    let original_filename = file_name.ok_or_else(|| AppError::BadRequest("No filename provided".to_string()))?;
    let mime_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());

    // Parse file type
    let file_type = FileType::from_str(&file_type_str);

    // Validate file size
    let max_size = file_type.max_size_mb();
    if let Err(e) =
        crate::utils::file_processor::FileValidator::validate_size(file_data.len(), max_size)
    {
        warn!("File size validation failed: {}", e);
        return Err(AppError::BadRequest(e));
    }

    // Validate file extension
    if !validation_config.is_allowed_extension(&original_filename, &file_type) {
        warn!("File extension not allowed: {}", original_filename);
        return Err(AppError::BadRequest(format!("File type '{}' not allowed for {}", original_filename, file_type_str)));
    }

    // Process image if needed
    let (processed_data, width, height, thumbnail_data) = if matches!(
        file_type,
        FileType::ProfileImage | FileType::SchoolLogo | FileType::SchoolBanner
    ) {
        info!("Processing image file");

        // Validate it's actually an image
        if !ImageProcessor::is_valid_image(&file_data) {
            return Err(AppError::BadRequest("Invalid image file".to_string()));
        }

        // Get original dimensions
        let (orig_width, orig_height) = ImageProcessor::get_dimensions(&file_data).map_err(|e| {
            error!("Failed to get image dimensions: {}", e);
            AppError::BadRequest("Invalid image".to_string())
        })?;

        // Resize if needed (max 2048x2048 for storage efficiency)
        let resized = ImageProcessor::resize_image(&file_data, 2048, 2048).unwrap_or(file_data);

        // Create thumbnail for profile images
        let thumbnail = if file_type == FileType::ProfileImage {
            ImageProcessor::create_thumbnail(&resized, 150).ok()
        } else {
            None
        };

        (resized, Some(orig_width as i32), Some(orig_height as i32), thumbnail)
    } else {
        (file_data, None, None, None)
    };

    // Generate storage path
    let file_id = Uuid::new_v4();
    let extension = std::path::Path::new(&original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");

    let storage_folder = file_type.storage_folder();
    let storage_path = if matches!(file_type, FileType::Document | FileType::Transcript | FileType::Certificate | FileType::IdCard) {
        // For user documents, create user-specific folder
        format!(
            "school-{}/{}/{}/{}.{}",
            subdomain, storage_folder, user_id, file_id, extension
        )
    } else {
        format!(
            "school-{}/{}/{}.{}",
            subdomain, storage_folder, file_id, extension
        )
    };

    let thumbnail_path = thumbnail_data.as_ref().map(|_| {
        format!(
            "school-{}/{}/thumbnails/{}_thumb.jpg",
            subdomain, storage_folder, file_id
        )
    });

    // Generate checksum
    let checksum = FileHasher::sha256(&processed_data);

    // Store file size before moving processed_data  
    let file_size = processed_data.len() as i64;

    // Upload to R2
    info!("Uploading to R2: {}", storage_path);
    r2_client
        .upload_file(&storage_path, processed_data, &mime_type)
        .await
        .map_err(|e| {
            error!("Failed to upload to R2: {}", e);
            AppError::InternalServerError("Failed to upload file".to_string())
        })?;

    // Upload thumbnail if exists
    if let (Some(thumb_data), Some(thumb_path)) = (thumbnail_data, &thumbnail_path) {
        info!("Uploading thumbnail: {}", thumb_path);
        if let Err(e) = r2_client
            .upload_file(thumb_path, thumb_data, "image/jpeg")
            .await
        {
            warn!("Failed to upload thumbnail: {}", e);
            // Continue anyway, thumbnail is optional
        }
    }

    // Get school database URL
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            error!("Failed to get school database: {}", e);
            AppError::NotFound("School not found".to_string())
        })?;

    // Get pool
    let pool = state
        .pool_manager
        .get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            error!("Failed to get database pool: {}", e);
            AppError::InternalServerError("Database unavailable".to_string())
        })?;

    let file_record = sqlx::query_as::<_, File>(
        r#"
        INSERT INTO files (
            user_id, school_id, filename, original_filename,
            file_size, mime_type, storage_path, file_type,
            width, height, has_thumbnail, thumbnail_path,
            is_temporary, is_public, checksum, uploaded_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(&subdomain)
    .bind(format!("{}.{}", file_id, extension))
    .bind(&original_filename)
    .bind(file_size)
    .bind(&mime_type)
    .bind(&storage_path)
    .bind(file_type.as_str())
    .bind(width)
    .bind(height)
    .bind(thumbnail_path.is_some())
    .bind(&thumbnail_path)
    .bind(is_temporary)
    .bind(true) // is_public - can be controlled later
    .bind(&checksum)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        error!("Failed to save file metadata: {}", e);
        AppError::InternalServerError("Failed to save file metadata".to_string())
    })?;

    // Build file response with URLs
    let url_builder = FileUrlBuilder::new().map_err(|e| {
        error!("Failed to create URL builder: {}", e);
        AppError::InternalServerError("Configuration error".to_string())
    })?;

    let file_response = FileResponse::from_file(file_record, url_builder.base_url());

    info!("File uploaded successfully: {}", file_id);

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "file": file_response
        })),
    ))
}

/// Delete a file
///
/// DELETE /api/files/:id
pub async fn delete_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    subdomain_header: axum::http::HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        error!("Invalid user ID in claims: {}", claims.sub);
        AppError::AuthError("Invalid user authentication".to_string())
    })?;

    info!("Deleting file: {} for user: {}", file_id, user_id);

    let subdomain = extract_subdomain_from_request(&subdomain_header)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    // Get school database URL
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            error!("Failed to get school database: {}", e);
             AppError::NotFound("School not found".to_string())
        })?;

    // Get pool  
    let pool = state
        .pool_manager
        .get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            error!("Failed to get database pool: {}", e);
            AppError::InternalServerError("Database unavailable".to_string())
        })?;

    // Get file metadata
    let file = sqlx::query_as::<_, File>(
        "SELECT * FROM files WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        error!("Database error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?
    .ok_or(AppError::NotFound("File not found".to_string()))?;

    // Soft delete in database
    sqlx::query("UPDATE files SET deleted_at = NOW() WHERE id = $1")
        .bind(file_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            error!("Failed to delete file metadata: {}", e);
            AppError::InternalServerError("Failed to delete file".to_string())
        })?;

    // Delete from R2 (optional - can be done by cleanup job later)
    let r2_client = R2Client::new().await.ok();
    if let Some(r2) = r2_client {
        if let Err(e) = r2.delete_file(&file.storage_path).await {
            warn!("Failed to delete file from R2: {}", e);
            // Continue anyway, soft delete succeeded
        }

        // Delete thumbnail if exists
        if let Some(ref thumb_path) = file.thumbnail_path {
            let _ = r2.delete_file(thumb_path).await;
        }
    }

    info!("File deleted successfully: {}", file_id);

    Ok((
        StatusCode::OK,
        Json(DeleteFileResponse {
            success: true,
            message: "File deleted successfully".to_string(),
        }),
    ))
}

/// Get file list for current user
///
/// GET /api/files
pub async fn list_user_files(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    subdomain_header: axum::http::HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        error!("Invalid user ID in claims: {}", claims.sub);
        AppError::AuthError("Invalid user authentication".to_string())
    })?;
    let subdomain = extract_subdomain_from_request(&subdomain_header)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    // Get school database URL
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            error!("Failed to get school database: {}", e);
            AppError::NotFound("School not found".to_string())
        })?;

    // Get pool
    let pool = state
        .pool_manager
        .get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            error!("Failed to get database pool: {}", e);
            AppError::InternalServerError("Database unavailable".to_string())
        })?;

    let files = sqlx::query_as::<_, File>(
        "SELECT * FROM files WHERE user_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch files: {}", e);
        AppError::InternalServerError("Failed to fetch files".to_string())
    })?;

    let total = files.len() as i64;

    let url_builder = FileUrlBuilder::new().map_err(|_| {
        AppError::InternalServerError("Configuration error".to_string())
    })?;

    let file_responses: Vec<FileResponse> = files
        .into_iter()
        .map(|f| FileResponse::from_file(f, url_builder.base_url()))
        .collect();

    Ok((
        StatusCode::OK,
        Json(FileListResponse {
            success: true,
            files: file_responses,
            total,
        }),
    ))
}
