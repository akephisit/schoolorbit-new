use super::*;
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::{
        lifecycle::models::*,
        results::{models as rm, services as rs},
    },
    permissions::registry::codes,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn retained_state(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT md5(jsonb_build_object(
        'students',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM student_academic_years r),
        'placements',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM homeroom_placements r),
        'runs',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_promotion_runs r),
        'items',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_promotion_run_items r),
        'approvals',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_promotion_run_approvals r),
        'receipts',(SELECT jsonb_agg(to_jsonb(r) ORDER BY item_id) FROM academic_promotion_execution_receipts r)
    )::text)").fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn promotion_impact_projection_preserves_executed_hold_transfer_and_planned_placement() {
    for (name, outcome) in [
        ("promotion_impact_hold", PromotionDecisionOutcome::Hold),
        (
            "promotion_impact_transfer",
            PromotionDecisionOutcome::TransferOut,
        ),
        (
            "promotion_impact_promote",
            PromotionDecisionOutcome::Promote,
        ),
    ] {
        let (pool, executor, corrector, context, calc, approved) =
            super::promotion_execution::tests::approved_fixture(name, outcome).await;
        crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
            .await
            .unwrap();
        let done = execute_run(
            &pool,
            &executor,
            calc.run.id,
            ExecutePromotionRunInput {
                request_id: Uuid::new_v4(),
                row_version: approved.row_version,
                limit: 100,
            },
        )
        .await
        .unwrap();
        assert!(done.failures.is_empty());
        assert_eq!(done.receipts.len(), 1);
        let reader = ActorContext {
            user_id: executor.user_id,
            permissions: vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL.into()],
        };
        let initial = get_promotion_impacts(
            &pool,
            &reader,
            calc.run.id,
            PromotionImpactQuery { after_id: None },
        )
        .await
        .unwrap();
        assert!(initial.impacts.is_empty());
        let before = retained_state(&pool).await;
        let course: Uuid = sqlx::query_scalar("SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1").bind(calc.items[0].student_academic_year_id).fetch_one(&pool).await.unwrap();
        let correction = rs::correct_result(
            &pool,
            &corrector,
            &context,
            rm::ResultCorrectionInput::Course {
                course_result_id: course,
                outcome: rm::CourseOfficialOutcome::Numeric,
                numeric_grade: Some("4".into()),
                expected_effective_version: 1,
            },
        )
        .await
        .unwrap();
        let impact = get_promotion_impacts(
            &pool,
            &reader,
            calc.run.id,
            PromotionImpactQuery { after_id: None },
        )
        .await
        .unwrap();
        assert_eq!(impact.impacts.len(), 1);
        assert_eq!(impact.total_count, 1);
        assert_eq!(impact.impacts[0].item_id, calc.items[0].id);
        assert_eq!(
            impact.impacts[0].student_academic_year_id,
            calc.items[0].student_academic_year_id
        );
        assert_eq!(
            impact.impacts[0].evidence.correction.id,
            correction.corrections.last().unwrap().id
        );
        assert_ne!(impact.source_checksum, initial.source_checksum);
        let refreshed = get_promotion_impacts(
            &pool,
            &reader,
            calc.run.id,
            PromotionImpactQuery { after_id: None },
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(&impact).unwrap(),
            serde_json::to_value(refreshed).unwrap()
        );
        let tail = get_promotion_impacts(
            &pool,
            &reader,
            calc.run.id,
            PromotionImpactQuery {
                after_id: Some(impact.impacts[0].id),
            },
        )
        .await
        .unwrap();
        assert!(tail.impacts.is_empty());
        assert!(tail.next_cursor.is_none());
        assert_eq!(retained_state(&pool).await, before);
    }
}

#[tokio::test]
async fn promotion_impact_projection_ignores_unexecuted_stale_items_and_requires_read_scope() {
    let (pool, executor, corrector, context, calc, _) =
        super::promotion_execution::tests::approved_fixture(
            "promotion_impact_unexecuted",
            PromotionDecisionOutcome::Hold,
        )
        .await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
        .await
        .unwrap();
    let course: Uuid = sqlx::query_scalar("SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1").bind(calc.items[0].student_academic_year_id).fetch_one(&pool).await.unwrap();
    rs::correct_result(
        &pool,
        &corrector,
        &context,
        rm::ResultCorrectionInput::Course {
            course_result_id: course,
            outcome: rm::CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let before = retained_state(&pool).await;
    assert!(get_promotion_impacts(
        &pool,
        &executor,
        calc.run.id,
        PromotionImpactQuery { after_id: None }
    )
    .await
    .unwrap()
    .impacts
    .is_empty());
    for permissions in [
        vec![],
        vec![codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL.into()],
        vec![codes::ACADEMIC_PROMOTION_CORRECT_SCHOOL.into()],
    ] {
        let denied = ActorContext {
            user_id: executor.user_id,
            permissions,
        };
        assert!(matches!(
            get_promotion_impacts(
                &pool,
                &denied,
                calc.run.id,
                PromotionImpactQuery { after_id: None }
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
    }
    assert!(matches!(
        get_promotion_impacts(
            &pool,
            &executor,
            Uuid::new_v4(),
            PromotionImpactQuery { after_id: None }
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        get_promotion_impacts(
            &pool,
            &executor,
            calc.run.id,
            PromotionImpactQuery {
                after_id: Some(Uuid::new_v4())
            }
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        get_promotion_impacts(
            &pool,
            &executor,
            Uuid::nil(),
            PromotionImpactQuery { after_id: None }
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    assert_eq!(retained_state(&pool).await, before);
}
