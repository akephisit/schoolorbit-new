use super::*;
use school_authorization::ActorContext;
use sqlx::PgPool;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct StudentIdentity {
    pub student_academic_year_id: Uuid,
    pub student_code: Option<String>,
    pub student_name: String,
    pub grade_level_name: String,
    pub study_program_name: String,
}

pub async fn list_aggregate_students(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
) -> Result<Vec<AggregateStudent>, AppError> {
    crate::policy::aggregate::require_aggregate_read(actor)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let coverage = term_closure_coverage(&mut tx, context).await?;
    let ids: Vec<_> = coverage
        .students
        .iter()
        .map(|row| row.student_academic_year_id)
        .collect();
    let identities = student_identities(&mut tx, context.academic_year_id, &ids).await?;
    let mut coverage: BTreeMap<_, _> = coverage
        .students
        .into_iter()
        .map(|row| (row.student_academic_year_id, row))
        .collect();
    let rows = identities
        .into_iter()
        .map(|identity| {
            let closure = coverage
                .remove(&identity.student_academic_year_id)
                .ok_or_else(|| {
                    AppError::InternalServerError("Aggregate coverage identity mismatch".into())
                })?;
            Ok(AggregateStudent {
                student_academic_year_id: identity.student_academic_year_id,
                student_code: identity.student_code,
                student_name: identity.student_name,
                grade_level_name: identity.grade_level_name,
                study_program_name: identity.study_program_name,
                closure,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    tx.commit().await?;
    Ok(rows)
}

pub(super) async fn student_identities(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    ids: &[Uuid],
) -> Result<Vec<StudentIdentity>, AppError> {
    let identities:Vec<StudentIdentity>=sqlx::query_as(
        "SELECT sy.id AS student_academic_year_id,info.student_id AS student_code,
         concat_ws(' ',nullif(btrim(student.title),''),student.first_name,student.last_name) AS student_name,
         CASE grade.level_type WHEN 'kindergarten' THEN 'อนุบาลปีที่ '||grade.year WHEN 'primary' THEN 'ประถมศึกษาปีที่ '||grade.year WHEN 'secondary' THEN 'มัธยมศึกษาปีที่ '||grade.year ELSE 'ระดับชั้น '||grade.year END AS grade_level_name,
         program.name_th AS study_program_name
         FROM student_academic_years sy JOIN users student ON student.id=sy.student_id
         LEFT JOIN student_info info ON info.user_id=sy.student_id
         JOIN grade_levels grade ON grade.id=sy.grade_level_id
         JOIN study_programs program ON program.id=sy.study_program_id
         WHERE sy.academic_year_id=$1 AND sy.id=ANY($2)
         ORDER BY info.student_id NULLS LAST,student.first_name,student.last_name,sy.id"
    ).bind(year).bind(ids).fetch_all(&mut **tx).await?;
    if identities.len() != ids.len() {
        return Err(AppError::Conflict(
            "ข้อมูลนักเรียนสำหรับตรวจผลสรุปไม่ครบ กรุณาตรวจทะเบียนนักเรียน".into(),
        ));
    }
    Ok(identities)
}
