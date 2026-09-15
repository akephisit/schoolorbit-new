use super::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct AnnualResultContext {
    pub academic_year_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnualClosureStudent {
    pub student_academic_year_id: Uuid,
    pub revision_id: Option<Uuid>,
    pub revision: Option<i64>,
    pub is_current: bool,
    pub hold_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnualClosureCoverage {
    pub students: Vec<AnnualClosureStudent>,
    pub ready: bool,
    pub source_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnualResultStudent {
    pub student_academic_year_id: Uuid,
    pub student_code: Option<String>,
    pub student_name: String,
    pub grade_level_name: String,
    pub study_program_name: String,
    pub closure: AnnualClosureStudent,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnualTermSource {
    pub academic_term_id: Uuid,
    pub term_name: String,
    pub sequence: i32,
    pub is_current: bool,
    pub revision: Option<TermAggregateRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnualResultPreview {
    pub academic_year_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub terms: Vec<AnnualTermSource>,
    pub totals: CourseCreditTotals,
    pub can_lock: bool,
    pub needs_hold: bool,
    pub source_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnnualLockInput {
    pub expected_revision: Option<i64>,
    pub source_checksum: String,
    pub request_id: Uuid,
    pub hold_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnualResultRevision {
    pub id: Uuid,
    pub revision: i64,
    pub snapshot: AnnualResultPreview,
    pub official_gpa: Option<String>,
    pub hold_reason: Option<String>,
    pub locked_by: Uuid,
    pub locked_at: DateTime<Utc>,
    pub is_current: bool,
}
