use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FilePurpose {
    SchoolLogo,
    SchoolBanner,
    ProfileImage,
    AchievementImage,
    AdmissionApplicationDocument,
    Transcript,
    Certificate,
    IdentityCard,
    QuestionBankImage,
    CourseMaterial,
    AssignmentAttachment,
    GenericPrivateDocument,
    CertificateTemplateBackground,
    CertificateTemplateImage,
    SchoolFont,
}

impl FilePurpose {
    pub const ALL: [Self; 15] = [
        Self::SchoolLogo,
        Self::SchoolBanner,
        Self::ProfileImage,
        Self::AchievementImage,
        Self::AdmissionApplicationDocument,
        Self::Transcript,
        Self::Certificate,
        Self::IdentityCard,
        Self::QuestionBankImage,
        Self::CourseMaterial,
        Self::AssignmentAttachment,
        Self::GenericPrivateDocument,
        Self::CertificateTemplateBackground,
        Self::CertificateTemplateImage,
        Self::SchoolFont,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::SchoolLogo => "school_logo",
            Self::SchoolBanner => "school_banner",
            Self::ProfileImage => "profile_image",
            Self::AchievementImage => "achievement_image",
            Self::AdmissionApplicationDocument => "admission_application_document",
            Self::Transcript => "transcript",
            Self::Certificate => "certificate",
            Self::IdentityCard => "identity_card",
            Self::QuestionBankImage => "question_bank_image",
            Self::CourseMaterial => "course_material",
            Self::AssignmentAttachment => "assignment_attachment",
            Self::GenericPrivateDocument => "generic_private_document",
            Self::CertificateTemplateBackground => "certificate_template_background",
            Self::CertificateTemplateImage => "certificate_template_image",
            Self::SchoolFont => "school_font",
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
    Ttf,
    Otf,
}

impl DetectedContent {
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
            Self::Pdf => "application/pdf",
            Self::Ttf => "font/ttf",
            Self::Otf => "font/otf",
        }
    }

    pub const fn canonical_extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Pdf => "pdf",
            Self::Ttf => "ttf",
            Self::Otf => "otf",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct PdfPageBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PdfPageBox {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x: normalize_pdf_point(x),
            y: normalize_pdf_point(y),
            width: normalize_pdf_point(width),
            height: normalize_pdf_point(height),
        }
    }
}

fn normalize_pdf_point(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

const fn default_font_weight() -> u16 {
    400
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontInspectionStyle {
    #[default]
    Normal,
    Italic,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileInspectionMetadata {
    #[default]
    Unknown,
    Image {
        width_px: u32,
        height_px: u32,
    },
    Pdf {
        page_count: u32,
        crop_box: PdfPageBox,
        media_box: PdfPageBox,
        rotation: i16,
    },
    Font {
        family_name: Option<String>,
        units_per_em: u16,
        #[serde(default = "default_font_weight")]
        weight: u16,
        #[serde(default)]
        style: FontInspectionStyle,
        #[serde(default)]
        is_variable: bool,
    },
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detected_content_owns_the_canonical_extension() {
        assert_eq!(DetectedContent::Jpeg.canonical_extension(), "jpg");
        assert_eq!(DetectedContent::Png.canonical_extension(), "png");
        assert_eq!(DetectedContent::Webp.canonical_extension(), "webp");
        assert_eq!(DetectedContent::Pdf.canonical_extension(), "pdf");
        assert_eq!(DetectedContent::Pdf.mime_type(), "application/pdf");
        assert_eq!(DetectedContent::Ttf.canonical_extension(), "ttf");
        assert_eq!(DetectedContent::Ttf.mime_type(), "font/ttf");
        assert_eq!(DetectedContent::Otf.canonical_extension(), "otf");
        assert_eq!(DetectedContent::Otf.mime_type(), "font/otf");
    }

    #[test]
    fn inspection_metadata_uses_the_persisted_tagged_shape() {
        assert_eq!(
            serde_json::to_value(FileInspectionMetadata::Unknown).unwrap(),
            json!({"kind": "unknown"})
        );
        assert_eq!(
            serde_json::to_value(FileInspectionMetadata::Pdf {
                page_count: 1,
                crop_box: PdfPageBox::new(18.0, 24.0, 841.89, 595.28),
                media_box: PdfPageBox::new(0.0, 0.0, 900.0, 650.0),
                rotation: 90,
            })
            .unwrap(),
            json!({
                "kind": "pdf",
                "page_count": 1,
                "crop_box": {"x": 18.0, "y": 24.0, "width": 841.89, "height": 595.28},
                "media_box": {"x": 0.0, "y": 0.0, "width": 900.0, "height": 650.0},
                "rotation": 90,
            })
        );
        for purpose in [
            FilePurpose::CertificateTemplateBackground,
            FilePurpose::CertificateTemplateImage,
            FilePurpose::SchoolFont,
        ] {
            assert!(FilePurpose::ALL.contains(&purpose));
        }
        assert!(
            !FilePurpose::ALL
                .iter()
                .any(|purpose| purpose.code() == "certificate_template_font"),
            "the legacy template-owned font purpose must leave the runtime contract"
        );
    }

    #[test]
    fn download_grants_stay_provider_neutral() {
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
