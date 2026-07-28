use std::fmt;

use uuid::Uuid;

use super::platform_types::{
    DerivativeRecipe, DetectedContent, FilePurpose, FileVisibility, RetentionClass,
    ScanRequirement, StorageClass,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyKey {
    SchoolBranding,
    ProfileImage,
    AdmissionApplicationDocument,
    IdentityDocument,
    QuestionBankImage,
    CourseworkAttachment,
    ExplicitOwningResource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentLimits {
    pub max_bytes: u64,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub max_decoded_pixels: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PurposeDefinition {
    pub domain_segment: &'static str,
    pub purpose_segment: &'static str,
    pub visibility: FileVisibility,
    pub allowed_content: &'static [DetectedContent],
    pub limits: ContentLimits,
    pub scan_requirement: ScanRequirement,
    pub derivatives: &'static [DerivativeRecipe],
    pub retention_class: RetentionClass,
    pub policy_key: PolicyKey,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PurposeRegistryError {
    UnknownPurposeCode,
    UnsupportedDetectedContent,
    InvalidVersion,
    DerivativeNotAllowed,
    InvalidPersistedObjectKey,
}

/// Immutable storage identity constructed only by the purpose registry.
/// Its tuple field remains private so other modules cannot inject raw key text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectKey(String, StorageClass);

impl ObjectKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub const fn storage_class(&self) -> StorageClass {
        self.1
    }
}

impl fmt::Display for PurposeRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownPurposeCode => "unknown file purpose",
            Self::UnsupportedDetectedContent => "detected content is not allowed for this purpose",
            Self::InvalidVersion => "file version must be positive",
            Self::DerivativeNotAllowed => "derivative is not allowed for this purpose",
            Self::InvalidPersistedObjectKey => "persisted file object key is invalid",
        })
    }
}

impl std::error::Error for PurposeRegistryError {}

const IMAGE_CONTENT: &[DetectedContent] = &[
    DetectedContent::Jpeg,
    DetectedContent::Png,
    DetectedContent::Webp,
];
const PDF_CONTENT: &[DetectedContent] = &[DetectedContent::Pdf];
const ADMISSION_CONTENT: &[DetectedContent] = &[
    DetectedContent::Jpeg,
    DetectedContent::Png,
    DetectedContent::Pdf,
];
const THUMBNAIL_256: &[DerivativeRecipe] = &[DerivativeRecipe::Thumbnail256Webp];
const THUMBNAIL_1024: &[DerivativeRecipe] = &[DerivativeRecipe::Thumbnail1024Webp];

const fn image_limits(max_bytes: u64, max_width: u32, max_height: u32) -> ContentLimits {
    ContentLimits {
        max_bytes,
        max_width: Some(max_width),
        max_height: Some(max_height),
        max_decoded_pixels: Some(max_width as u64 * max_height as u64),
    }
}

const fn document_limits(max_bytes: u64) -> ContentLimits {
    ContentLimits {
        max_bytes,
        max_width: None,
        max_height: None,
        max_decoded_pixels: None,
    }
}

pub fn purpose_from_code(code: &str) -> Result<FilePurpose, PurposeRegistryError> {
    FilePurpose::ALL
        .into_iter()
        .find(|purpose| purpose.code() == code)
        .ok_or(PurposeRegistryError::UnknownPurposeCode)
}

pub fn purpose_definition(purpose: FilePurpose) -> Result<PurposeDefinition, PurposeRegistryError> {
    let definition = match purpose {
        FilePurpose::SchoolLogo => PurposeDefinition {
            domain_segment: "school",
            purpose_segment: "logo",
            visibility: FileVisibility::Public,
            allowed_content: IMAGE_CONTENT,
            limits: image_limits(2 * 1024 * 1024, 2048, 2048),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: THUMBNAIL_256,
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::SchoolBranding,
        },
        FilePurpose::SchoolBanner => PurposeDefinition {
            domain_segment: "school",
            purpose_segment: "banner",
            visibility: FileVisibility::Public,
            allowed_content: IMAGE_CONTENT,
            limits: image_limits(5 * 1024 * 1024, 4096, 2048),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: THUMBNAIL_1024,
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::SchoolBranding,
        },
        FilePurpose::ProfileImage => PurposeDefinition {
            domain_segment: "identity",
            purpose_segment: "profile-image",
            visibility: FileVisibility::Private,
            allowed_content: IMAGE_CONTENT,
            limits: image_limits(5 * 1024 * 1024, 2048, 2048),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: THUMBNAIL_256,
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::ProfileImage,
        },
        FilePurpose::AdmissionApplicationDocument => PurposeDefinition {
            domain_segment: "admission",
            purpose_segment: "application-document",
            visibility: FileVisibility::Private,
            allowed_content: ADMISSION_CONTENT,
            limits: image_limits(20 * 1024 * 1024, 4096, 4096),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: &[],
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::AdmissionApplicationDocument,
        },
        FilePurpose::Transcript => PurposeDefinition {
            domain_segment: "identity",
            purpose_segment: "transcript",
            visibility: FileVisibility::Private,
            allowed_content: PDF_CONTENT,
            limits: document_limits(20 * 1024 * 1024),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: &[],
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::IdentityDocument,
        },
        FilePurpose::Certificate => PurposeDefinition {
            domain_segment: "identity",
            purpose_segment: "certificate",
            visibility: FileVisibility::Private,
            allowed_content: PDF_CONTENT,
            limits: document_limits(20 * 1024 * 1024),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: &[],
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::IdentityDocument,
        },
        FilePurpose::IdentityCard => PurposeDefinition {
            domain_segment: "identity",
            purpose_segment: "id-card",
            visibility: FileVisibility::Private,
            allowed_content: PDF_CONTENT,
            limits: document_limits(5 * 1024 * 1024),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: &[],
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::IdentityDocument,
        },
        FilePurpose::QuestionBankImage => PurposeDefinition {
            domain_segment: "question-bank",
            purpose_segment: "image",
            visibility: FileVisibility::Private,
            allowed_content: IMAGE_CONTENT,
            limits: image_limits(10 * 1024 * 1024, 4096, 4096),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: THUMBNAIL_1024,
            retention_class: RetentionClass::Temporary,
            policy_key: PolicyKey::QuestionBankImage,
        },
        FilePurpose::CourseMaterial => PurposeDefinition {
            domain_segment: "academic",
            purpose_segment: "course-material",
            visibility: FileVisibility::Private,
            allowed_content: PDF_CONTENT,
            limits: document_limits(10 * 1024 * 1024),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: &[],
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::CourseworkAttachment,
        },
        FilePurpose::AssignmentAttachment => PurposeDefinition {
            domain_segment: "academic",
            purpose_segment: "assignment-attachment",
            visibility: FileVisibility::Private,
            allowed_content: PDF_CONTENT,
            limits: document_limits(10 * 1024 * 1024),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: &[],
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::CourseworkAttachment,
        },
        FilePurpose::GenericPrivateDocument => PurposeDefinition {
            domain_segment: "document",
            purpose_segment: "private-document",
            visibility: FileVisibility::Private,
            allowed_content: PDF_CONTENT,
            limits: document_limits(20 * 1024 * 1024),
            scan_requirement: ScanRequirement::RequiredClean,
            derivatives: &[],
            retention_class: RetentionClass::Standard,
            policy_key: PolicyKey::ExplicitOwningResource,
        },
    };

    Ok(definition)
}

pub fn original_object_key(
    tenant_id: Uuid,
    purpose: FilePurpose,
    file_id: Uuid,
    version: u32,
    detected_content: DetectedContent,
) -> Result<ObjectKey, PurposeRegistryError> {
    let definition = purpose_definition(purpose)?;
    if version == 0 {
        return Err(PurposeRegistryError::InvalidVersion);
    }
    if !definition.allowed_content.contains(&detected_content) {
        return Err(PurposeRegistryError::UnsupportedDetectedContent);
    }

    Ok(ObjectKey(
        format!(
            "tenants/{tenant_id}/{}/{}/{file_id}/v{version}/original.{}",
            definition.domain_segment,
            definition.purpose_segment,
            detected_content.canonical_extension(),
        ),
        definition.visibility.into(),
    ))
}

pub fn derivative_object_key(
    tenant_id: Uuid,
    purpose: FilePurpose,
    file_id: Uuid,
    version: u32,
    derivative: DerivativeRecipe,
) -> Result<ObjectKey, PurposeRegistryError> {
    let definition = purpose_definition(purpose)?;
    if version == 0 {
        return Err(PurposeRegistryError::InvalidVersion);
    }
    if !definition.derivatives.contains(&derivative) {
        return Err(PurposeRegistryError::DerivativeNotAllowed);
    }

    Ok(ObjectKey(
        format!(
            "tenants/{tenant_id}/{}/{}/{file_id}/v{version}/derivatives/{}.{}",
            definition.domain_segment,
            definition.purpose_segment,
            derivative.variant(),
            derivative.detected_content().canonical_extension(),
        ),
        definition.visibility.into(),
    ))
}

/// Rehydrates a server-generated key from trusted platform metadata.
///
/// Business modules cannot call this boundary or supply raw keys. The repository
/// uses it only after loading immutable locator columns written by this platform.
pub(crate) fn persisted_object_key(
    value: String,
    storage_class: StorageClass,
) -> Result<ObjectKey, PurposeRegistryError> {
    if value.len() > 1024
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'-')))
    {
        return Err(PurposeRegistryError::InvalidPersistedObjectKey);
    }

    let segments = value.split('/').collect::<Vec<_>>();
    if segments.len() < 7
        || segments[0] != "tenants"
        || segments
            .iter()
            .any(|segment| segment.is_empty() || matches!(*segment, "." | ".."))
        || Uuid::parse_str(segments[1]).is_err()
        || Uuid::parse_str(segments[4]).is_err()
        || !segments[5]
            .strip_prefix('v')
            .is_some_and(|version| version.parse::<u32>().is_ok_and(|version| version > 0))
        || !segments
            .last()
            .is_some_and(|filename| filename.contains('.') && !filename.starts_with('.'))
    {
        return Err(PurposeRegistryError::InvalidPersistedObjectKey);
    }

    Ok(ObjectKey(value, storage_class))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::files::platform_types::{
        DetectedContent, FilePurpose, FileVisibility, RetentionClass, ScanRequirement, StorageClass,
    };
    use uuid::Uuid;

    #[test]
    fn every_initial_purpose_has_only_server_owned_policy_properties() {
        let cases = [
            (
                FilePurpose::SchoolLogo,
                "school",
                "logo",
                FileVisibility::Public,
                &[
                    DetectedContent::Jpeg,
                    DetectedContent::Png,
                    DetectedContent::Webp,
                ][..],
                2 * 1024 * 1024,
                Some(2048),
                Some(2048),
                Some(2048 * 2048),
                &[DerivativeRecipe::Thumbnail256Webp][..],
                PolicyKey::SchoolBranding,
            ),
            (
                FilePurpose::SchoolBanner,
                "school",
                "banner",
                FileVisibility::Public,
                &[
                    DetectedContent::Jpeg,
                    DetectedContent::Png,
                    DetectedContent::Webp,
                ][..],
                5 * 1024 * 1024,
                Some(4096),
                Some(2048),
                Some(4096 * 2048),
                &[DerivativeRecipe::Thumbnail1024Webp][..],
                PolicyKey::SchoolBranding,
            ),
            (
                FilePurpose::ProfileImage,
                "identity",
                "profile-image",
                FileVisibility::Private,
                &[
                    DetectedContent::Jpeg,
                    DetectedContent::Png,
                    DetectedContent::Webp,
                ][..],
                5 * 1024 * 1024,
                Some(2048),
                Some(2048),
                Some(2048 * 2048),
                &[DerivativeRecipe::Thumbnail256Webp][..],
                PolicyKey::ProfileImage,
            ),
            (
                FilePurpose::AdmissionApplicationDocument,
                "admission",
                "application-document",
                FileVisibility::Private,
                &[
                    DetectedContent::Jpeg,
                    DetectedContent::Png,
                    DetectedContent::Pdf,
                ][..],
                20 * 1024 * 1024,
                Some(4096),
                Some(4096),
                Some(4096 * 4096),
                &[],
                PolicyKey::AdmissionApplicationDocument,
            ),
            (
                FilePurpose::Transcript,
                "identity",
                "transcript",
                FileVisibility::Private,
                &[DetectedContent::Pdf][..],
                20 * 1024 * 1024,
                None,
                None,
                None,
                &[],
                PolicyKey::IdentityDocument,
            ),
            (
                FilePurpose::Certificate,
                "identity",
                "certificate",
                FileVisibility::Private,
                &[DetectedContent::Pdf][..],
                20 * 1024 * 1024,
                None,
                None,
                None,
                &[],
                PolicyKey::IdentityDocument,
            ),
            (
                FilePurpose::IdentityCard,
                "identity",
                "id-card",
                FileVisibility::Private,
                &[DetectedContent::Pdf][..],
                5 * 1024 * 1024,
                None,
                None,
                None,
                &[],
                PolicyKey::IdentityDocument,
            ),
            (
                FilePurpose::QuestionBankImage,
                "question-bank",
                "image",
                FileVisibility::Private,
                &[
                    DetectedContent::Jpeg,
                    DetectedContent::Png,
                    DetectedContent::Webp,
                ][..],
                10 * 1024 * 1024,
                Some(4096),
                Some(4096),
                Some(4096 * 4096),
                &[DerivativeRecipe::Thumbnail1024Webp][..],
                PolicyKey::QuestionBankImage,
            ),
            (
                FilePurpose::CourseMaterial,
                "academic",
                "course-material",
                FileVisibility::Private,
                &[DetectedContent::Pdf][..],
                10 * 1024 * 1024,
                None,
                None,
                None,
                &[],
                PolicyKey::CourseworkAttachment,
            ),
            (
                FilePurpose::AssignmentAttachment,
                "academic",
                "assignment-attachment",
                FileVisibility::Private,
                &[DetectedContent::Pdf][..],
                10 * 1024 * 1024,
                None,
                None,
                None,
                &[],
                PolicyKey::CourseworkAttachment,
            ),
            (
                FilePurpose::GenericPrivateDocument,
                "document",
                "private-document",
                FileVisibility::Private,
                &[DetectedContent::Pdf][..],
                20 * 1024 * 1024,
                None,
                None,
                None,
                &[],
                PolicyKey::ExplicitOwningResource,
            ),
        ];

        for (
            purpose,
            domain,
            segment,
            visibility,
            allowed_content,
            max_bytes,
            max_width,
            max_height,
            max_decoded_pixels,
            derivatives,
            policy_key,
        ) in cases
        {
            let definition = purpose_definition(purpose).expect("initial purpose must resolve");

            assert_eq!(definition.domain_segment, domain);
            assert_eq!(definition.purpose_segment, segment);
            assert_eq!(definition.visibility, visibility);
            assert_eq!(definition.allowed_content, allowed_content);
            assert_eq!(definition.limits.max_bytes, max_bytes);
            assert_eq!(definition.limits.max_width, max_width);
            assert_eq!(definition.limits.max_height, max_height);
            assert_eq!(definition.limits.max_decoded_pixels, max_decoded_pixels);
            assert_eq!(definition.scan_requirement, ScanRequirement::RequiredClean);
            assert_eq!(definition.derivatives, derivatives);
            assert_eq!(
                definition.retention_class,
                if purpose == FilePurpose::QuestionBankImage {
                    RetentionClass::Temporary
                } else {
                    RetentionClass::Standard
                }
            );
            assert_eq!(definition.policy_key, policy_key);
        }
    }

    #[test]
    fn unknown_purpose_code_is_rejected() {
        assert!(purpose_from_code("not-an-approved-purpose").is_err());
    }

    #[test]
    fn original_key_uses_registry_segments_stable_tenant_id_and_detected_extension() {
        let tenant_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let file_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let key = original_object_key(
            tenant_id,
            FilePurpose::SchoolLogo,
            file_id,
            1,
            DetectedContent::Webp,
        )
        .expect("valid version must produce an object key");

        assert_eq!(
            key.as_str(),
            "tenants/11111111-1111-1111-1111-111111111111/school/logo/22222222-2222-2222-2222-222222222222/v1/original.webp"
        );
        assert_eq!(key.storage_class(), StorageClass::Public);
    }

    #[test]
    fn registry_key_keeps_its_server_owned_storage_class() {
        let key = original_object_key(
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            FilePurpose::ProfileImage,
            Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            1,
            DetectedContent::Png,
        )
        .unwrap();

        assert_eq!(key.storage_class(), StorageClass::Private);
    }

    #[test]
    fn generated_keys_cannot_include_submitted_or_personal_identifiers() {
        let key = original_object_key(
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            FilePurpose::AdmissionApplicationDocument,
            Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            7,
            DetectedContent::Pdf,
        )
        .expect("valid version must produce an object key");
        let key = key.as_str();

        for forbidden in [
            "Ava Example",
            "student-2026-0007",
            "application-12345",
            "1234567890123",
            "submitted-name.pdf",
            "renamable-subdomain",
        ] {
            assert!(
                !key.contains(forbidden),
                "generated key must not include submitted or personal identifier {forbidden:?}"
            );
        }
        assert!(key.ends_with("/original.pdf"));
    }

    #[test]
    fn derivative_keys_use_registry_variant_and_detected_extension() {
        let key = derivative_object_key(
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            FilePurpose::QuestionBankImage,
            Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            3,
            DerivativeRecipe::Thumbnail1024Webp,
        )
        .expect("approved derivative must produce an object key");

        assert_eq!(
            key.as_str(),
            "tenants/11111111-1111-1111-1111-111111111111/question-bank/image/22222222-2222-2222-2222-222222222222/v3/derivatives/thumbnail-1024.webp"
        );
    }

    #[test]
    fn key_generation_rejects_invalid_versions_and_unapproved_content_or_derivatives() {
        let tenant_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let file_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        assert_eq!(
            original_object_key(
                tenant_id,
                FilePurpose::SchoolLogo,
                file_id,
                0,
                DetectedContent::Png,
            ),
            Err(PurposeRegistryError::InvalidVersion),
        );
        assert_eq!(
            derivative_object_key(
                tenant_id,
                FilePurpose::SchoolLogo,
                file_id,
                0,
                DerivativeRecipe::Thumbnail256Webp,
            ),
            Err(PurposeRegistryError::InvalidVersion),
        );
        assert_eq!(
            original_object_key(
                tenant_id,
                FilePurpose::SchoolLogo,
                file_id,
                1,
                DetectedContent::Pdf,
            ),
            Err(PurposeRegistryError::UnsupportedDetectedContent),
        );
        assert_eq!(
            derivative_object_key(
                tenant_id,
                FilePurpose::QuestionBankImage,
                file_id,
                1,
                DerivativeRecipe::Thumbnail256Webp,
            ),
            Err(PurposeRegistryError::DerivativeNotAllowed),
        );
    }

    #[test]
    fn persisted_keys_accept_only_the_platform_key_shape() {
        let valid_original = "tenants/11111111-1111-1111-1111-111111111111/identity/profile-image/22222222-2222-2222-2222-222222222222/v1/original.png";
        let valid_derivative = "tenants/11111111-1111-1111-1111-111111111111/question-bank/image/22222222-2222-2222-2222-222222222222/v3/derivatives/thumbnail-1024.webp";

        assert_eq!(
            persisted_object_key(valid_original.to_string(), StorageClass::Private)
                .expect("valid original key should rehydrate")
                .storage_class(),
            StorageClass::Private,
        );
        assert_eq!(
            persisted_object_key(valid_derivative.to_string(), StorageClass::Private)
                .expect("valid derivative key should rehydrate")
                .as_str(),
            valid_derivative,
        );

        for invalid in [
            "tenants/not-a-uuid/identity/profile-image/22222222-2222-2222-2222-222222222222/v1/original.png",
            "tenants/11111111-1111-1111-1111-111111111111/identity/profile-image/not-a-uuid/v1/original.png",
            "tenants/11111111-1111-1111-1111-111111111111/identity/profile-image/22222222-2222-2222-2222-222222222222/v0/original.png",
            "tenants/11111111-1111-1111-1111-111111111111/identity/../22222222-2222-2222-2222-222222222222/v1/original.png",
            "tenants/11111111-1111-1111-1111-111111111111/identity/profile_image/22222222-2222-2222-2222-222222222222/v1/original.png",
            "tenants/11111111-1111-1111-1111-111111111111/identity/profile-image/22222222-2222-2222-2222-222222222222/v1/no-extension",
        ] {
            assert_eq!(
                persisted_object_key(invalid.to_string(), StorageClass::Private),
                Err(PurposeRegistryError::InvalidPersistedObjectKey),
                "invalid persisted key should be rejected: {invalid}",
            );
        }
    }
}
