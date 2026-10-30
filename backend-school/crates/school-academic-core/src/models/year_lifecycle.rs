use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearLifecycleContext {
    pub academic_year_id: Uuid,
    pub year: i32,
    pub name: String,
    pub status: AcademicYearStatus,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearTermLifecycleState {
    pub academic_term_id: Uuid,
    pub name: String,
    pub sequence: i32,
    pub status: AcademicTermStatus,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
    pub row_version: i64,
}
