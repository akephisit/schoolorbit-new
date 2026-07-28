use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{
    platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility},
    repository::PlatformFile,
};

#[derive(Debug, ToSchema)]
#[schema(as = FileUploadMultipart)]
#[allow(dead_code)] // OpenAPI-only shape; the handler streams multipart fields directly.
pub struct FileUploadMultipart {
    pub purpose: FilePurpose,
    pub resource_id: Option<Uuid>,
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FileAccessQuery {
    pub resource_id: Option<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    pub id: Uuid,
    pub purpose: FilePurpose,
    pub lifecycle_status: FileLifecycleStatus,
    pub display_filename: String,
    pub detected_mime_type: String,
    pub byte_size: i64,
    #[schema(required = true)]
    pub current_version: Option<u32>,
    #[schema(required = true)]
    pub public_content_url: Option<String>,
}

impl From<PlatformFile> for FileMetadata {
    fn from(file: PlatformFile) -> Self {
        let public_content_url = (file.visibility == FileVisibility::Public
            && file.lifecycle_status == FileLifecycleStatus::Ready)
            .then(|| format!("/api/public/files/{}/content", file.id));

        Self {
            id: file.id,
            purpose: file.purpose,
            lifecycle_status: file.lifecycle_status,
            display_filename: file.display_filename,
            detected_mime_type: file.detected_mime_type,
            byte_size: file.byte_size,
            current_version: file.current_version,
            public_content_url,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileDeleteResult {
    pub pending_retry: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_exposes_only_platform_identity_and_authorized_fields() {
        let id = Uuid::new_v4();
        let metadata = FileMetadata::from(PlatformFile {
            id,
            owner_user_id: Some(Uuid::new_v4()),
            purpose: FilePurpose::SchoolLogo,
            visibility: FileVisibility::Public,
            lifecycle_status: FileLifecycleStatus::Ready,
            current_version: Some(1),
            display_filename: "logo.png".to_string(),
            detected_mime_type: "image/png".to_string(),
            byte_size: 64,
        });
        let json = serde_json::to_value(metadata).unwrap();

        assert_eq!(
            json["publicContentUrl"],
            format!("/api/public/files/{id}/content")
        );
        for forbidden in [
            "ownerUserId",
            "visibility",
            "storagePath",
            "objectKey",
            "bucket",
            "provider",
            "checksum",
            "signedUrl",
        ] {
            assert!(json.get(forbidden).is_none());
        }
    }
}
