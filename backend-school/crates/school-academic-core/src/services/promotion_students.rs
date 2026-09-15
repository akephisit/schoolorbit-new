use super::super::models::{AcademicYearStatus, YearLifecycleContext};
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PromotionStudentContext {
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub student_code: Option<String>,
    pub student_name: String,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
    pub status: super::super::models::StudentAcademicYearStatus,
    pub row_version: i64,
    pub existing_target_student_year_id: Option<Uuid>,
    pub existing_target_row_version: Option<i64>,
}

pub async fn read_promotion_students(
    tx: &mut Transaction<'_, Postgres>,
    source_year: Uuid,
    target_year: Uuid,
    students: &[Uuid],
) -> Result<Vec<PromotionStudentContext>, AppError> {
    if source_year == target_year
        || source_year.is_nil()
        || target_year.is_nil()
        || students.is_empty()
        || students.len() > 500
        || students
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != students.len()
    {
        return Err(AppError::ValidationError(
            "เลือกปีต่างกันและนักเรียนไม่ซ้ำ 1–500 รายการ".into(),
        ));
    }
    super::lifecycle_context::read_year_context(tx, target_year).await?;
    let rows:Vec<PromotionStudentContext>=sqlx::query_as(
        "SELECT source.id AS student_academic_year_id,source.student_id,info.student_id AS student_code,
         concat_ws(' ',nullif(btrim(student.title),''),student.first_name,student.last_name) AS student_name,
         source.grade_level_id,source.study_program_id,source.status,source.row_version,
         target.id AS existing_target_student_year_id,target.row_version AS existing_target_row_version
         FROM student_academic_years source JOIN users student ON student.id=source.student_id
         LEFT JOIN student_info info ON info.user_id=source.student_id
         LEFT JOIN student_academic_years target ON target.student_id=source.student_id AND target.academic_year_id=$2
         WHERE source.academic_year_id=$1 AND source.id=ANY($3) ORDER BY source.id"
    ).bind(source_year).bind(target_year).bind(students).fetch_all(&mut **tx).await?;
    if rows.len() != students.len() {
        return Err(AppError::NotFound("ไม่พบนักเรียนครบตามปีต้นทางที่เลือก".into()));
    }
    Ok(rows)
}

/// Caller acquires the tenant transition lock before this ordered context lock.
/// This validates preparation only; activation is a separate Core command.
pub async fn validate_run_years(
    tx: &mut Transaction<'_, Postgres>,
    source_year: Uuid,
    target_year: Uuid,
) -> Result<(), AppError> {
    let years: Vec<YearLifecycleContext> = sqlx::query_as(
        "SELECT id AS academic_year_id,year,name,start_date,end_date,status,row_version FROM academic_years WHERE id=ANY($1) ORDER BY id FOR SHARE"
    ).bind(vec![source_year,target_year]).fetch_all(&mut **tx).await?;
    let source = years
        .iter()
        .find(|year| year.academic_year_id == source_year)
        .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษาต้นทาง".into()))?;
    let target = years
        .iter()
        .find(|year| year.academic_year_id == target_year)
        .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษาปลายทาง".into()))?;
    if source_year == target_year
        || target.year <= source.year
        || target.start_date <= source.end_date
    {
        return Err(AppError::ValidationError(
            "ปีปลายทางต้องอยู่หลังปีต้นทางและช่วงวันที่ต้องไม่ทับกัน".into(),
        ));
    }
    if !matches!(
        source.status,
        AcademicYearStatus::Active | AcademicYearStatus::Closing | AcademicYearStatus::Closed
    ) || target.status != AcademicYearStatus::Planning
    {
        return Err(AppError::Conflict(
            "เลือกปีต้นทางที่เปิดเรียนหรือปิดแล้ว และปีปลายทางที่กำลังวางแผน".into(),
        ));
    }
    Ok(())
}
