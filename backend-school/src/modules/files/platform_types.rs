use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::purpose_registry::ObjectKey;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FilePurpose {
    SchoolLogo,
    SchoolBanner,
    ProfileImage,
    AdmissionApplicationDocument,
    Transcript,
    Certificate,
    IdentityCard,
    QuestionBankImage,
    CourseMaterial,
    AssignmentAttachment,
    GenericPrivateDocument,
}

impl FilePurpose {
    pub const ALL: [Self; 11] = [
        Self::SchoolLogo,
        Self::SchoolBanner,
        Self::ProfileImage,
        Self::AdmissionApplicationDocument,
        Self::Transcript,
        Self::Certificate,
        Self::IdentityCard,
        Self::QuestionBankImage,
        Self::CourseMaterial,
        Self::AssignmentAttachment,
        Self::GenericPrivateDocument,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::SchoolLogo => "school_logo",
            Self::SchoolBanner => "school_banner",
            Self::ProfileImage => "profile_image",
            Self::AdmissionApplicationDocument => "admission_application_document",
            Self::Transcript => "transcript",
            Self::Certificate => "certificate",
            Self::IdentityCard => "identity_card",
            Self::QuestionBankImage => "question_bank_image",
            Self::CourseMaterial => "course_material",
            Self::AssignmentAttachment => "assignment_attachment",
            Self::GenericPrivateDocument => "generic_private_document",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileVisibility {
    Public,
    Private,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageClass {
    Public,
    Private,
}

impl From<FileVisibility> for StorageClass {
    fn from(value: FileVisibility) -> Self {
        match value {
            FileVisibility::Public => Self::Public,
            FileVisibility::Private => Self::Private,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileLifecycleStatus {
    Pending,
    Processing,
    Ready,
    DeleteRequested,
    Deleted,
    Failed,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectedContent {
    Jpeg,
    Png,
    Webp,
    Pdf,
}

impl DetectedContent {
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
            Self::Pdf => "application/pdf",
        }
    }

    pub const fn canonical_extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivativeRecipe {
    Thumbnail256Webp,
    Thumbnail1024Webp,
}

impl DerivativeRecipe {
    pub const fn variant(self) -> &'static str {
        match self {
            Self::Thumbnail256Webp => "thumbnail-256",
            Self::Thumbnail1024Webp => "thumbnail-1024",
        }
    }

    pub const fn detected_content(self) -> DetectedContent {
        DetectedContent::Webp
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionClass {
    Standard,
    Temporary,
    LegalHold,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanRequirement {
    RequiredClean,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderObjectReference {
    pub provider_code: String,
    pub object_key: ObjectKey,
}

impl ProviderObjectReference {
    pub const fn storage_class(&self) -> StorageClass {
        self.object_key.storage_class()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DownloadGrant {
    Redirect {
        location: String,
        expires_at: DateTime<Utc>,
    },
    Stream {
        content_type: String,
        content_length: Option<u64>,
    },
}

/// The only client-selectable file-platform property is an approved purpose.
/// Uploaded bytes and a submitted filename are carried by multipart transport, not this DTO.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileUploadRequest {
    pub purpose: FilePurpose,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn upload_request_exposes_only_client_selectable_purpose() {
        let request = FileUploadRequest {
            purpose: FilePurpose::ProfileImage,
        };

        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({"purpose": "profile_image"})
        );
        for server_owned_property in [
            "provider",
            "providerCode",
            "bucket",
            "bucketName",
            "visibility",
            "storageClass",
            "objectKey",
            "key",
            "owner",
            "ownerUserId",
            "resourceOwner",
            "createdBy",
            "lifecycle",
            "lifecycleStatus",
            "scanPolicy",
            "scanRequirement",
            "scanStatus",
            "retention",
            "retentionClass",
        ] {
            let mut request = serde_json::json!({"purpose": "profile_image"});
            request[server_owned_property] = json!("client-controlled");
            assert!(
                serde_json::from_value::<FileUploadRequest>(request).is_err(),
                "server-owned property {server_owned_property:?} must not be accepted"
            );
        }
    }

    #[test]
    fn detected_content_owns_the_canonical_extension() {
        assert_eq!(DetectedContent::Jpeg.canonical_extension(), "jpg");
        assert_eq!(DetectedContent::Png.canonical_extension(), "png");
        assert_eq!(DetectedContent::Webp.canonical_extension(), "webp");
        assert_eq!(DetectedContent::Pdf.canonical_extension(), "pdf");
        assert_eq!(DetectedContent::Pdf.mime_type(), "application/pdf");
    }

    #[test]
    fn provider_references_and_download_grants_stay_provider_neutral() {
        let object_key = crate::modules::files::purpose_registry::original_object_key(
            uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            FilePurpose::ProfileImage,
            uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            1,
            DetectedContent::Png,
        )
        .expect("registry must create provider object keys");
        let reference = ProviderObjectReference {
            provider_code: "r2".to_string(),
            object_key,
        };
        assert_eq!(
            reference.object_key.as_str(),
            "tenants/11111111-1111-1111-1111-111111111111/identity/profile-image/22222222-2222-2222-2222-222222222222/v1/original.png"
        );
        assert_eq!(reference.storage_class(), StorageClass::Private);

        let grant = DownloadGrant::Redirect {
            location: "https://provider.example/download".to_string(),
            expires_at: chrono::Utc::now(),
        };
        assert!(matches!(grant, DownloadGrant::Redirect { .. }));
        let stream_grant = DownloadGrant::Stream {
            content_type: "application/pdf".to_string(),
            content_length: Some(64),
        };
        assert!(matches!(stream_grant, DownloadGrant::Stream { .. }));

        let lifecycle_states = [
            FileLifecycleStatus::Pending,
            FileLifecycleStatus::Processing,
            FileLifecycleStatus::Ready,
            FileLifecycleStatus::DeleteRequested,
            FileLifecycleStatus::Deleted,
            FileLifecycleStatus::Failed,
            FileLifecycleStatus::Quarantined,
        ];
        assert_eq!(lifecycle_states.len(), 7);
    }
}
