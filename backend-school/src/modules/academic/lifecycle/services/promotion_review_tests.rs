use super::*;
use crate::{
    modules::academic::{
        cutover_test_support::apply_migrations_through,
        results::{self, models as rm, services as rs},
    },
    permissions::registry::codes,
};

pub(crate) fn hold(row_version: i64) -> ReviewPromotionItemInput {
    ReviewPromotionItemInput {
        row_version,
        decision: PromotionDecisionInput {
            outcome: PromotionDecisionOutcome::Hold,
            target_grade_level_id: None,
            target_study_program_id: None,
            target_homeroom_id: None,
            reason: Some("รอฝ่ายวิชาการพิจารณาเงื่อนไขเพิ่มเติม".into()),
            condition: None,
        },
    }
}

pub(crate) async fn ready_run(
    name: &str,
) -> (
    PgPool,
    ActorContext,
    ActorContext,
    rm::ResultContext,
    PromotionRunCalculation,
) {
    ready_run_cohort(name, false).await
}

pub(crate) async fn ready_run_with_extra_student(
    name: &str,
) -> (
    PgPool,
    ActorContext,
    ActorContext,
    rm::ResultContext,
    PromotionRunCalculation,
) {
    ready_run_cohort(name, true).await
}

async fn ready_run_cohort(
    name: &str,
    extra_student: bool,
) -> (
    PgPool,
    ActorContext,
    ActorContext,
    rm::ResultContext,
    PromotionRunCalculation,
) {
    let (pool, results_actor, context, student) =
        results::aggregate_revision_tests::ready_aggregate_fixture_with_cohort(name, extra_student)
            .await;
    apply_migrations_through(&pool, 76).await.unwrap();
    sqlx::query(
        "UPDATE academic_terms SET included_in_year_result=(id=$2) WHERE academic_year_id=$1",
    )
    .bind(context.academic_year_id)
    .bind(context.academic_term_id)
    .execute(&pool)
    .await
    .unwrap();
    let policy = rs::create_aggregate_policy(
        &pool,
        &results_actor,
        rm::AggregatePolicyInput {
            name: "E2E-LIFECYCLE-promotion".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        },
    )
    .await
    .unwrap();
    let cohort: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM student_academic_years WHERE academic_year_id=$1 AND status='active' ORDER BY id").bind(context.academic_year_id).fetch_all(&pool).await.unwrap();
    for student in &cohort {
        let preview = rs::preview_aggregate(
            &pool,
            &results_actor,
            *student,
            &rm::AggregatePreviewQuery {
                academic_year_id: context.academic_year_id,
                academic_term_id: context.academic_term_id,
                policy_id: policy.id,
            },
        )
        .await
        .unwrap();
        rs::lock_term_aggregate(
            &pool,
            &results_actor,
            &context,
            *student,
            rm::AggregateLockInput {
                policy_id: policy.id,
                source_checksum: preview.source_checksum,
                expected_revision: None,
                request_id: Uuid::new_v4(),
                hold_reason: None,
            },
        )
        .await
        .unwrap();
        let preview = rs::preview_annual(&pool, &results_actor, context.academic_year_id, *student)
            .await
            .unwrap();
        rs::lock_annual(
            &pool,
            &results_actor,
            context.academic_year_id,
            *student,
            rm::AnnualLockInput {
                expected_revision: None,
                source_checksum: preview.source_checksum,
                request_id: Uuid::new_v4(),
                hold_reason: None,
            },
        )
        .await
        .unwrap();
    }
    let target:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) SELECT max(year)+1,'E2E-LIFECYCLE-target',max(end_date)+1,max(end_date)+366,'MON','planning' FROM academic_years RETURNING id").fetch_one(&pool).await.unwrap();
    let (grade, program): (Uuid, Uuid) = sqlx::query_as(
        "SELECT grade_level_id,study_program_id FROM student_academic_years WHERE id=$1",
    )
    .bind(student)
    .fetch_one(&pool)
    .await
    .unwrap();
    let rule = PromotionRuleInput {
        from_grade_level_id: grade,
        from_study_program_id: program,
        target_grade_level_id: None,
        target_study_program_id: None,
        success_outcome: PromotionSuccessOutcome::Graduate,
        minimum_earned_credits: "0".into(),
        require_no_exceptional_outcomes: true,
        require_activities_passed: true,
        minimum_learner_level: 1,
    };
    let policy:Uuid=sqlx::query_scalar("INSERT INTO academic_promotion_policy_versions(id,name,rules,progression_row_version,reviewed_by) VALUES(gen_random_uuid(),'E2E-LIFECYCLE-reviewed',$1,1,$2) RETURNING id").bind(sqlx::types::Json(vec![rule])).bind(results_actor.user_id).fetch_one(&pool).await.unwrap();
    let actor = ActorContext {
        user_id: results_actor.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL.into(),
        ],
    };
    let run = super::super::create_run(
        &pool,
        &actor,
        CreatePromotionRunInput {
            request_id: Uuid::new_v4(),
            source_year_id: context.academic_year_id,
            target_year_id: target,
            policy_id: policy,
        },
    )
    .await
    .unwrap();
    let calc = super::super::calculate_run(
        &pool,
        &actor,
        run.id,
        CalculatePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: run.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(calc.items.len(), if extra_student { 2 } else { 1 });
    assert!(calc.items[0].annual_revision_id.is_some());
    (pool, actor, results_actor, context, calc)
}

#[tokio::test]
async fn promotion_review_requires_manage_current_evidence_and_explicit_decision_without_moving_students(
) {
    let (pool, actor, results_actor, context, calc) = ready_run("promotion_review_current").await;
    let item = &calc.items[0];
    let before: String = sqlx::query_scalar(
        "SELECT md5(string_agg(row_to_json(s)::text,'' ORDER BY id)) FROM student_academic_years s",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for grants in [
        vec![],
        vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL],
        vec![codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL],
        vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL,
            codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL,
        ],
    ] {
        let denied = ActorContext {
            user_id: actor.user_id,
            permissions: grants.into_iter().map(String::from).collect(),
        };
        assert!(matches!(
            review_item(&pool, &denied, calc.run.id, item.id, hold(1)).await,
            Err(AppError::Forbidden(_))
        ));
    }
    let mut invalid = hold(1);
    invalid.decision.reason = None;
    assert!(matches!(
        review_item(&pool, &actor, calc.run.id, item.id, invalid).await,
        Err(AppError::ValidationError(_))
    ));
    assert!(matches!(
        review_item(&pool, &actor, Uuid::new_v4(), item.id, hold(1)).await,
        Err(AppError::NotFound(_))
    ));
    let reviewed = review_item(&pool, &actor, calc.run.id, item.id, hold(1))
        .await
        .unwrap();
    assert_eq!(reviewed.run.status, PromotionRunStatus::Reviewed);
    assert_eq!(reviewed.run.row_version, 3);
    assert_eq!(reviewed.item.status, PromotionItemStatus::Reviewed);
    assert_eq!(reviewed.item.row_version, 2);
    assert_eq!(reviewed.item.reviewed_by, Some(actor.user_id));
    assert_eq!(
        reviewed.item.decision.as_ref().unwrap().outcome,
        PromotionDecisionOutcome::Hold
    );
    assert!(matches!(
        review_item(&pool, &actor, calc.run.id, item.id, hold(1)).await,
        Err(AppError::Conflict(_))
    ));
    let saved: PromotionRunItem = sqlx::query_as(&format!(
        "SELECT {} FROM academic_promotion_run_items WHERE id=$1",
        super::super::promotion_calculation::ITEM_COLUMNS
    ))
    .bind(item.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(saved.row_version, 2);
    let result:Uuid=sqlx::query_scalar("SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1").bind(item.student_academic_year_id).fetch_one(&pool).await.unwrap();
    rs::correct_result(
        &pool,
        &results_actor,
        &context,
        rm::ResultCorrectionInput::Course {
            course_result_id: result,
            outcome: rm::CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        review_item(&pool, &actor, calc.run.id, item.id, hold(2)).await,
        Err(AppError::Conflict(_))
    ));
    let after: String = sqlx::query_scalar(
        "SELECT md5(string_agg(row_to_json(s)::text,'' ORDER BY id)) FROM student_academic_years s",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn promotion_review_cannot_override_missing_annual_results_with_a_reason() {
    let (pool, actor, input) =
        super::super::promotion_runs::tests::fixture("promotion_review_missing").await;
    let run = super::super::create_run(&pool, &actor, input)
        .await
        .unwrap();
    let calc = super::super::calculate_run(
        &pool,
        &actor,
        run.id,
        CalculatePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: 1,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        review_item(&pool, &actor, run.id, calc.items[0].id, hold(1)).await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn promotion_review_rolls_back_on_audit_failure_and_recalculation_requires_review_again() {
    let (pool, actor, _, _, calc) = ready_run("promotion_review_atomic").await;
    let item = &calc.items[0];
    sqlx::raw_sql("CREATE FUNCTION fail_promotion_review_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_code='promotion_item.reviewed' THEN RAISE EXCEPTION 'fixture audit unavailable'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_promotion_review_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_promotion_review_audit();").execute(&pool).await.unwrap();
    assert!(review_item(&pool, &actor, calc.run.id, item.id, hold(1))
        .await
        .is_err());
    let unchanged: (String, i64, Option<Uuid>) = sqlx::query_as(
        "SELECT status,row_version,reviewed_by FROM academic_promotion_run_items WHERE id=$1",
    )
    .bind(item.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unchanged, ("calculated".into(), 1, None));
    sqlx::raw_sql("DROP TRIGGER fail_promotion_review_audit ON academic_audit_events; DROP FUNCTION fail_promotion_review_audit();").execute(&pool).await.unwrap();
    let review = review_item(&pool, &actor, calc.run.id, item.id, hold(1))
        .await
        .unwrap();
    let calc = super::super::calculate_run(
        &pool,
        &actor,
        calc.run.id,
        CalculatePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: review.run.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(calc.run.status, PromotionRunStatus::Calculated);
    assert!(calc.run.approved_by.is_none());
    assert_eq!(calc.items[0].row_version, 3);
    assert!(calc.items[0].decision.is_none());
    assert!(calc.items[0].reviewed_by.is_none());
    assert!(matches!(
        review_item(&pool, &actor, calc.run.id, item.id, hold(2)).await,
        Err(AppError::Conflict(_))
    ));
}
