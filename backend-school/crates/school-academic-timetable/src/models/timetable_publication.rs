use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::timetable_source::TimetableSourceIssue;

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewTimetablePublicationRequest {
    pub row_version: i64,
    pub effective_from: NaiveDate,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishTimetableVersionRequest {
    pub row_version: i64,
    pub effective_from: NaiveDate,
    pub preview_hash: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetablePublicationFindingCode {
    InvalidDate,
    DeliveryDateMismatch,
    DateAlreadyPublished,
    NoContentChange,
    SourceNeedsReview,
    PeriodCountMismatch,
    MissingTargets,
    PendingSynchronization,
    Collision,
    InactiveResource,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetablePublicationFinding {
    pub code: TimetablePublicationFindingCode,
    pub message: String,
    #[schema(required = true)]
    pub block_id: Option<Uuid>,
    #[schema(required = true)]
    pub learning_offering_id: Option<Uuid>,
    #[schema(required = true)]
    pub learning_group_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetablePublicationPreview {
    pub timetable_version_id: Uuid,
    pub row_version: i64,
    pub delivery_version_id: Uuid,
    pub latest_delivery_version_id: Uuid,
    pub effective_from: NaiveDate,
    pub block_count: usize,
    pub content_changed: bool,
    pub can_publish: bool,
    pub source_issues: Vec<TimetableSourceIssue>,
    pub findings: Vec<TimetablePublicationFinding>,
    pub preview_hash: String,
}
