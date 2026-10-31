use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TermLifecycleContext {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub year_name: String,
    pub term_name: String,
    pub year_status: AcademicYearStatus,
    pub term_status: AcademicTermStatus,
    pub year_row_version: i64,
    pub term_row_version: i64,
    pub operation_date: NaiveDate,
    pub sequence: i32,
    pub bell_schedule_id: Uuid,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
}
