//! Typed certificate-domain contracts shared by handlers and pure services.

use std::{collections::BTreeMap, fmt};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, ToSchema)]
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
    Purging,
});

impl CertificateCampaignStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Closed => "closed",
            Self::Archived => "archived",
            Self::Purging => "purging",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "closed" => Some(Self::Closed),
            "archived" => Some(Self::Archived),
            "purging" => Some(Self::Purging),
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
string_enum!(CandidateValidationCode {
    InvalidRecipientType,
    MissingStudentId,
    MissingStaffUsername,
    UnexpectedInternalLookup,
    MissingFirstName,
    MissingLastName,
    NameTooLong,
    ValueTooLong,
    ForbiddenSensitiveValue,
    AccountNotFound,
    AccountInactive,
    NameSourceRequired,
    TemplateRequired,
    TemplateNotFound,
    TemplateIncompatible,
    TemplateNotReady,
    DuplicateCandidate,
});
string_enum!(CertificateIssueRequestStatus {
    Pending,
    Reviewing,
    Returned,
    Withdrawn,
    Issued,
});
string_enum!(CertificateIssueCode {
    CandidateNotReady,
    AccountStateChanged,
    TemplateNotReady,
    TemplateIncompatible,
    AssetUnavailable,
    CampaignUnavailable,
    ReviewerRequestedChanges,
});
string_enum!(CertificateResourceLockCode { ResourceLocked });
string_enum!(CertificateIssueRunOutcome { Issued, Returned });
string_enum!(CertificateStatus { Issued, Revoked });
string_enum!(CertificateTemplateAssetKind { Image, Font });
string_enum!(CertificateFontStyle { Normal, Italic });
string_enum!(CertificateFontUploadStatus {
    Ready,
    DuplicateSelection,
    DuplicateExisting,
    UnsupportedVariable,
    UnsupportedWeight,
    MissingFamily,
    Unavailable,
});
string_enum!(GeometryAction {
    Preserve,
    Scale,
    Reset
});
string_enum!(CertificatePreviewKind {
    Short,
    Normal,
    Long,
    Candidate,
});
string_enum!(CertificateTemplateDeleteDisposition {
    Deleted,
    Deactivated,
});

impl RecipientType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Student => "student",
            Self::Staff => "staff",
            Self::External => "external",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "student" => Some(Self::Student),
            "staff" => Some(Self::Staff),
            "external" => Some(Self::External),
            _ => None,
        }
    }
}

impl CertificateImportSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Csv => "csv",
            Self::Manual => "manual",
            Self::AccountSearch => "account_search",
            Self::Replacement => "replacement",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "xlsx" => Some(Self::Xlsx),
            "csv" => Some(Self::Csv),
            "manual" => Some(Self::Manual),
            "account_search" => Some(Self::AccountSearch),
            "replacement" => Some(Self::Replacement),
            _ => None,
        }
    }
}

impl CandidateMatchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::NameMismatch => "name_mismatch",
            Self::NotFound => "not_found",
            Self::Inactive => "inactive",
            Self::ExternalConfirmed => "external_confirmed",
            Self::NotApplicable => "not_applicable",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "matched" => Some(Self::Matched),
            "name_mismatch" => Some(Self::NameMismatch),
            "not_found" => Some(Self::NotFound),
            "inactive" => Some(Self::Inactive),
            "external_confirmed" => Some(Self::ExternalConfirmed),
            "not_applicable" => Some(Self::NotApplicable),
            _ => None,
        }
    }
}

impl CandidateValidationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NeedsReview => "needs_review",
            Self::Invalid => "invalid",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ready" => Some(Self::Ready),
            "needs_review" => Some(Self::NeedsReview),
            "invalid" => Some(Self::Invalid),
            _ => None,
        }
    }
}

impl CandidateNameSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Account => "account",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "file" => Some(Self::File),
            "account" => Some(Self::Account),
            _ => None,
        }
    }
}

impl CandidateValidationCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRecipientType => "invalid_recipient_type",
            Self::MissingStudentId => "missing_student_id",
            Self::MissingStaffUsername => "missing_staff_username",
            Self::UnexpectedInternalLookup => "unexpected_internal_lookup",
            Self::MissingFirstName => "missing_first_name",
            Self::MissingLastName => "missing_last_name",
            Self::NameTooLong => "name_too_long",
            Self::ValueTooLong => "value_too_long",
            Self::ForbiddenSensitiveValue => "forbidden_sensitive_value",
            Self::AccountNotFound => "account_not_found",
            Self::AccountInactive => "account_inactive",
            Self::NameSourceRequired => "name_source_required",
            Self::TemplateRequired => "template_required",
            Self::TemplateNotFound => "template_not_found",
            Self::TemplateIncompatible => "template_incompatible",
            Self::TemplateNotReady => "template_not_ready",
            Self::DuplicateCandidate => "duplicate_candidate",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "invalid_recipient_type" => Some(Self::InvalidRecipientType),
            "missing_student_id" => Some(Self::MissingStudentId),
            "missing_staff_username" => Some(Self::MissingStaffUsername),
            "unexpected_internal_lookup" => Some(Self::UnexpectedInternalLookup),
            "missing_first_name" => Some(Self::MissingFirstName),
            "missing_last_name" => Some(Self::MissingLastName),
            "name_too_long" => Some(Self::NameTooLong),
            "value_too_long" => Some(Self::ValueTooLong),
            "forbidden_sensitive_value" => Some(Self::ForbiddenSensitiveValue),
            "account_not_found" => Some(Self::AccountNotFound),
            "account_inactive" => Some(Self::AccountInactive),
            "name_source_required" => Some(Self::NameSourceRequired),
            "template_required" => Some(Self::TemplateRequired),
            "template_not_found" => Some(Self::TemplateNotFound),
            "template_incompatible" => Some(Self::TemplateIncompatible),
            "template_not_ready" => Some(Self::TemplateNotReady),
            "duplicate_candidate" => Some(Self::DuplicateCandidate),
            _ => None,
        }
    }
}

impl CertificateIssueRequestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Reviewing => "reviewing",
            Self::Returned => "returned",
            Self::Withdrawn => "withdrawn",
            Self::Issued => "issued",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "reviewing" => Some(Self::Reviewing),
            "returned" => Some(Self::Returned),
            "withdrawn" => Some(Self::Withdrawn),
            "issued" => Some(Self::Issued),
            _ => None,
        }
    }
}

impl CertificateIssueCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CandidateNotReady => "candidate_not_ready",
            Self::AccountStateChanged => "account_state_changed",
            Self::TemplateNotReady => "template_not_ready",
            Self::TemplateIncompatible => "template_incompatible",
            Self::AssetUnavailable => "asset_unavailable",
            Self::CampaignUnavailable => "campaign_unavailable",
            Self::ReviewerRequestedChanges => "reviewer_requested_changes",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "candidate_not_ready" => Some(Self::CandidateNotReady),
            "account_state_changed" => Some(Self::AccountStateChanged),
            "template_not_ready" => Some(Self::TemplateNotReady),
            "template_incompatible" => Some(Self::TemplateIncompatible),
            "asset_unavailable" => Some(Self::AssetUnavailable),
            "campaign_unavailable" => Some(Self::CampaignUnavailable),
            "reviewer_requested_changes" => Some(Self::ReviewerRequestedChanges),
            _ => None,
        }
    }
}

impl CertificateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Issued => "issued",
            Self::Revoked => "revoked",
        }
    }
}

impl CertificateTemplateAssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Font => "font",
        }
    }
}

impl CertificateFontStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "normal" => Some(Self::Normal),
            "italic" => Some(Self::Italic),
            _ => None,
        }
    }
}

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
    pub font_style: CertificateFontStyle,
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
    pub lock_aspect_ratio: bool,
    pub aspect_ratio: f64,
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

#[derive(Clone, Debug, Default, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateCandidateListQuery {
    pub status: Option<CandidateValidationStatus>,
    pub template_id: Option<Uuid>,
    pub search: Option<String>,
}

#[derive(Clone, Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateAccountSearchQuery {
    pub recipient_type: RecipientType,
    pub search: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCandidateCapabilities {
    pub can_update: bool,
    pub can_delete: bool,
    pub can_choose_name: bool,
    pub can_confirm_external: bool,
    pub can_confirm_duplicate: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCandidateDetail {
    pub id: Uuid,
    pub campaign_id: Uuid,
    #[schema(required = true)]
    pub batch_id: Option<Uuid>,
    #[schema(required = true)]
    pub template_id: Option<Uuid>,
    #[schema(required = true)]
    pub template_name: Option<String>,
    pub recipient_type: RecipientType,
    #[schema(required = true)]
    pub matched_user_id: Option<Uuid>,
    #[schema(required = true)]
    pub student_id: Option<String>,
    #[schema(required = true)]
    pub staff_username: Option<String>,
    #[schema(required = true)]
    pub imported_title: Option<String>,
    pub imported_first_name: String,
    pub imported_last_name: String,
    #[schema(required = true)]
    pub account_title: Option<String>,
    #[schema(required = true)]
    pub account_first_name: Option<String>,
    #[schema(required = true)]
    pub account_last_name: Option<String>,
    #[schema(required = true)]
    pub selected_name_source: Option<CandidateNameSource>,
    #[schema(required = true)]
    pub activity_item: Option<String>,
    #[schema(required = true)]
    pub award_or_role: Option<String>,
    pub custom_values: BTreeMap<String, String>,
    pub match_status: CandidateMatchStatus,
    pub validation_status: CandidateValidationStatus,
    pub validation_codes: Vec<CandidateValidationCode>,
    pub duplicate_confirmed: bool,
    #[schema(required = true)]
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub capabilities: CertificateCandidateCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCandidateSummary {
    pub total_count: i64,
    pub ready_count: i64,
    pub review_count: i64,
    pub invalid_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCandidateListResponse {
    pub items: Vec<CertificateCandidateDetail>,
    pub summary: CertificateCandidateSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateImportBatchSummary {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub source: CertificateImportSource,
    pub row_count: i32,
    pub custom_headers: Vec<String>,
    pub ready_count: i32,
    pub review_count: i32,
    pub invalid_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCandidateImportResult {
    pub batch: CertificateImportBatchSummary,
    pub candidates: Vec<CertificateCandidateDetail>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCandidateAccount {
    pub user_id: Uuid,
    pub recipient_type: RecipientType,
    #[schema(required = true)]
    pub student_id: Option<String>,
    #[schema(required = true)]
    pub staff_username: Option<String>,
    #[schema(required = true)]
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateManualExternalCandidateRequest {
    pub template_id: Option<Uuid>,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub activity_item: Option<String>,
    pub award_or_role: Option<String>,
    #[serde(default)]
    pub custom_values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAccountCertificateCandidateRequest {
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub activity_item: Option<String>,
    pub award_or_role: Option<String>,
    #[serde(default)]
    pub custom_values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCertificateCandidateRequest {
    pub expected_updated_at: DateTime<Utc>,
    pub template_id: Option<Uuid>,
    pub recipient_type: RecipientType,
    pub student_id: Option<String>,
    pub staff_username: Option<String>,
    pub imported_title: Option<String>,
    pub imported_first_name: String,
    pub imported_last_name: String,
    pub selected_name_source: Option<CandidateNameSource>,
    pub activity_item: Option<String>,
    pub award_or_role: Option<String>,
    #[serde(default)]
    pub custom_values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(
    tag = "operation",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CertificateCandidateBulkRequest {
    AssignTemplate {
        #[serde(rename = "candidateIds")]
        #[schema(rename = "candidateIds")]
        candidate_ids: Vec<Uuid>,
        #[serde(rename = "templateId")]
        #[schema(rename = "templateId")]
        template_id: Uuid,
    },
    ChooseName {
        #[serde(rename = "candidateIds")]
        #[schema(rename = "candidateIds")]
        candidate_ids: Vec<Uuid>,
        #[serde(rename = "nameSource")]
        #[schema(rename = "nameSource")]
        name_source: CandidateNameSource,
    },
    ConfirmExternal {
        #[serde(rename = "candidateIds")]
        #[schema(rename = "candidateIds")]
        candidate_ids: Vec<Uuid>,
    },
    ConfirmDuplicate {
        #[serde(rename = "candidateIds")]
        #[schema(rename = "candidateIds")]
        candidate_ids: Vec<Uuid>,
    },
    SoftDelete {
        #[serde(rename = "candidateIds")]
        #[schema(rename = "candidateIds")]
        candidate_ids: Vec<Uuid>,
    },
}

impl CertificateCandidateBulkRequest {
    pub fn candidate_ids(&self) -> &[Uuid] {
        match self {
            Self::AssignTemplate { candidate_ids, .. }
            | Self::ChooseName { candidate_ids, .. }
            | Self::ConfirmExternal { candidate_ids }
            | Self::ConfirmDuplicate { candidate_ids }
            | Self::SoftDelete { candidate_ids } => candidate_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCandidateBulkResult {
    pub updated_count: u32,
    pub candidates: Vec<CertificateCandidateDetail>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmitCertificateIssueRequest {
    pub candidate_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReturnCertificateIssueRequest {
    pub issue_codes: Vec<CertificateIssueCode>,
    pub return_note: String,
}

#[derive(Clone, Debug, Default, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateIssueRequestListQuery {
    pub status: Option<CertificateIssueRequestStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateIssueRequestCapabilities {
    pub can_withdraw: bool,
    pub can_start_review: bool,
    pub can_return: bool,
    pub can_issue: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateIssueRequestSummary {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub campaign_name: String,
    #[schema(required = true)]
    pub owner_organization_unit_id: Option<Uuid>,
    #[schema(required = true)]
    pub owner_organization_unit_name: Option<String>,
    pub status: CertificateIssueRequestStatus,
    pub submitted_by: Uuid,
    pub submitted_by_name: String,
    #[schema(required = true)]
    pub reviewed_by: Option<Uuid>,
    #[schema(required = true)]
    pub reviewed_by_name: Option<String>,
    pub submitted_at: DateTime<Utc>,
    #[schema(required = true)]
    pub reviewed_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub returned_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub withdrawn_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub issued_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub return_note: Option<String>,
    pub issue_codes: Vec<CertificateIssueCode>,
    pub item_count: i64,
    pub template_count: i64,
    pub ready_count: i64,
    pub review_count: i64,
    pub invalid_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub capabilities: CertificateIssueRequestCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateIssueRequestItem {
    pub candidate_id: Uuid,
    #[schema(required = true)]
    pub template_id: Option<Uuid>,
    #[schema(required = true)]
    pub template_name: Option<String>,
    pub recipient_type: RecipientType,
    #[schema(required = true)]
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub activity_item: Option<String>,
    #[schema(required = true)]
    pub award_or_role: Option<String>,
    pub validation_status: CandidateValidationStatus,
    pub validation_codes: Vec<CandidateValidationCode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateIssueRequestDetail {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub campaign_name: String,
    #[schema(required = true)]
    pub owner_organization_unit_id: Option<Uuid>,
    #[schema(required = true)]
    pub owner_organization_unit_name: Option<String>,
    pub status: CertificateIssueRequestStatus,
    pub submitted_by: Uuid,
    pub submitted_by_name: String,
    #[schema(required = true)]
    pub reviewed_by: Option<Uuid>,
    #[schema(required = true)]
    pub reviewed_by_name: Option<String>,
    pub submitted_at: DateTime<Utc>,
    #[schema(required = true)]
    pub reviewed_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub returned_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub withdrawn_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub issued_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub return_note: Option<String>,
    pub issue_codes: Vec<CertificateIssueCode>,
    pub item_count: i64,
    pub template_count: i64,
    pub ready_count: i64,
    pub review_count: i64,
    pub invalid_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub capabilities: CertificateIssueRequestCapabilities,
    pub items: Vec<CertificateIssueRequestItem>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateResourceLocked {
    pub code: CertificateResourceLockCode,
    #[schema(required = true)]
    pub request_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IssueCertificateRequest {
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCapabilities {
    pub can_read: bool,
    pub can_download: bool,
    pub can_revoke: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssuedCertificateSummary {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub campaign_name: String,
    #[schema(required = true)]
    pub owner_organization_unit_id: Option<Uuid>,
    #[schema(required = true)]
    pub owner_organization_unit_name: Option<String>,
    pub template_id: Uuid,
    pub template_name: String,
    pub academic_year_id: Uuid,
    pub academic_year_value: i32,
    pub activity_sequence: i32,
    pub certificate_sequence: i32,
    pub certificate_number: String,
    pub recipient_type: RecipientType,
    #[schema(required = true)]
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub activity_item: Option<String>,
    #[schema(required = true)]
    pub award_or_role: Option<String>,
    pub issue_date: NaiveDate,
    pub status: CertificateStatus,
    #[schema(required = true)]
    pub replacement_for_certificate_id: Option<Uuid>,
    #[schema(required = true)]
    pub replaced_by_certificate_id: Option<Uuid>,
    #[schema(required = true)]
    pub replacement_candidate_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub capabilities: CertificateCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssuedCertificateDetail {
    #[serde(flatten)]
    pub summary: IssuedCertificateSummary,
    pub issue_run_id: Uuid,
    pub custom_values: BTreeMap<String, String>,
    pub school_name: String,
    #[schema(required = true)]
    pub owner_organization_unit_name_snapshot: Option<String>,
    #[schema(required = true)]
    pub revoked_by: Option<Uuid>,
    #[schema(required = true)]
    pub revoked_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub revocation_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateIssueCandidateProblem {
    pub candidate_id: Uuid,
    pub issue_codes: Vec<CertificateIssueCode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum IssueCertificateOutcome {
    Issued {
        #[serde(rename = "issueRunId")]
        #[schema(rename = "issueRunId")]
        issue_run_id: Uuid,
        #[serde(rename = "requestId")]
        #[schema(rename = "requestId")]
        request_id: Uuid,
        #[serde(rename = "campaignId")]
        #[schema(rename = "campaignId")]
        campaign_id: Uuid,
        #[serde(rename = "activitySequence")]
        #[schema(rename = "activitySequence")]
        activity_sequence: i32,
        #[serde(rename = "firstCertificateSequence")]
        #[schema(rename = "firstCertificateSequence")]
        first_certificate_sequence: i32,
        #[serde(rename = "lastCertificateSequence")]
        #[schema(rename = "lastCertificateSequence")]
        last_certificate_sequence: i32,
        certificates: Vec<IssuedCertificateSummary>,
    },
    Returned {
        #[serde(rename = "issueRunId")]
        #[schema(rename = "issueRunId")]
        issue_run_id: Uuid,
        #[serde(rename = "requestId")]
        #[schema(rename = "requestId")]
        request_id: Uuid,
        #[serde(rename = "campaignId")]
        #[schema(rename = "campaignId")]
        campaign_id: Uuid,
        #[serde(rename = "issueCodes")]
        #[schema(rename = "issueCodes")]
        issue_codes: Vec<CertificateIssueCode>,
        #[serde(rename = "candidateProblems")]
        #[schema(rename = "candidateProblems")]
        candidate_problems: Vec<CertificateIssueCandidateProblem>,
    },
}

#[derive(Clone, Debug, Default, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IssuedCertificateListQuery {
    pub status: Option<CertificateStatus>,
    pub template_id: Option<Uuid>,
    pub search: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevokeCertificateRequest {
    pub reason: String,
    pub create_replacement_candidate: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateReplacementCandidate {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub template_id: Uuid,
    pub validation_status: CandidateValidationStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RevokeCertificateResult {
    pub certificate: IssuedCertificateDetail,
    #[schema(required = true)]
    pub replacement_candidate: Option<CertificateReplacementCandidate>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualCertificateVerificationRequest {
    pub certificate_number: String,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QrCertificateVerificationRequest {
    pub certificate_number: String,
    pub proof: String,
}

#[derive(Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicCertificateRenderRequest {
    pub receipt: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicCertificateVerificationData {
    pub status: CertificateStatus,
    pub certificate_number: String,
    #[schema(required = true)]
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub campaign_name: String,
    pub academic_year: i32,
    pub template_name: String,
    #[schema(required = true)]
    pub activity_item: Option<String>,
    #[schema(required = true)]
    pub award_or_role: Option<String>,
    pub issue_date: NaiveDate,
    pub issuer_school_name: String,
    #[schema(required = true)]
    pub replacement_certificate_number: Option<String>,
    #[schema(required = true)]
    pub receipt: Option<String>,
    #[schema(required = true)]
    pub receipt_expires_at: Option<DateTime<Utc>>,
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
    pub can_prepare_candidates: bool,
    pub can_delete: bool,
    pub can_submit: bool,
    pub can_download: bool,
    pub can_change_status: bool,
    pub can_manage_templates: bool,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateCampaignPurgeCounts {
    pub template_count: i64,
    pub candidate_count: i64,
    pub request_count: i64,
    pub open_request_count: i64,
    pub issued_certificate_count: i64,
    pub revoked_certificate_count: i64,
    pub file_count: i64,
    pub total_file_bytes: i64,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartCertificateCampaignPurgeRequest {
    pub confirmation_name: String,
    pub expected_updated_at: DateTime<Utc>,
    pub expected_impact: CertificateCampaignPurgeCounts,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignPurgeImpact {
    pub campaign_id: Uuid,
    pub campaign_name: String,
    pub updated_at: DateTime<Utc>,
    pub counts: CertificateCampaignPurgeCounts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CertificateCampaignPurgePhase {
    DeletingFiles,
    Failed,
    Finalizing,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignPurgeStatus {
    pub campaign_id: Uuid,
    pub phase: CertificateCampaignPurgePhase,
    pub file_count: i64,
    pub deleted_file_count: i64,
    #[schema(required = true)]
    pub last_error_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCertificateTemplateRequest {
    pub name: String,
    pub allowed_recipient_types: Vec<RecipientType>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCertificateTemplateRequest {
    pub expected_updated_at: DateTime<Utc>,
    pub name: Option<String>,
    pub allowed_recipient_types: Option<Vec<RecipientType>>,
    pub safe_margin_points: Option<f64>,
    pub show_safe_area: Option<bool>,
    pub layout: Option<CertificateLayoutV1>,
    pub is_active: Option<bool>,
    #[serde(default)]
    pub confirm_missing_issued_values: bool,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachCertificateBackgroundRequest {
    pub file_id: Uuid,
    pub geometry_action: GeometryAction,
    pub preview_confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachCertificateAssetRequest {
    pub file_id: Uuid,
    pub kind: CertificateTemplateAssetKind,
    pub display_name: String,
    #[serde(default)]
    pub rights_confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectCertificateFontUploadsRequest {
    pub file_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachCertificateFontBatchRequest {
    pub file_ids: Vec<Uuid>,
    #[serde(default)]
    pub rights_confirmed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateFontUploadInspectionFile {
    pub file_id: Uuid,
    pub display_filename: String,
    #[schema(required = true)]
    pub font_family: Option<String>,
    #[schema(required = true)]
    pub font_weight: Option<u16>,
    #[schema(required = true)]
    pub font_style: Option<CertificateFontStyle>,
    pub status: CertificateFontUploadStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateFontUploadInspection {
    pub files: Vec<CertificateFontUploadInspectionFile>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificatePageBox {
    pub x_points: f64,
    pub y_points: f64,
    pub width_points: f64,
    pub height_points: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificatePageGeometry {
    pub crop_box: CertificatePageBox,
    pub media_box: CertificatePageBox,
    pub rotation: i16,
    pub displayed_width_points: f64,
    pub displayed_height_points: f64,
    pub paper_label: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateTemplateCapabilities {
    pub can_read: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub can_preview: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateTemplateAsset {
    pub id: Uuid,
    pub file_id: Uuid,
    pub kind: CertificateTemplateAssetKind,
    pub display_name: String,
    #[schema(required = true)]
    pub font_family: Option<String>,
    #[schema(required = true)]
    pub font_weight: Option<u16>,
    #[schema(required = true)]
    pub font_style: Option<CertificateFontStyle>,
    #[schema(required = true)]
    pub image_width_pixels: Option<u32>,
    #[schema(required = true)]
    pub image_height_pixels: Option<u32>,
    pub rights_confirmed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateTemplateDetail {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub background_file_id: Option<Uuid>,
    #[schema(required = true)]
    pub page_geometry: Option<CertificatePageGeometry>,
    pub safe_margin_points: f64,
    pub show_safe_area: bool,
    pub allowed_recipient_types: Vec<RecipientType>,
    pub layout: CertificateLayoutV1,
    pub assets: Vec<CertificateTemplateAsset>,
    pub is_active: bool,
    pub is_ready: bool,
    pub issued_certificate_count: i64,
    pub missing_variable_certificate_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub capabilities: CertificateTemplateCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateTemplateDeleteResult {
    pub disposition: CertificateTemplateDeleteDisposition,
    pub detached_file_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateTemplateVariableCatalog {
    pub variables: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificatePreviewManifestRequest {
    pub preview_kind: CertificatePreviewKind,
    pub candidate_id: Option<Uuid>,
    #[serde(default)]
    pub sample_values: BTreeMap<String, String>,
    #[schema(required = false)]
    pub layout: Option<CertificateLayoutV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateRenderFileGrant {
    pub file_id: Uuid,
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateBuiltInFont {
    pub family: String,
    pub weight: u16,
    pub style: CertificateFontStyle,
    pub asset_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateRenderFontGrant {
    pub asset_id: Uuid,
    pub file_id: Uuid,
    pub family: String,
    pub weight: u16,
    pub style: CertificateFontStyle,
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateRenderImageGrant {
    pub asset_id: Uuid,
    pub file_id: Uuid,
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateRenderCampaignValues {
    pub academic_year: String,
    pub campaign_name: String,
    pub event_date: NaiveDate,
    pub issue_date: NaiveDate,
    pub school_name: String,
    pub owner_organization_unit_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateRenderManifest {
    pub template_id: Uuid,
    pub page_geometry: CertificatePageGeometry,
    pub layout: CertificateLayoutV1,
    pub campaign_values: CertificateRenderCampaignValues,
    pub recipient_values: BTreeMap<String, String>,
    pub certificate_number: String,
    pub qr_payload: String,
    pub built_in_fonts: Vec<CertificateBuiltInFont>,
    pub font_grants: Vec<CertificateRenderFontGrant>,
    pub image_grants: Vec<CertificateRenderImageGrant>,
    pub background_grant: CertificateRenderFileGrant,
    pub suggested_filename: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateRenderManifestBatchRequest {
    pub certificate_ids: Vec<Uuid>,
}
