use std::fmt;

use uuid::Uuid;

use super::platform_types::{
    DerivativeRecipe, DetectedContent, FilePurpose, FileVisibility, ObjectKey, RetentionClass,
    ScanRequirement,
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
}

impl fmt::Display for PurposeRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownPurposeCode => "unknown file purpose",
            Self::UnsupportedDetectedContent => "detected content is not allowed for this purpose",
            Self::InvalidVersion => "file version must be positive",
            Self::DerivativeNotAllowed => "derivative is not allowed for this purpose",
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
            allowed_content: PDF_CONTENT,
            limits: document_limits(20 * 1024 * 1024),
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
            retention_class: RetentionClass::Standard,
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

    Ok(ObjectKey::new(format!(
        "tenants/{tenant_id}/{}/{}/{file_id}/v{version}/original.{}",
        definition.domain_segment,
        definition.purpose_segment,
        detected_content.canonical_extension(),
    )))
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

    Ok(ObjectKey::new(format!(
        "tenants/{tenant_id}/{}/{}/{file_id}/v{version}/derivatives/{}.{}",
        definition.domain_segment,
        definition.purpose_segment,
        derivative.variant(),
        derivative.detected_content().canonical_extension(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::files::platform_types::{
        DetectedContent, FilePurpose, FileVisibility, RetentionClass, ScanRequirement,
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
                2 * 1024 * 1024,
                &[DerivativeRecipe::Thumbnail256Webp][..],
                PolicyKey::SchoolBranding,
            ),
            (
                FilePurpose::SchoolBanner,
                "school",
                "banner",
                FileVisibility::Public,
                5 * 1024 * 1024,
                &[DerivativeRecipe::Thumbnail1024Webp][..],
                PolicyKey::SchoolBranding,
            ),
            (
                FilePurpose::ProfileImage,
                "identity",
                "profile-image",
                FileVisibility::Private,
                5 * 1024 * 1024,
                &[DerivativeRecipe::Thumbnail256Webp][..],
                PolicyKey::ProfileImage,
            ),
            (
                FilePurpose::AdmissionApplicationDocument,
                "admission",
                "application-document",
                FileVisibility::Private,
                20 * 1024 * 1024,
                &[],
                PolicyKey::AdmissionApplicationDocument,
            ),
            (
                FilePurpose::Transcript,
                "identity",
                "transcript",
                FileVisibility::Private,
                20 * 1024 * 1024,
                &[],
                PolicyKey::IdentityDocument,
            ),
            (
                FilePurpose::Certificate,
                "identity",
                "certificate",
                FileVisibility::Private,
                20 * 1024 * 1024,
                &[],
                PolicyKey::IdentityDocument,
            ),
            (
                FilePurpose::IdentityCard,
                "identity",
                "id-card",
                FileVisibility::Private,
                5 * 1024 * 1024,
                &[],
                PolicyKey::IdentityDocument,
            ),
            (
                FilePurpose::QuestionBankImage,
                "question-bank",
                "image",
                FileVisibility::Private,
                10 * 1024 * 1024,
                &[DerivativeRecipe::Thumbnail1024Webp][..],
                PolicyKey::QuestionBankImage,
            ),
            (
                FilePurpose::CourseMaterial,
                "academic",
                "course-material",
                FileVisibility::Private,
                10 * 1024 * 1024,
                &[],
                PolicyKey::CourseworkAttachment,
            ),
            (
                FilePurpose::AssignmentAttachment,
                "academic",
                "assignment-attachment",
                FileVisibility::Private,
                10 * 1024 * 1024,
                &[],
                PolicyKey::CourseworkAttachment,
            ),
            (
                FilePurpose::GenericPrivateDocument,
                "document",
                "private-document",
                FileVisibility::Private,
                20 * 1024 * 1024,
                &[],
                PolicyKey::ExplicitOwningResource,
            ),
        ];

        for (purpose, domain, segment, visibility, max_bytes, derivatives, policy_key) in cases {
            let definition = purpose_definition(purpose).expect("initial purpose must resolve");

            assert_eq!(definition.domain_segment, domain);
            assert_eq!(definition.purpose_segment, segment);
            assert_eq!(definition.visibility, visibility);
            assert_eq!(definition.limits.max_bytes, max_bytes);
            assert_eq!(definition.scan_requirement, ScanRequirement::RequiredClean);
            assert_eq!(definition.derivatives, derivatives);
            assert_eq!(definition.retention_class, RetentionClass::Standard);
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
}
