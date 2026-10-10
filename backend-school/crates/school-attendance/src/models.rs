use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AttendanceResult {
    Unchecked,
    Present,
    Late,
    Absent,
    Leave,
    Activity,
}
impl AttendanceResult {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unchecked => "unchecked",
            Self::Present => "present",
            Self::Late => "late",
            Self::Absent => "absent",
            Self::Leave => "leave",
            Self::Activity => "activity",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Unchecked => "ยังไม่เช็ค",
            Self::Present => "มา",
            Self::Late => "สาย",
            Self::Absent => "ขาด",
            Self::Leave => "ลา",
            Self::Activity => "กิจกรรม",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AttendanceKind {
    Arrival,
    Flag,
    Lesson,
    Special,
}
impl AttendanceKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Arrival => "arrival",
            Self::Flag => "flag",
            Self::Lesson => "lesson",
            Self::Special => "special",
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttendanceConfiguration {
    pub enabled: bool,
    pub weekdays: Vec<u32>,
    pub late_after: NaiveTime,
    pub digest_times: Vec<NaiveTime>,
    pub evidence_days: u32,
    pub face_distance: f32,
    pub face_margin: f32,
    pub activity_counts_as_present: bool,
}
impl Default for AttendanceConfiguration {
    fn default() -> Self {
        Self {
            enabled: false,
            weekdays: vec![1, 2, 3, 4, 5],
            late_after: NaiveTime::from_hms_opt(8, 0, 0).unwrap_or(NaiveTime::MIN),
            digest_times: vec![],
            evidence_days: 30,
            face_distance: 0.45,
            face_margin: 0.1,
            activity_counts_as_present: true,
        }
    }
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceSettings {
    pub academic_term_id: Uuid,
    pub configuration: AttendanceConfiguration,
    pub row_version: i64,
    pub archived: bool,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAttendanceSettings {
    pub configuration: AttendanceConfiguration,
    pub row_version: i64,
}
#[derive(Debug, Clone, Deserialize, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttendanceDay {
    pub date: NaiveDate,
    pub counted: bool,
    pub note: String,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAttendanceDays {
    pub days: Vec<AttendanceDay>,
    pub row_version: i64,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct AttendanceQuery {
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct AttendanceTermQuery {
    pub academic_term_id: Uuid,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct AttendanceMonthQuery {
    pub academic_term_id: Uuid,
    pub start: NaiveDate,
    pub end: NaiveDate,
}
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceSession {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
    pub kind: AttendanceKind,
    pub source_key: String,
    pub title: String,
    pub teacher_ids: Vec<Uuid>,
    #[schema(required = true)]
    pub homeroom_id: Option<Uuid>,
    #[schema(required = true)]
    pub learning_group_id: Option<Uuid>,
    #[schema(required = true)]
    pub offering_id: Option<Uuid>,
    #[schema(required = true)]
    pub special_round_id: Option<Uuid>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    #[schema(required = true)]
    pub count_override: Option<bool>,
    pub cancelled: bool,
    #[schema(required = true)]
    pub cancellation_reason: Option<String>,
    #[schema(required = true)]
    pub saved_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub saved_by: Option<Uuid>,
    pub row_version: i64,
}
#[derive(Debug, Clone)]
pub struct AttendanceSeed {
    pub session: AttendanceSession,
    pub students: Vec<AttendanceRosterStudent>,
}
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceRosterStudent {
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub class_number: Option<i32>,
}
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceRecord {
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub display_name: String,
    #[schema(required = true)]
    pub class_number: Option<i32>,
    pub result: AttendanceResult,
    pub origin: String,
    pub note: String,
    #[schema(required = true)]
    pub observed_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub evidence_file_id: Option<Uuid>,
    #[schema(required = true)]
    pub arrival_at: Option<DateTime<Utc>>,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceDetail {
    pub session: AttendanceSession,
    pub students: Vec<AttendanceRecord>,
    pub counted: bool,
    pub writable: bool,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceWorkspace {
    pub date: NaiveDate,
    pub counted: bool,
    pub sessions: Vec<AttendanceSession>,
    pub settings: AttendanceSettings,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenAttendanceSession {
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
    pub kind: AttendanceKind,
    pub source_key: String,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttendanceResultInput {
    pub student_id: Uuid,
    pub result: AttendanceResult,
    pub note: String,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAttendanceResults {
    pub row_version: i64,
    pub reason: String,
    pub students: Vec<AttendanceResultInput>,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttendanceCancellation {
    pub row_version: i64,
    pub cancelled: bool,
    pub reason: String,
}
#[derive(Debug, Clone, Deserialize, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttendanceAudienceGroup {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub name: String,
    pub student_ids: Vec<Uuid>,
    pub row_version: i64,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAttendanceAudience {
    #[schema(required = true)]
    pub id: Option<Uuid>,
    pub name: String,
    pub student_ids: Vec<Uuid>,
    pub row_version: i64,
}
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialAttendanceGroup {
    pub name: String,
    pub teacher_ids: Vec<Uuid>,
    pub homeroom_ids: Vec<Uuid>,
    pub audience_group_ids: Vec<Uuid>,
    pub student_ids: Vec<Uuid>,
    pub use_homeroom_advisors: bool,
}
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialAttendanceDefinition {
    pub title: String,
    pub dates: Vec<NaiveDate>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub counted: bool,
    pub notify: bool,
    pub groups: Vec<SpecialAttendanceGroup>,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecialAttendanceTemplate {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub definition: SpecialAttendanceDefinition,
    pub row_version: i64,
}
#[derive(Debug, Clone, Deserialize, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttendanceDevice {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub operator_id: Uuid,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAttendanceDevice {
    pub name: String,
    pub enabled: bool,
    pub operator_id: Uuid,
}
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FaceDescriptor {
    pub values: Vec<f32>,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnrollAttendanceFace {
    pub model: String,
    pub descriptors: Vec<FaceDescriptor>,
    pub consent_confirmed: bool,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceFace {
    pub student_id: Uuid,
    pub model: String,
    pub descriptors: Vec<FaceDescriptor>,
    pub row_version: i64,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttendanceScan {
    pub event_id: Uuid,
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub student_id: Uuid,
    pub descriptor: FaceDescriptor,
    pub evidence_file_id: Uuid,
    pub captured_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceScanOutcome {
    pub event_id: Uuid,
    pub student_id: Uuid,
    pub result: AttendanceResult,
    pub duplicate: bool,
    pub teacher_conflict: bool,
}
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceSummary {
    pub academic_term_id: Uuid,
    pub student_id: Uuid,
    pub category: String,
    pub scope_key: String,
    pub scope_label: String,
    pub display_name: String,
    pub present: i64,
    pub late: i64,
    pub absent: i64,
    pub leave: i64,
    pub activity: i64,
    pub unchecked: i64,
    pub expected: i64,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceReport {
    pub summaries: Vec<AttendanceSummary>,
    pub archived: bool,
    pub activity_counts_as_present: bool,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendancePurgeImpact {
    pub academic_term_id: Uuid,
    pub records: i64,
    pub evidence: i64,
    pub evidence_bytes: i64,
    pub archived: bool,
    pub can_purge: bool,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PurgeAttendanceTerm {
    pub expected_records: i64,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceStudentOption {
    pub id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub homeroom_id: Option<Uuid>,
    #[schema(required = true)]
    pub homeroom_name: Option<String>,
}
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceTeacherOption {
    pub id: Uuid,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceOptions {
    pub students: Vec<AttendanceStudentOption>,
    pub teachers: Vec<AttendanceTeacherOption>,
    pub audiences: Vec<AttendanceAudienceGroup>,
    pub specials: Vec<SpecialAttendanceTemplate>,
    pub devices: Vec<AttendanceDevice>,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenAttendanceKiosk {
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
    pub device_id: Uuid,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceKioskWorkspace {
    pub sessions: Vec<AttendanceSession>,
    pub faces: Vec<AttendanceFace>,
    pub students: Vec<AttendanceStudentOption>,
    pub configuration: AttendanceConfiguration,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct AttendanceReportQuery {
    pub academic_term_id: Uuid,
    pub student_id: Option<Uuid>,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct AttendanceHistoryQuery {
    pub academic_term_id: Uuid,
    pub student_id: Uuid,
    pub start: NaiveDate,
    pub end: NaiveDate,
}
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceHistoryItem {
    pub session_id: Uuid,
    pub date: NaiveDate,
    pub kind: AttendanceKind,
    pub title: String,
    pub result: AttendanceResult,
    pub note: String,
    #[schema(required = true)]
    pub observed_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub evidence_file_id: Option<Uuid>,
    pub cancelled: bool,
}
