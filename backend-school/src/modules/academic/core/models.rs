use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AcademicYearStatus {
    Planning,
    Ready,
    Active,
    Closing,
    Closed,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AcademicTermStatus {
    Planning,
    Ready,
    Active,
    Closing,
    Closed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AcademicTermType {
    Regular,
    Summer,
    Remedial,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum VersionStatus {
    Draft,
    Published,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum StudentAcademicYearStatus {
    Planned,
    Active,
    Completed,
    Withdrawn,
    Graduated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum GradeProgressionKind {
    Promote,
    Repeat,
    Graduate,
    Exception,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAcademicYearRequest {
    pub year: i32,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub school_days: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAcademicYearRequest {
    pub year: i32,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub school_days: Vec<String>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAcademicTermRequest {
    pub academic_year_id: Uuid,
    pub sequence: i32,
    pub code: String,
    pub name: String,
    pub term_type: AcademicTermType,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
    pub bell_schedule_id: Uuid,
}

#[cfg(test)]
impl CreateAcademicTermRequest {
    pub fn fixture(academic_year_id: Uuid, sequence: i32, code: &str) -> Self {
        Self {
            academic_year_id,
            sequence,
            code: code.to_string(),
            name: code.to_string(),
            term_type: AcademicTermType::Regular,
            start_date: NaiveDate::from_ymd_opt(2027, 5, 1).expect("valid fixture date"),
            end_date: NaiveDate::from_ymd_opt(2027, 9, 30).expect("valid fixture date"),
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: Uuid::new_v4(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAcademicTermRequest {
    pub sequence: i32,
    pub code: String,
    pub name: String,
    pub term_type: AcademicTermType,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
    pub bell_schedule_id: Uuid,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BellSchedulePeriodInput {
    pub name: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub order_index: i32,
    pub applicable_days: Vec<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceBellSchedulePeriodsRequest {
    pub periods: Vec<BellSchedulePeriodInput>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradeProgressionInput {
    pub from_grade_level_id: Uuid,
    pub to_grade_level_id: Option<Uuid>,
    pub transition_kind: GradeProgressionKind,
    pub curriculum_id: Option<Uuid>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceGradeProgressionsRequest {
    pub progressions: Vec<GradeProgressionInput>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicYear {
    pub id: Uuid,
    pub year: i32,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub school_days: Vec<String>,
    pub status: AcademicYearStatus,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicTerm {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub sequence: i32,
    pub code: String,
    pub name: String,
    pub term_type: AcademicTermType,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
    pub bell_schedule_id: Uuid,
    pub status: AcademicTermStatus,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicYearOption {
    pub id: Uuid,
    pub year: i32,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: AcademicYearStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicTermOption {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub sequence: i32,
    pub code: String,
    pub name: String,
    pub term_type: AcademicTermType,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
    pub status: AcademicTermStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicContextOptions {
    pub years: Vec<AcademicYearOption>,
    pub terms: Vec<AcademicTermOption>,
    pub active_academic_year_id: Option<Uuid>,
    pub active_academic_term_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateBellScheduleRequest {
    pub academic_year_id: Uuid,
    pub code: String,
    pub name: String,
    pub is_default: bool,
    pub owning_organization_unit_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateBellScheduleRequest {
    pub code: String,
    pub name: String,
    pub is_default: bool,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BellSchedule {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub code: String,
    pub name: String,
    pub is_default: bool,
    pub status: VersionStatus,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BellSchedulePeriod {
    pub id: Uuid,
    pub bell_schedule_id: Uuid,
    pub name: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub order_index: i32,
    pub applicable_days: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct GradeProgression {
    pub id: Uuid,
    pub from_grade_level_id: Uuid,
    pub to_grade_level_id: Option<Uuid>,
    pub transition_kind: GradeProgressionKind,
    pub curriculum_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GradeProgressionSet {
    pub row_version: i64,
    pub progressions: Vec<GradeProgression>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSubject {
    pub id: Uuid,
    pub code: String,
    pub owning_organization_unit_id: Option<Uuid>,
    pub archived_at: Option<DateTime<Utc>>,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCatalogSubjectRequest {
    pub code: String,
    pub owning_organization_unit_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCatalogSubjectRequest {
    pub code: String,
    pub owning_organization_unit_id: Option<Uuid>,
    pub archived: bool,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SubjectVersion {
    pub id: Uuid,
    pub subject_id: Uuid,
    pub version_no: i32,
    pub name_th: String,
    pub name_en: Option<String>,
    pub credit: String,
    pub hours_per_semester: Option<i32>,
    pub subject_type: String,
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
    pub term_code: Option<String>,
    pub periods_per_week: Option<i32>,
    pub grade_level_ids: Vec<Uuid>,
    pub status: VersionStatus,
    pub published_at: Option<DateTime<Utc>>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSubjectVersionRequest {
    pub name_th: String,
    pub name_en: Option<String>,
    #[schema(value_type = String, pattern = r"^(0|[1-9][0-9]*)(\.[0-9]{1,2})?$")]
    pub credit: String,
    pub hours_per_semester: Option<i32>,
    pub subject_type: String,
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
    pub term_code: Option<String>,
    pub periods_per_week: Option<i32>,
    pub grade_level_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSubjectVersionRequest {
    pub name_th: String,
    pub name_en: Option<String>,
    #[schema(value_type = String, pattern = r"^(0|[1-9][0-9]*)(\.[0-9]{1,2})?$")]
    pub credit: String,
    pub hours_per_semester: Option<i32>,
    pub subject_type: String,
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
    pub term_code: Option<String>,
    pub periods_per_week: Option<i32>,
    pub grade_level_ids: Vec<Uuid>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CatalogActivity {
    pub id: Uuid,
    pub code: String,
    pub activity_type: String,
    pub owning_organization_unit_id: Option<Uuid>,
    pub archived_at: Option<DateTime<Utc>>,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCatalogActivityRequest {
    pub code: String,
    pub activity_type: String,
    pub owning_organization_unit_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCatalogActivityRequest {
    pub code: String,
    pub activity_type: String,
    pub owning_organization_unit_id: Option<Uuid>,
    pub archived: bool,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivityVersion {
    pub id: Uuid,
    pub activity_id: Uuid,
    pub version_no: i32,
    pub name: String,
    pub description: Option<String>,
    #[schema(value_type = String, pattern = r"^(0|[1-9][0-9]*)(\.[0-9]{1,2})?$")]
    pub hours_per_week: String,
    pub scheduling_mode: String,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
    pub term_code: Option<String>,
    pub grade_level_ids: Vec<Uuid>,
    pub status: VersionStatus,
    pub published_at: Option<DateTime<Utc>>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateActivityVersionRequest {
    pub name: String,
    pub description: Option<String>,
    #[schema(value_type = String, pattern = r"^(0|[1-9][0-9]*)(\.[0-9]{1,2})?$")]
    pub hours_per_week: String,
    pub scheduling_mode: String,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
    pub term_code: Option<String>,
    pub grade_level_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateActivityVersionRequest {
    pub name: String,
    pub description: Option<String>,
    #[schema(value_type = String, pattern = r"^(0|[1-9][0-9]*)(\.[0-9]{1,2})?$")]
    pub hours_per_week: String,
    pub scheduling_mode: String,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
    pub term_code: Option<String>,
    pub grade_level_ids: Vec<Uuid>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DefaultTeacher {
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceDefaultTeachersRequest {
    pub teachers: Vec<DefaultTeacher>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SubjectGroup {
    pub id: Uuid,
    pub code: String,
    pub name_th: String,
    pub name_en: String,
    pub display_order: Option<i32>,
    pub is_active: Option<bool>,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSubjectGroupRequest {
    pub code: String,
    pub name_th: String,
    pub name_en: String,
    pub display_order: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSubjectGroupRequest {
    pub code: String,
    pub name_th: String,
    pub name_en: String,
    pub display_order: i32,
    pub is_active: bool,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Curriculum {
    pub id: Uuid,
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub grade_level_ids: Vec<Uuid>,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCurriculumRequest {
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub grade_level_ids: Vec<Uuid>,
    pub owning_organization_unit_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCurriculumRequest {
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub grade_level_ids: Vec<Uuid>,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumVersion {
    pub id: Uuid,
    pub curriculum_id: Uuid,
    pub version_name: String,
    pub start_academic_year_id: Uuid,
    pub end_academic_year_id: Option<Uuid>,
    pub description: Option<String>,
    pub status: VersionStatus,
    pub published_at: Option<DateTime<Utc>>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCurriculumVersionRequest {
    pub version_name: String,
    pub start_academic_year_id: Uuid,
    pub end_academic_year_id: Option<Uuid>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCurriculumVersionRequest {
    pub version_name: String,
    pub start_academic_year_id: Uuid,
    pub end_academic_year_id: Option<Uuid>,
    pub description: Option<String>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StudyProgram {
    pub id: Uuid,
    pub curriculum_version_id: Uuid,
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub is_default: bool,
    pub status: VersionStatus,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StudyProgramOption {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub curriculum_id: Uuid,
    pub curriculum_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateStudyProgramRequest {
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub is_default: bool,
    pub owning_organization_unit_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateStudyProgramRequest {
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub is_default: bool,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum RequirementKind {
    Required,
    Elective,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum RequirementResourceKind {
    Course,
    Activity,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ProgramRequirement {
    pub id: Uuid,
    pub resource_kind: RequirementResourceKind,
    pub catalog_version_id: Uuid,
    pub grade_level_id: Uuid,
    pub recommended_term_code: Option<String>,
    pub requirement_kind: RequirementKind,
    pub credit: Option<String>,
    pub hours: Option<String>,
    pub display_order: i32,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StudyProgramRequirement {
    pub study_program_id: Uuid,
    #[serde(flatten)]
    pub requirement: ProgramRequirement,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumProgramWorkspace {
    pub programs: Vec<StudyProgram>,
    pub requirements: Vec<StudyProgramRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicSetupWorkspace {
    pub years: Vec<AcademicYear>,
    pub terms: Vec<AcademicTerm>,
    pub bell_schedules: Vec<BellSchedule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgramRequirementInput {
    pub resource_kind: RequirementResourceKind,
    pub catalog_version_id: Uuid,
    pub grade_level_id: Uuid,
    pub recommended_term_code: Option<String>,
    pub requirement_kind: RequirementKind,
    #[schema(value_type = Option<String>, pattern = r"^(0|[1-9][0-9]*)(\.[0-9]{1,2})?$")]
    pub credit: Option<String>,
    #[schema(value_type = Option<String>, pattern = r"^(0|[1-9][0-9]*)(\.[0-9]{1,2})?$")]
    pub hours: Option<String>,
    pub display_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceProgramRequirementsRequest {
    pub requirements: Vec<ProgramRequirementInput>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishVersionRequest {
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Homeroom {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub room_number: Option<String>,
    pub study_program_id: Uuid,
    pub capacity: i32,
    pub is_active: Option<bool>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateHomeroomRequest {
    pub academic_year_id: Uuid,
    pub code: String,
    pub name: String,
    pub grade_level_id: Uuid,
    pub room_number: Option<String>,
    pub study_program_id: Uuid,
    pub capacity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateHomeroomRequest {
    pub code: String,
    pub name: String,
    pub grade_level_id: Uuid,
    pub room_number: Option<String>,
    pub study_program_id: Uuid,
    pub capacity: i32,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HomeroomAdvisor {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HomeroomAdvisorAssignment {
    pub id: Uuid,
    pub homeroom_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HomeroomAdvisorInput {
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceHomeroomAdvisorsRequest {
    pub advisors: Vec<HomeroomAdvisorInput>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StudentAcademicYear {
    pub id: Uuid,
    pub student_id: Uuid,
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
    pub status: StudentAcademicYearStatus,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateStudentAcademicYearRequest {
    pub academic_year_id: Uuid,
    pub student_id: Uuid,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateStudentAcademicYearRequest {
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
    pub row_version: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudentAcademicYearFilter {
    pub academic_year_id: Uuid,
    pub student_id: Option<Uuid>,
    pub grade_level_id: Option<Uuid>,
    pub study_program_id: Option<Uuid>,
    pub homeroom_id: Option<Uuid>,
    pub status: Option<StudentAcademicYearStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum HomeroomPlacementStatus {
    Planned,
    Current,
    Ended,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HomeroomPlacement {
    pub id: Uuid,
    pub student_academic_year_id: Uuid,
    pub academic_year_id: Uuid,
    pub homeroom_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub status: HomeroomPlacementStatus,
    pub enrollment_type: String,
    pub class_number: Option<i32>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateHomeroomPlacementRequest {
    pub homeroom_id: Uuid,
    pub start_date: NaiveDate,
    pub status: HomeroomPlacementStatus,
    pub enrollment_type: String,
    pub class_number: Option<i32>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferHomeroomPlacementRequest {
    pub target_homeroom_id: Uuid,
    pub transfer_date: NaiveDate,
    pub enrollment_type: String,
    pub class_number: Option<i32>,
    pub reason: String,
    pub row_version: i64,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HomeroomPlacementTransfer {
    pub ended_placement: HomeroomPlacement,
    pub new_placement: HomeroomPlacement,
    pub replayed: bool,
}
