use super::*;
use crate::middleware::permission::ActorContext;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_annual_students(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
) -> Result<Vec<AnnualResultStudent>, AppError> {
    crate::policies::academic_aggregate_access_policy::require_aggregate_read(actor)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::academic::{
            cutover_test_support::apply_migrations_through, results::services_tests::fixture,
        },
        permissions::registry::codes,
    };

    #[tokio::test]
    async fn annual_roster_retains_missing_results_and_requires_both_school_domains() {
        let (pool, actor, context, _) = fixture("annual_roster").await;
        apply_migrations_through(&pool, 69).await.unwrap();
        let office = ActorContext {
            user_id: actor.user_id,
            permissions: vec![
                codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
                codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
            ],
        };
        let rows = list_annual_students(&pool, &office, context.academic_year_id)
            .await
            .unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| !row.student_name.is_empty()
            && !row.grade_level_name.is_empty()
            && !row.study_program_name.is_empty()
            && row.closure.revision_id.is_none()
            && !row.closure.is_current));
        for permissions in [
            vec![codes::ACADEMIC_RESULT_READ_SCHOOL.into()],
            vec![
                codes::ACADEMIC_RESULT_READ_ASSIGNED.into(),
                codes::ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED.into(),
            ],
        ] {
            let denied = ActorContext {
                user_id: actor.user_id,
                permissions,
            };
            assert!(matches!(
                list_annual_students(&pool, &denied, context.academic_year_id).await,
                Err(AppError::Forbidden(_))
            ));
        }
        assert!(list_annual_students(&pool, &office, Uuid::new_v4())
            .await
            .is_err());
    }
}
