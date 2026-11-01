use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub enum TimetableDraftSourceConflictCode {
    #[serde(rename = "TIMETABLE_DELIVERY_SOURCE_OUTDATED")]
    DeliverySourceOutdated,
}

impl TimetableDraftSourceConflictCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeliverySourceOutdated => "TIMETABLE_DELIVERY_SOURCE_OUTDATED",
        }
    }
}

/// Present only when a latest-source guard rejected the operation before any write.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableDraftSourceConflict {
    pub code: TimetableDraftSourceConflictCode,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TimetableSourceIssueCode {
    MissingOffering,
    HomeroomCoverageMismatch,
    MissingInstructor,
    MissingGroup,
    GroupOfferingMismatch,
    IneligibleInstructor,
    InstructorRoleMismatch,
    SchedulingModeMismatch,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableSourceIssue {
    pub block_id: Uuid,
    pub learning_group_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub code: TimetableSourceIssueCode,
}
