use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActivationIssueCode {
    YearState,
    TermState,
    OtherRunningYear,
    OtherRunningTerm,
    EarlierYearOpen,
    EarlierTermOpen,
    NotFirstTerm,
    YearOverlap,
    TermConfiguration,
    BellSchedule,
    StudentReference,
    PlacementReference,
    RoomCapacity,
    ExistingActiveEnrollment,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivationIssue {
    pub code: ActivationIssueCode,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivationStudent {
    pub id: Uuid,
    pub student_id: Uuid,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
    pub status: StudentAcademicYearStatus,
    pub row_version: i64,
    pub reference_valid: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivationPlacement {
    pub id: Uuid,
    pub student_academic_year_id: Uuid,
    pub homeroom_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub status: HomeroomPlacementStatus,
    pub row_version: i64,
    pub room_row_version: Option<i64>,
    pub capacity: Option<i32>,
    pub reference_valid: bool,
    pub eligible: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ActivationState {
    pub context: TermLifecycleContext,
    pub opens_year: bool,
    pub predecessor: Option<YearLifecycleContext>,
    pub students: Vec<ActivationStudent>,
    pub placements: Vec<ActivationPlacement>,
    pub issues: Vec<ActivationIssue>,
    pub source_checksum: String,
}
