//! Typed certificate-domain contracts shared by handlers and pure services.

use std::{collections::BTreeMap, fmt};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, ToSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }
    };
}

string_enum!(RecipientType {
    Student,
    Staff,
    External,
});
string_enum!(CertificateCampaignStatus {
    Draft,
    Active,
    Closed,
    Archived,
});

impl CertificateCampaignStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Closed => "closed",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "closed" => Some(Self::Closed),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}
string_enum!(CertificateImportSource {
    Xlsx,
    Csv,
    Manual,
    AccountSearch,
    Replacement,
});
string_enum!(CandidateMatchStatus {
    Matched,
    NameMismatch,
    NotFound,
    Inactive,
    ExternalConfirmed,
    NotApplicable,
});
string_enum!(CandidateValidationStatus {
    Ready,
    NeedsReview,
    Invalid,
});
string_enum!(CandidateNameSource { File, Account });
string_enum!(CertificateIssueRequestStatus {
    Pending,
    Reviewing,
    Returned,
    Withdrawn,
    Issued,
});
string_enum!(CertificateIssueRunOutcome { Issued, Returned });
string_enum!(CertificateStatus { Issued, Revoked });
string_enum!(CertificateTemplateAssetKind { Image, Font });

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(transparent)]
pub struct CertificateNumber(pub(super) String);

impl CertificateNumber {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CertificateNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CertificateNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateNumberError {
    InvalidAcademicYear,
    ActivitySequenceOutOfRange,
    CertificateSequenceOutOfRange,
    InvalidFormat,
    InvalidCheckDigit,
}

impl fmt::Display for CertificateNumberError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidAcademicYear => "invalid academic year",
            Self::ActivitySequenceOutOfRange => "activity sequence out of range",
            Self::CertificateSequenceOutOfRange => "certificate sequence out of range",
            Self::InvalidFormat => "invalid certificate number format",
            Self::InvalidCheckDigit => "invalid certificate number check digit",
        };
        formatter.write_str(message)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateLayoutV1 {
    pub schema_version: u16,
    pub elements: Vec<CertificateElement>,
}

impl Default for CertificateLayoutV1 {
    fn default() -> Self {
        Self {
            schema_version: 1,
            elements: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CertificateElement {
    Text(TextElement),
    Image(ImageElement),
    Qr(QrElement),
}

impl CertificateElement {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Text(element) => element.id,
            Self::Image(element) => element.id,
            Self::Qr(element) => element.id,
        }
    }

    pub fn frame(&self) -> ElementFrame {
        match self {
            Self::Text(element) => element.frame,
            Self::Image(element) => element.frame,
            Self::Qr(element) => element.frame,
        }
    }

    pub fn frame_mut(&mut self) -> &mut ElementFrame {
        match self {
            Self::Text(element) => &mut element.frame,
            Self::Image(element) => &mut element.frame,
            Self::Qr(element) => &mut element.frame,
        }
    }

    pub fn rotation(&self) -> f64 {
        match self {
            Self::Text(element) => element.rotation,
            Self::Image(element) => element.rotation,
            Self::Qr(element) => element.rotation,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElementFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextElement {
    pub id: Uuid,
    pub content: String,
    pub frame: ElementFrame,
    pub rotation: f64,
    pub font_source: CertificateFontSource,
    pub font_family: String,
    pub font_weight: u16,
    pub font_size: f64,
    pub min_font_size: f64,
    pub color: String,
    pub alignment: TextAlignment,
    pub line_height: f64,
    pub auto_shrink: bool,
    pub shadow: Option<TextShadow>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CertificateFontSource {
    #[default]
    BuiltIn,
    Asset {
        asset_id: Uuid,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextShadow {
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur: f64,
    pub color: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageElement {
    pub id: Uuid,
    pub frame: ElementFrame,
    pub rotation: f64,
    pub asset_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QrElement {
    pub id: Uuid,
    pub frame: ElementFrame,
    pub rotation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageGeometry {
    source_width: f64,
    source_height: f64,
    rotation: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageGeometryError {
    InvalidDimensions,
    InvalidRotation,
}

impl PageGeometry {
    pub fn new(
        source_width: f64,
        source_height: f64,
        rotation: i16,
    ) -> Result<Self, PageGeometryError> {
        if !source_width.is_finite()
            || !source_height.is_finite()
            || source_width <= 0.0
            || source_height <= 0.0
        {
            return Err(PageGeometryError::InvalidDimensions);
        }
        if !matches!(rotation, 0 | 90 | 180 | 270) {
            return Err(PageGeometryError::InvalidRotation);
        }
        Ok(Self {
            source_width,
            source_height,
            rotation,
        })
    }

    pub fn source_size(self) -> (f64, f64) {
        (self.source_width, self.source_height)
    }

    pub fn displayed_size(self) -> (f64, f64) {
        match self.rotation {
            90 | 270 => (self.source_height, self.source_width),
            _ => (self.source_width, self.source_height),
        }
    }

    pub fn rotation(self) -> i16 {
        self.rotation
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateImportRequest {
    pub source: CertificateImportSource,
    pub headers: Vec<String>,
    pub rows: Vec<CertificateImportRowInput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateImportRowInput {
    pub recipient_type: String,
    pub student_id: Option<String>,
    pub staff_username: Option<String>,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub activity_item: Option<String>,
    pub award_or_role: Option<String>,
    pub template_name: Option<String>,
    #[serde(default)]
    pub custom_values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCertificateCampaignRequest {
    pub academic_year_id: Uuid,
    pub owner_organization_unit_id: Option<Uuid>,
    pub name: String,
    pub event_date: NaiveDate,
}

/// Explicit nullable update wrapper: omitted means unchanged; `{ "value": null }`
/// changes a unit-owned draft into a school-owned draft.
#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NullableUuidUpdate {
    #[schema(required = true)]
    pub value: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCertificateCampaignRequest {
    pub expected_updated_at: DateTime<Utc>,
    pub academic_year_id: Option<Uuid>,
    pub owner_organization_unit_id: Option<NullableUuidUpdate>,
    pub name: Option<String>,
    pub event_date: Option<NaiveDate>,
    #[serde(default)]
    pub confirm_affects_issued_certificates: bool,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeCertificateCampaignStatusRequest {
    pub expected_updated_at: DateTime<Utc>,
    pub status: CertificateCampaignStatus,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignCapabilities {
    pub can_read: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub can_submit: bool,
    pub can_download: bool,
    pub can_change_status: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignSummary {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_value: i32,
    pub academic_year_name: String,
    #[schema(required = true)]
    pub owner_organization_unit_id: Option<Uuid>,
    #[schema(required = true)]
    pub owner_organization_unit_code: Option<String>,
    #[schema(required = true)]
    pub owner_organization_unit_name: Option<String>,
    pub name: String,
    pub event_date: NaiveDate,
    pub status: CertificateCampaignStatus,
    #[schema(required = true)]
    pub activity_sequence: Option<i32>,
    pub template_count: i64,
    pub candidate_count: i64,
    pub issued_certificate_count: i64,
    pub has_open_issue_request: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub capabilities: CertificateCampaignCapabilities,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignDetail {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_value: i32,
    pub academic_year_name: String,
    #[schema(required = true)]
    pub owner_organization_unit_id: Option<Uuid>,
    #[schema(required = true)]
    pub owner_organization_unit_code: Option<String>,
    #[schema(required = true)]
    pub owner_organization_unit_name: Option<String>,
    pub name: String,
    pub event_date: NaiveDate,
    pub status: CertificateCampaignStatus,
    #[schema(required = true)]
    pub activity_sequence: Option<i32>,
    pub next_certificate_sequence: i32,
    pub template_count: i64,
    pub candidate_count: i64,
    pub issued_certificate_count: i64,
    pub has_open_issue_request: bool,
    #[schema(required = true)]
    pub created_by: Option<Uuid>,
    #[schema(required = true)]
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub capabilities: CertificateCampaignCapabilities,
}

#[derive(Clone, Debug, Default, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateCampaignListQuery {
    pub academic_year_id: Option<Uuid>,
    pub status: Option<CertificateCampaignStatus>,
    pub search: Option<String>,
}
