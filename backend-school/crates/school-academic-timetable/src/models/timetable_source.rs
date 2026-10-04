use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
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

#[derive(Clone, Debug, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableSourceIssue {
    pub block_id: Uuid,
    pub learning_group_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub code: TimetableSourceIssueCode,
}
