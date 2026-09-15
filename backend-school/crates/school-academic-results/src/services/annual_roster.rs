use super::*;
use school_authorization::ActorContext;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_annual_students(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
) -> Result<Vec<AnnualResultStudent>, AppError> {
    crate::policy::aggregate::require_aggregate_read(actor)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let coverage = annual_revisions::annual_closure_coverage(&mut tx, year).await?;
    let ids: Vec<_> = coverage
        .students
        .iter()
        .map(|row| row.student_academic_year_id)
        .collect();
    let identities = aggregate_roster::student_identities(&mut tx, year, &ids).await?;
    let mut coverage: std::collections::BTreeMap<_, _> = coverage
        .students
        .into_iter()
        .map(|row| (row.student_academic_year_id, row))
        .collect();
    let rows = identities
        .into_iter()
        .map(|row| {
            let closure = coverage
                .remove(&row.student_academic_year_id)
                .ok_or_else(|| {
                    AppError::InternalServerError("Annual coverage identity mismatch".into())
                })?;
            Ok(AnnualResultStudent {
                student_academic_year_id: row.student_academic_year_id,
                student_code: row.student_code,
                student_name: row.student_name,
                grade_level_name: row.grade_level_name,
                study_program_name: row.study_program_name,
                closure,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    tx.commit().await?;
    Ok(rows)
}
