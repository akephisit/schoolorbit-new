use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
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
    pub year_start_date: NaiveDate,
    pub year_end_date: NaiveDate,
    pub term_start_date: NaiveDate,
    pub planned_end_date: Option<NaiveDate>,
    pub closed_on: Option<NaiveDate>,
    pub sequence: i32,
    pub bell_schedule_id: Uuid,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
}
