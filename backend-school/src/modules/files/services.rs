use axum::extract::Multipart;
use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    error::AppError,
    services::r2_client::R2Client,
    utils::{file_hash::FileHasher, file_processor::ImageProcessor, file_url::FileUrlBuilder},
};

use super::models::{File, FileListResponse, FileResponse, FileType, FileValidationConfig};

struct ProcessedFileData {
    data: Vec<u8>,
    width: Option<i32>,
    height: Option<i32>,
    thumbnail_data: Option<Vec<u8>>,
}

pub async fn upload_file(
    pool: &PgPool,
    user_id: Uuid,
    subdomain: &str,
    mut multipart: Multipart,
) -> Result<FileResponse, AppError> {
    let validation_config = FileValidationConfig::from_env();
    let r2_client = R2Client::new().await.map_err(|e| {
        error!("Failed to initialize R2 client: {}", e);
        AppError::InternalServerError("Storage service unavailable".to_string())
    })?;

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
                    .map(str::to_string)
                    .or(Some("unnamed".to_string()));
                content_type = field
                    .content_type()
                    .map(str::to_string)
                    .or(Some("application/octet-stream".to_string()));

                let data = field.bytes().await.map_err(|e| {
                    error!("Failed to read file data: {}", e);
                    AppError::BadRequest("Failed to read file".to_string())
                })?;

                file_data = Some(data.to_vec());
            }
            "file_type" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::BadRequest("Invalid file_type".to_string()))?;
                file_type_str = String::from_utf8_lossy(&data).to_string();
            }
            "is_temporary" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::BadRequest("Invalid is_temporary".to_string()))?;
                let value = String::from_utf8_lossy(&data).to_string();
                is_temporary = value == "true" || value == "1";
            }
            _ => {
                warn!("Unknown multipart field: {}", field_name);
            }
        }
    }

    let file_data =
        file_data.ok_or_else(|| AppError::BadRequest("No file provided".to_string()))?;
    let original_filename =
        file_name.ok_or_else(|| AppError::BadRequest("No filename provided".to_string()))?;
    let mime_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    let file_type = FileType::from_str(&file_type_str);

    let max_size = file_type.max_size_mb();
    if let Err(error) =
        crate::utils::file_processor::FileValidator::validate_size(file_data.len(), max_size)
    {
        warn!("File size validation failed: {}", error);
        return Err(AppError::BadRequest(error));
    }

    if !validation_config.is_allowed_extension(&original_filename, &file_type) {
        warn!("File extension not allowed: {}", original_filename);
        return Err(AppError::BadRequest(format!(
            "File type '{}' not allowed for {}",
            original_filename, file_type_str
        )));
    }

    let processed = process_file_data(file_data, &file_type)?;
    let file_id = Uuid::new_v4();
    let extension = std::path::Path::new(&original_filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin");
    let storage_folder = file_type.storage_folder();
    let storage_path = storage_path(
        subdomain,
        storage_folder,
        user_id,
        file_id,
        extension,
        &file_type,
    );
    let thumbnail_path = processed.thumbnail_data.as_ref().map(|_| {
        format!(
            "school-{}/{}/thumbnails/{}_thumb.jpg",
            subdomain, storage_folder, file_id
        )
    });
    let checksum = FileHasher::sha256(&processed.data);
    let file_size = processed.data.len() as i64;

    info!("Uploading to R2: {}", storage_path);
    r2_client
        .upload_file(&storage_path, processed.data, &mime_type)
        .await
        .map_err(|e| {
            error!("Failed to upload to R2: {}", e);
            AppError::InternalServerError("Failed to upload file".to_string())
        })?;

    if let (Some(thumb_data), Some(thumb_path)) = (processed.thumbnail_data, &thumbnail_path) {
        info!("Uploading thumbnail: {}", thumb_path);
        if let Err(error) = r2_client
            .upload_file(thumb_path, thumb_data, "image/jpeg")
            .await
        {
            warn!("Failed to upload thumbnail: {}", error);
        }
    }

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
    .bind(subdomain)
    .bind(format!("{}.{}", file_id, extension))
    .bind(&original_filename)
    .bind(file_size)
    .bind(&mime_type)
    .bind(&storage_path)
    .bind(file_type.as_str())
    .bind(processed.width)
    .bind(processed.height)
    .bind(thumbnail_path.is_some())
    .bind(&thumbnail_path)
    .bind(is_temporary)
    .bind(true)
    .bind(&checksum)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("Failed to save file metadata: {}", e);
        AppError::InternalServerError("Failed to save file metadata".to_string())
    })?;

    let url_builder = FileUrlBuilder::new().map_err(|e| {
        error!("Failed to create URL builder: {}", e);
        AppError::InternalServerError("Configuration error".to_string())
    })?;

    info!("File uploaded successfully: {}", file_id);

    Ok(FileResponse::from_file(file_record, url_builder.base_url()))
}

pub async fn delete_file(pool: &PgPool, user_id: Uuid, file_id: Uuid) -> Result<(), AppError> {
    let file = sqlx::query_as::<_, File>(
        "SELECT * FROM files WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("Database error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?
    .ok_or(AppError::NotFound("File not found".to_string()))?;

    sqlx::query("UPDATE files SET deleted_at = NOW() WHERE id = $1")
        .bind(file_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to delete file metadata: {}", e);
            AppError::InternalServerError("Failed to delete file".to_string())
        })?;

    if let Ok(r2_client) = R2Client::new().await {
        if let Err(error) = r2_client.delete_file(&file.storage_path).await {
            warn!("Failed to delete file from R2: {}", error);
        }

        if let Some(thumb_path) = &file.thumbnail_path {
            if let Err(error) = r2_client.delete_file(thumb_path).await {
                warn!("Failed to delete file thumbnail from R2: {}", error);
            }
        }
    }

    info!("File deleted successfully: {}", file_id);

    Ok(())
}

pub async fn list_user_files(pool: &PgPool, user_id: Uuid) -> Result<FileListResponse, AppError> {
    let files = sqlx::query_as::<_, File>(
        "SELECT * FROM files WHERE user_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch files: {}", e);
        AppError::InternalServerError("Failed to fetch files".to_string())
    })?;

    let total = files.len() as i64;
    let url_builder = FileUrlBuilder::new()
        .map_err(|_| AppError::InternalServerError("Configuration error".to_string()))?;
    let file_responses = files
        .into_iter()
        .map(|file| FileResponse::from_file(file, url_builder.base_url()))
        .collect();

    Ok(FileListResponse {
        success: true,
        files: file_responses,
        total,
    })
}

fn process_file_data(
    file_data: Vec<u8>,
    file_type: &FileType,
) -> Result<ProcessedFileData, AppError> {
    if !matches!(
        file_type,
        FileType::ProfileImage | FileType::SchoolLogo | FileType::SchoolBanner
    ) {
        return Ok(ProcessedFileData {
            data: file_data,
            width: None,
            height: None,
            thumbnail_data: None,
        });
    }

    info!("Processing image file");

    if !ImageProcessor::is_valid_image(&file_data) {
        return Err(AppError::BadRequest("Invalid image file".to_string()));
    }

    let (original_width, original_height) =
        ImageProcessor::get_dimensions(&file_data).map_err(|e| {
            error!("Failed to get image dimensions: {}", e);
            AppError::BadRequest("Invalid image".to_string())
        })?;
    let resized = ImageProcessor::resize_image(&file_data, 2048, 2048).unwrap_or(file_data);
    let thumbnail = if *file_type == FileType::ProfileImage {
        ImageProcessor::create_thumbnail(&resized, 150).ok()
    } else {
        None
    };

    Ok(ProcessedFileData {
        data: resized,
        width: Some(original_width as i32),
        height: Some(original_height as i32),
        thumbnail_data: thumbnail,
    })
}

fn storage_path(
    subdomain: &str,
    storage_folder: &str,
    user_id: Uuid,
    file_id: Uuid,
    extension: &str,
    file_type: &FileType,
) -> String {
    if matches!(
        file_type,
        FileType::Document | FileType::Transcript | FileType::Certificate | FileType::IdCard
    ) {
        return format!(
            "school-{}/{}/{}/{}.{}",
            subdomain, storage_folder, user_id, file_id, extension
        );
    }

    format!(
        "school-{}/{}/{}.{}",
        subdomain, storage_folder, file_id, extension
    )
}
