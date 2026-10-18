//! Student Care wire contracts. Instrument versions have no lifecycle status.
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! care_enum {
    ($name:ident { $($variant:ident),+ }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        #[cfg_attr(feature="api-docs", derive(utoipa::ToSchema))]
        pub enum $name { $($variant),+ }
    }
}
care_enum!(CareFamily {
    Sdq,
    Eq,
    Depression,
    Other
});
care_enum!(CareRespondent {
    SelfReport,
    Teacher,
    Guardian
});
care_enum!(CareInstrumentMode {
    Digital,
    ManualResult
});
care_enum!(CareAssignmentStatus {
    Assigned,
    InProgress,
    Submitted,
    Cancelled
});
care_enum!(CareRoundStatus {
    Open,
    Closed,
    Cancelled
});
care_enum!(CareVisitStatus {
    Planned,
    Completed,
    Cancelled
});
care_enum!(CareCaseStatus {
    Open,
    Monitoring,
    Closed
});
care_enum!(CareEventKind {
    Note,
    Contact,
    Referral,
    FollowUp,
    Acknowledgement,
    Closure,
    Reopened
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareOption {
    pub key: String,
    pub label: String,
    pub points: i32,
    pub urgent: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareQuestion {
    pub key: String,
    pub text: String,
    pub scale_key: String,
    pub reverse: bool,
    pub options: Vec<CareOption>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareBand {
    pub key: String,
    pub label: String,
    pub minimum: i32,
    pub maximum: i32,
    pub urgent: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareScale {
    pub key: String,
    pub label: String,
    pub source_scale_keys: Vec<String>,
    pub bands: Vec<CareBand>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareInstrumentDefinition {
    pub name: String,
    pub family: CareFamily,
    pub variant: String,
    pub language: String,
    pub respondent: CareRespondent,
    pub minimum_age: i32,
    pub maximum_age: i32,
    pub mode: CareInstrumentMode,
    pub instructions: String,
    pub source: String,
    pub license_note: String,
    pub digital_rights_confirmed: bool,
    pub questions: Vec<CareQuestion>,
    pub scales: Vec<CareScale>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareInstrumentVersion {
    pub id: Uuid,
    pub instrument_id: Uuid,
    pub version: i32,
    pub definition: CareInstrumentDefinition,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CreateCareInstrumentVersion {
    pub request_id: Uuid,
    pub instrument_id: Option<Uuid>,
    pub expected_version: Option<i32>,
    pub definition: CareInstrumentDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareRound {
    pub id: Uuid,
    pub name: String,
    pub purpose: String,
    pub instrument_version_ids: Vec<Uuid>,
    pub academic_term_id: Uuid,
    pub roster_date: NaiveDate,
    pub due_date: NaiveDate,
    pub status: CareRoundStatus,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CreateCareRound {
    pub request_id: Uuid,
    pub name: String,
    pub purpose: String,
    pub instrument_version_ids: Vec<Uuid>,
    pub academic_term_id: Uuid,
    pub roster_date: NaiveDate,
    pub due_date: NaiveDate,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct UpdateCareRound {
    pub request_id: Uuid,
    pub row_version: i64,
    pub status: CareRoundStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareStudentOption {
    pub id: Uuid,
    pub name: String,
    pub student_code: Option<String>,
    pub homeroom: String,
    pub advisor_ids: Vec<Uuid>,
    pub guardian_ids: Vec<Uuid>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareStaffOption {
    pub id: Uuid,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
pub struct CareListQuery {
    pub after_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub follow_up_before: Option<NaiveDate>,
    pub round_id: Option<Uuid>,
    pub respondent_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    pub status: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
pub struct CareStudentQuery {
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
    pub search: Option<String>,
    pub after_id: Option<Uuid>,
    pub limit: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareAnswer {
    pub question_key: String,
    pub option_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareScaleResult {
    pub scale_key: String,
    pub score: i32,
    pub band_key: String,
    pub band_label: String,
    pub urgent: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareManualScore {
    pub scale_key: String,
    pub score: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareAssignment {
    pub created_by: Uuid,
    pub id: Uuid,
    pub round_id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub respondent_id: Uuid,
    pub respondent: CareRespondent,
    pub instrument_version_id: Uuid,
    pub instrument_name: String,
    pub reviewer_user_ids: Vec<Uuid>,
    pub status: CareAssignmentStatus,
    pub row_version: i64,
    pub submitted_at: Option<DateTime<Utc>>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub urgent: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareAssignmentDetail {
    pub assignment: CareAssignment,
    pub instrument: CareInstrumentVersion,
    pub answers: Vec<CareAnswer>,
    pub manual_scores: Vec<CareManualScore>,
    pub results: Vec<CareScaleResult>,
    pub manual_note: Option<String>,
    pub purpose: String,
    pub purpose_acknowledged_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareAssignmentTarget {
    pub student_id: Uuid,
    pub respondent_id: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CreateCareAssignments {
    pub request_id: Uuid,
    pub round_id: Uuid,
    pub instrument_version_id: Uuid,
    pub targets: Vec<CareAssignmentTarget>,
    pub reviewer_user_ids: Vec<Uuid>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct SaveCareResponse {
    pub acknowledge_purpose: bool,
    pub request_id: Uuid,
    pub row_version: i64,
    pub answers: Vec<CareAnswer>,
    pub manual_scores: Vec<CareManualScore>,
    pub manual_note: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct UpdateCareAssignmentReviewers {
    pub request_id: Uuid,
    pub row_version: i64,
    pub reviewer_user_ids: Vec<Uuid>,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareRevisionCommand {
    pub request_id: Uuid,
    pub row_version: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareVisit {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub academic_term_id: Uuid,
    pub planned_date: NaiveDate,
    pub visitor_ids: Vec<Uuid>,
    pub status: CareVisitStatus,
    pub row_version: i64,
    pub completed_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareVisitRecord {
    pub visit: CareVisit,
    pub visited_date: Option<NaiveDate>,
    pub contact_person: String,
    pub factual_observations: String,
    pub strengths: String,
    pub needs: String,
    pub agreed_actions: String,
    pub next_follow_up: Option<NaiveDate>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct SaveCareVisit {
    pub visited_date: Option<NaiveDate>,
    pub contact_person: String,
    pub factual_observations: String,
    pub request_id: Uuid,
    pub row_version: Option<i64>,
    pub student_id: Uuid,
    pub academic_term_id: Uuid,
    pub planned_date: NaiveDate,
    pub visitor_ids: Vec<Uuid>,
    pub status: CareVisitStatus,
    pub strengths: String,
    pub needs: String,
    pub agreed_actions: String,
    pub next_follow_up: Option<NaiveDate>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareCase {
    pub id: Uuid,
    pub source_assignment_id: Option<Uuid>,
    pub source_visit_id: Option<Uuid>,
    pub student_id: Uuid,
    pub student_name: String,
    pub title: String,
    pub status: CareCaseStatus,
    pub assigned_user_ids: Vec<Uuid>,
    pub next_follow_up: Option<NaiveDate>,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareCaseEvent {
    pub id: Uuid,
    pub kind: CareEventKind,
    pub note: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareCaseDetail {
    pub case: CareCase,
    pub events: Vec<CareCaseEvent>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct SaveCareCase {
    pub source_assignment_id: Option<Uuid>,
    pub source_visit_id: Option<Uuid>,
    pub request_id: Uuid,
    pub row_version: Option<i64>,
    pub student_id: Uuid,
    pub title: String,
    pub assigned_user_ids: Vec<Uuid>,
    pub next_follow_up: Option<NaiveDate>,
    pub note: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct AddCareCaseEvent {
    pub request_id: Uuid,
    pub row_version: i64,
    pub kind: CareEventKind,
    pub note: String,
    pub status: CareCaseStatus,
    pub next_follow_up: Option<NaiveDate>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CareOverview {
    pub follow_up_due_count: i64,
    pub assigned_count: i64,
    pub submitted_count: i64,
    pub pending_visit_count: i64,
    pub open_case_count: i64,
    pub urgent_unacknowledged_count: i64,
}
