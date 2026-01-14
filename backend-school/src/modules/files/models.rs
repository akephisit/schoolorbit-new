use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ===================================================================
// File Models
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct File {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub school_id: Option<String>,
    
    // File Information
    pub filename: String,
    pub original_filename: String,
    pub file_size: i64,
    pub mime_type: String,
    
    // Storage (PATH-BASED)
    pub storage_path: String,
    
    // File Classification
    pub file_type: String,
    
    // Image Metadata
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub has_thumbnail: bool,
    pub thumbnail_path: Option<String>,
    
    // Lifecycle
    pub is_temporary: bool,
    pub is_public: bool,
    pub expires_at: Option<DateTime<Utc>>,
    
    // Security
    pub checksum: Option<String>,
    
    // Audit
    pub uploaded_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// ===================================================================
// File Type Enum
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    ProfileImage,
    Document,
    Transcript,
    Certificate,
    CourseMaterial,
    Assignment,
    SchoolLogo,
    SchoolBanner,
    IdCard,
    Other,
}

impl FileType {
    pub fn as_str(&self) -> &str {
        match self {
            FileType::ProfileImage => "profile_image",
            FileType::Document => "document",
            FileType::Transcript => "transcript",
            FileType::Certificate => "certificate",
            FileType::CourseMaterial => "course_material",
            FileType::Assignment => "assignment",
            FileType::SchoolLogo => "school_logo",
            FileType::SchoolBanner => "school_banner",
            FileType::IdCard => "id_card",
            FileType::Other => "other",
        }
    }
    
    pub fn from_str(s: &str) -> Self {
        match s {
            "profile_image" => FileType::ProfileImage,
            "document" => FileType::Document,
            "transcript" => FileType::Transcript,
            "certificate" => FileType::Certificate,
            "course_material" => FileType::CourseMaterial,
            "assignment" => FileType::Assignment,
            "school_logo" => FileType::SchoolLogo,
            "school_banner" => FileType::SchoolBanner,
            "id_card" => FileType::IdCard,
            _ => FileType::Other,
        }
    }
    
    /// Get the maximum allowed file size in MB for this file type
    pub fn max_size_mb(&self) -> u64 {
        match self {
            FileType::ProfileImage => 5,
            FileType::SchoolLogo => 2,
            FileType::SchoolBanner => 5,
            FileType::Document | FileType::Transcript | FileType::Certificate => 20,
            FileType::CourseMaterial | FileType::Assignment => 10,
            FileType::IdCard => 5,
            FileType::Other => 10,
        }
    }
    
    /// Get the storage subfolder for this file type
    pub fn storage_folder(&self) -> &str {
        match self {
            FileType::ProfileImage => "users/profiles",
            FileType::Document | FileType::Transcript | FileType::Certificate | FileType::IdCard => "users/documents",
            FileType::CourseMaterial | FileType::Assignment => "courses",
            FileType::SchoolLogo | FileType::SchoolBanner => "school",
            FileType::Other => "files",
        }
    }
}

// ===================================================================
// Request/Response Models
// ===================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadFileRequest {
    pub file_type: String,
    pub is_temporary: Option<bool>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileResponse {
    pub id: Uuid,
    pub filename: String,
    pub original_filename: String,
    pub file_size: i64,
    pub mime_type: String,
    pub file_type: String,
    
    // URL (not path!)
    pub url: String,
    pub thumbnail_url: Option<String>,
    
    // Metadata
    pub width: Option<i32>,
    pub height: Option<i32>,
    
    pub created_at: DateTime<Utc>,
}

impl FileResponse {
    /// Convert File model to FileResponse with URLs
    pub fn from_file(file: File, base_url: &str) -> Self {
        let url = format!("{}/{}", base_url.trim_end_matches('/'), file.storage_path);
        let thumbnail_url = file.thumbnail_path
            .map(|path| format!("{}/{}", base_url.trim_end_matches('/'), path));
        
        Self {
            id: file.id,
            filename: file.filename,
            original_filename: file.original_filename,
            file_size: file.file_size,
            mime_type: file.mime_type,
            file_type: file.file_type,
            url,
            thumbnail_url,
            width: file.width,
            height: file.height,
            created_at: file.created_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileListResponse {
    pub success: bool,
    pub files: Vec<FileResponse>,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteFileResponse {
    pub success: bool,
    pub message: String,
}

// ===================================================================
// File Upload Metadata
// ===================================================================

#[derive(Debug, Clone)]
pub struct FileUploadMetadata {
    pub user_id: Uuid,
    pub school_id: String,
    pub file_type: FileType,
    pub original_filename: String,
    pub content_type: String,
    pub is_temporary: bool,
    pub is_public: bool,
}

// ===================================================================
// File Validation
// ===================================================================

#[derive(Debug, Clone)]
pub struct FileValidationConfig {
    pub max_file_size_mb: u64,
    pub max_profile_image_size_mb: u64,
    pub max_document_size_mb: u64,
    pub allowed_image_types: Vec<String>,
    pub allowed_document_types: Vec<String>,
}

impl FileValidationConfig {
    pub fn from_env() -> Self {
        use std::env;
        
        Self {
            max_file_size_mb: env::var("MAX_FILE_SIZE_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            max_profile_image_size_mb: env::var("MAX_PROFILE_IMAGE_SIZE_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            max_document_size_mb: env::var("MAX_DOCUMENT_SIZE_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20),
            allowed_image_types: env::var("ALLOWED_IMAGE_TYPES")
                .ok()
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                .unwrap_or_else(|| vec!["jpg".into(), "jpeg".into(), "png".into(), "webp".into(), "gif".into()]),
            allowed_document_types: env::var("ALLOWED_DOCUMENT_TYPES")
                .ok()
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                .unwrap_or_else(|| vec!["pdf".into(), "doc".into(), "docx".into(), "xls".into(), "xlsx".into()]),
        }
    }
    
    pub fn is_allowed_extension(&self, filename: &str, file_type: &FileType) -> bool {
        let extension = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        match file_type {
            FileType::ProfileImage | FileType::SchoolLogo | FileType::SchoolBanner => {
                self.allowed_image_types.contains(&extension)
            }
            FileType::Document | FileType::Transcript | FileType::Certificate => {
                self.allowed_document_types.contains(&extension)
            }
            _ => true, // Other types allow any extension
        }
    }
}
