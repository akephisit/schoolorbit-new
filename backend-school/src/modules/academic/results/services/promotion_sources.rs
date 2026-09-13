use super::*;
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) async fn promotion_annual_sources(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    students: &[Uuid],
) -> Result<BTreeMap<Uuid, Option<AnnualResultRevision>>, AppError> {
    // The owner validates 1–500 distinct student-year IDs and exact year scope.
    // Caller holds a consistent read transaction or the lifecycle transition lock.
    let previews = annual_revisions::annual_students_in_transaction(tx, year, students).await?;
    let rows:Vec<StoredSource>=sqlx::query_as(
        "SELECT DISTINCT ON(student_academic_year_id) student_academic_year_id,id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at FROM academic_annual_result_revisions WHERE academic_year_id=$1 AND student_academic_year_id=ANY($2) ORDER BY student_academic_year_id,revision DESC"
    ).bind(year).bind(students).fetch_all(&mut **tx).await?;
    let mut sources: BTreeMap<_, _> = students.iter().map(|id| (*id, None)).collect();
    for row in rows {
        let preview = previews.get(&row.student_academic_year_id).ok_or_else(|| {
            AppError::InternalServerError("ผลรายปีไม่ตรงกับนักเรียนในชุดที่ตรวจสอบ".into())
        })?;
        let is_current = preview.can_lock
            && preview.source_checksum == row.snapshot.source_checksum
            && preview.needs_hold == row.hold_reason.is_some();
        sources.insert(
            row.student_academic_year_id,
            Some(AnnualResultRevision {
                id: row.id,
                revision: row.revision,
                snapshot: row.snapshot.0,
                official_gpa: row.official_gpa,
                hold_reason: row.hold_reason,
                locked_by: row.locked_by,
                locked_at: row.locked_at,
                is_current,
            }),
        );
    }
    Ok(sources)
}

#[derive(sqlx::FromRow)]
struct StoredSource {
    student_academic_year_id: Uuid,
    id: Uuid,
    revision: i64,
    snapshot: sqlx::types::Json<AnnualResultPreview>,
    official_gpa: Option<String>,
    hold_reason: Option<String>,
    locked_by: Uuid,
    locked_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::{
        cutover_test_support::apply_migrations_through,
        results::aggregate_revision_tests::ready_aggregate_fixture,
    };

    #[tokio::test]
    async fn promotion_annual_source_batch_preserves_missing_zero_and_stale_evidence() {
        let (pool, actor, context, student) =
            ready_aggregate_fixture("promotion_annual_sources").await;
        apply_migrations_through(&pool, 69).await.unwrap();
        sqlx::query(
            "UPDATE academic_terms SET included_in_year_result=(id=$2) WHERE academic_year_id=$1",
        )
        .bind(context.academic_year_id)
        .bind(context.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
        let extra_user = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES ($1,$2,'fixture-not-a-login','E2E-LIFECYCLE','No result','student','active')")
            .bind(extra_user).bind(format!("promotion-no-result-{extra_user}")).execute(&pool).await.unwrap();
        let extra_student = Uuid::new_v4();
        sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,$2,academic_year_id,grade_level_id,study_program_id,'active' FROM student_academic_years WHERE id=$3")
            .bind(extra_student).bind(extra_user).bind(student).execute(&pool).await.unwrap();
        let students: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 2",
        )
        .bind(context.academic_year_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(students.len(), 2);
        let mut tx = pool.begin().await.unwrap();
        let empty = promotion_annual_sources(&mut tx, context.academic_year_id, &students)
            .await
            .unwrap();
        assert_eq!(empty.len(), 2);
        assert!(empty.values().all(Option::is_none));
        for bad in [
            vec![],
            vec![student, student],
            vec![Uuid::new_v4()],
            vec![student; 501],
        ] {
            assert!(
                promotion_annual_sources(&mut tx, context.academic_year_id, &bad)
                    .await
                    .is_err()
            );
        }
        assert!(promotion_annual_sources(&mut tx, Uuid::new_v4(), &students)
            .await
            .is_err());
        tx.rollback().await.unwrap();
        let policy = create_aggregate_policy(
            &pool,
            &actor,
            AggregatePolicyInput {
                name: "Promotion evidence".into(),
                passing_grade: "1".into(),
                minimum_learner_level: 1,
                allow_reviewed_holds: false,
            },
        )
        .await
        .unwrap();
        let query = AggregatePreviewQuery {
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            policy_id: policy.id,
        };
        let preview = preview_aggregate(&pool, &actor, student, &query)
            .await
            .unwrap();
        lock_term_aggregate(
            &pool,
            &actor,
            &context,
            student,
            AggregateLockInput {
                policy_id: policy.id,
                source_checksum: preview.source_checksum,
                expected_revision: None,
                request_id: Uuid::new_v4(),
                hold_reason: None,
            },
        )
        .await
        .unwrap();
        let annual = preview_annual(&pool, &actor, context.academic_year_id, student)
            .await
            .unwrap();
        let locked = lock_annual(
            &pool,
            &actor,
            context.academic_year_id,
            student,
            AnnualLockInput {
                expected_revision: None,
                source_checksum: annual.source_checksum,
                request_id: Uuid::new_v4(),
                hold_reason: None,
            },
        )
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let batch = promotion_annual_sources(&mut tx, context.academic_year_id, &students)
            .await
            .unwrap();
        assert!(batch.get(&extra_student).unwrap().is_none());
        let row = batch.get(&student).unwrap().as_ref().unwrap();
        assert_eq!(row.id, locked.id);
        assert_eq!(row.official_gpa.as_deref(), Some("0.00"));
        assert!(row.is_current);
        tx.rollback().await.unwrap();
        let course = &preview.results.courses[0];
        correct_result(
            &pool,
            &actor,
            &context,
            ResultCorrectionInput::Course {
                course_result_id: course.result_id.unwrap(),
                outcome: CourseOfficialOutcome::Numeric,
                numeric_grade: Some("4".into()),
                expected_effective_version: 1,
            },
        )
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let stale = promotion_annual_sources(&mut tx, context.academic_year_id, &[student])
            .await
            .unwrap();
        let row = stale.get(&student).unwrap().as_ref().unwrap();
        assert!(!row.is_current);
        assert_eq!(row.id, locked.id);
        assert_eq!(row.official_gpa.as_deref(), Some("0.00"));
        tx.rollback().await.unwrap();
    }
}
