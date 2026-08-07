use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{
    platform_service::PublicDelivery,
    platform_types::{DownloadGrant, FileLifecycleStatus, FilePurpose, FileVisibility},
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
pub struct FileDownloadGrantResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

impl TryFrom<DownloadGrant> for FileDownloadGrantResponse {
    type Error = ();

    fn try_from(grant: DownloadGrant) -> Result<Self, Self::Error> {
        match grant {
            DownloadGrant::Redirect {
                location,
                expires_at,
            } => Ok(Self {
                url: location,
                expires_at,
            }),
            DownloadGrant::Stream { .. } => Err(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicFileDeliveryResponse {
    pub url: String,
}

impl From<PublicDelivery> for PublicFileDeliveryResponse {
    fn from(delivery: PublicDelivery) -> Self {
        Self {
            url: delivery.location.to_string(),
        }
    }
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
    use crate::modules::files::platform_service::PublicDelivery;

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

    #[test]
    fn download_grant_response_exposes_only_temporary_delivery_fields() {
        let expires_at = Utc::now();
        let response = FileDownloadGrantResponse::try_from(DownloadGrant::Redirect {
            location: "https://provider.example/private?temporary=1".to_string(),
            expires_at,
        })
        .unwrap();
        let json = serde_json::to_value(response).unwrap();

        assert_eq!(json["url"], "https://provider.example/private?temporary=1");
        assert!(json.get("expiresAt").is_some());
        assert_eq!(json.as_object().unwrap().len(), 2);
    }

    #[test]
    fn download_grant_response_rejects_unsupported_stream_delivery() {
        assert!(FileDownloadGrantResponse::try_from(DownloadGrant::Stream {
            content_type: "image/jpeg".to_string(),
            content_length: Some(1),
        })
        .is_err());
    }

    #[test]
    fn public_delivery_response_exposes_only_the_delivery_url() {
        let response = PublicFileDeliveryResponse::from(PublicDelivery {
            location: url::Url::parse("https://public-files.example.test/logo.png").unwrap(),
            content_type: "image/png".to_string(),
        });
        let json = serde_json::to_value(response).unwrap();

        assert_eq!(json["url"], "https://public-files.example.test/logo.png");
        assert_eq!(json.as_object().unwrap().len(), 1);
    }
}
