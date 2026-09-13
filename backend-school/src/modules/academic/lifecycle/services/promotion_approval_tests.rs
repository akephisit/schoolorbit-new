use super::super::promotion_run_review::{
    review_item,
    tests::{hold, ready_run},
};
use super::*;
use crate::{
    modules::academic::results::{models as rm, services as rs},
    permissions::registry::codes,
};

fn approver(actor: &ActorContext) -> ActorContext {
    ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL.into(),
        ],
    }
}
fn request(review: &PromotionItemReview) -> ApprovePromotionRunInput {
    ApprovePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: review.run.row_version,
        source_checksum: intent_checksum(&review.run, std::slice::from_ref(&review.item)).unwrap(),
    }
}

#[tokio::test]
async fn promotion_approval_request_collision_with_calculation_is_a_conflict_not_decode_failure() {
    let (pool, reviewer, _, _, calc) = ready_run("promotion_approval_command_collision").await;
    let review = review_item(&pool, &reviewer, calc.run.id, calc.items[0].id, hold(1))
        .await
        .unwrap();
    let actor = approver(&reviewer);
    let input = request(&review);
    let calculation_request:Uuid=sqlx::query_scalar("SELECT request_id FROM academic_promotion_run_commands WHERE run_id=$1 AND action='calculate'").bind(calc.run.id).fetch_one(&pool).await.unwrap();
    let mut collision = input.clone();
    collision.request_id = calculation_request;
    let approve_collision = approve_run(&pool, &actor, calc.run.id, collision).await;
    let approved = approve_run(&pool, &actor, calc.run.id, input.clone())
        .await
        .unwrap();
    let calculate_collision = super::super::calculate_run(
        &pool,
        &reviewer,
        calc.run.id,
        CalculatePromotionRunInput {
            request_id: input.request_id,
            row_version: approved.row_version,
        },
    )
    .await;
    assert_eq!(
        (
            matches!(approve_collision, Err(AppError::Conflict(_))),
            matches!(calculate_collision, Err(AppError::Conflict(_)))
        ),
        (true, true)
    );
}

#[tokio::test]
async fn promotion_approval_requires_separate_permission_exact_review_and_actor_bound_replay() {
    let (pool, reviewer, _, _, calc) = ready_run("promotion_approval_scope").await;
    let review = review_item(&pool, &reviewer, calc.run.id, calc.items[0].id, hold(1))
        .await
        .unwrap();
    let actor = approver(&reviewer);
    let input = request(&review);
    for grants in [
        vec![],
        vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL],
        vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL,
            codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL,
        ],
        vec![codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL],
    ] {
        let denied = ActorContext {
            user_id: actor.user_id,
            permissions: grants.into_iter().map(String::from).collect(),
        };
        assert!(matches!(
            approve_run(&pool, &denied, calc.run.id, input.clone()).await,
            Err(AppError::Forbidden(_))
        ));
    }
    let mut stale = input.clone();
    stale.row_version -= 1;
    assert!(matches!(
        approve_run(&pool, &actor, calc.run.id, stale).await,
        Err(AppError::Conflict(_))
    ));
    let mut stale = input.clone();
    stale.source_checksum = "a".repeat(64);
    assert!(matches!(
        approve_run(&pool, &actor, calc.run.id, stale).await,
        Err(AppError::Conflict(_))
    ));
    let approved = approve_run(&pool, &actor, calc.run.id, input.clone())
        .await
        .unwrap();
    assert_eq!(approved.status, PromotionRunStatus::Approved);
    assert_eq!(approved.row_version, 4);
    assert_eq!(approved.approved_by, Some(actor.user_id));
    let replay = approve_run(&pool, &actor, calc.run.id, input.clone())
        .await
        .unwrap();
    assert_eq!(replay.row_version, 4);
    assert_eq!(replay.approved_at, approved.approved_at);
    let mut other = actor.clone();
    other.user_id = Uuid::new_v4();
    assert!(matches!(
        approve_run(&pool, &other, calc.run.id, input.clone()).await,
        Err(AppError::Conflict(_))
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_promotion_run_approvals WHERE run_id=$1")
            .bind(calc.run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='promotion_run.approved'").bind(calc.run.id).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn promotion_approval_rejects_unreviewed_or_corrected_sources() {
    let (pool, reviewer, results_actor, context, calc) =
        ready_run("promotion_approval_stale").await;
    let actor = approver(&reviewer);
    let input = ApprovePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: calc.run.row_version,
        source_checksum: intent_checksum(&calc.run, &calc.items).unwrap(),
    };
    assert!(matches!(
        approve_run(&pool, &actor, calc.run.id, input).await,
        Err(AppError::Conflict(_))
    ));
    let review = review_item(&pool, &reviewer, calc.run.id, calc.items[0].id, hold(1))
        .await
        .unwrap();
    let input = request(&review);
    let result:Uuid=sqlx::query_scalar("SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1").bind(review.item.student_academic_year_id).fetch_one(&pool).await.unwrap();
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
        approve_run(&pool, &actor, calc.run.id, input).await,
        Err(AppError::Conflict(_))
    ));
    let state: (String, i64) =
        sqlx::query_as("SELECT status,row_version FROM academic_promotion_runs WHERE id=$1")
            .bind(calc.run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, ("reviewed".into(), 3));
}

#[tokio::test]
async fn promotion_approval_audit_is_atomic_and_later_review_preserves_immutable_intent() {
    let (pool, reviewer, _, _, calc) = ready_run("promotion_approval_atomic").await;
    let review = review_item(&pool, &reviewer, calc.run.id, calc.items[0].id, hold(1))
        .await
        .unwrap();
    let actor = approver(&reviewer);
    let input = request(&review);
    sqlx::raw_sql("CREATE FUNCTION fail_promotion_approval_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_code='promotion_run.approved' THEN RAISE EXCEPTION 'fixture audit unavailable'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_promotion_approval_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_promotion_approval_audit();").execute(&pool).await.unwrap();
    assert!(approve_run(&pool, &actor, calc.run.id, input.clone())
        .await
        .is_err());
    let state: (String, i64) =
        sqlx::query_as("SELECT status,row_version FROM academic_promotion_runs WHERE id=$1")
            .bind(calc.run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, ("reviewed".into(), 3));
    sqlx::raw_sql("DROP TRIGGER fail_promotion_approval_audit ON academic_audit_events; DROP FUNCTION fail_promotion_approval_audit();").execute(&pool).await.unwrap();
    approve_run(&pool, &actor, calc.run.id, input)
        .await
        .unwrap();
    let intent: sqlx::types::Json<PromotionRunCalculation> =
        sqlx::query_scalar("SELECT intent FROM academic_promotion_run_approvals WHERE run_id=$1")
            .bind(calc.run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut changed = hold(2);
    changed.decision.reason = Some("ฝ่ายวิชาการขอตรวจหลักฐานใหม่".into());
    let revised = review_item(&pool, &reviewer, calc.run.id, review.item.id, changed)
        .await
        .unwrap();
    assert_eq!(revised.run.status, PromotionRunStatus::Reviewed);
    assert!(revised.run.approved_by.is_none());
    let retained: sqlx::types::Json<PromotionRunCalculation> =
        sqlx::query_scalar("SELECT intent FROM academic_promotion_run_approvals WHERE run_id=$1")
            .bind(calc.run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        retained.items[0].decision.as_ref().unwrap().reason,
        intent.items[0].decision.as_ref().unwrap().reason
    );
    assert_ne!(
        retained.items[0].decision.as_ref().unwrap().reason,
        revised.item.decision.as_ref().unwrap().reason
    );
    assert!(
        sqlx::query("DELETE FROM academic_promotion_run_approvals WHERE run_id=$1")
            .bind(calc.run.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE academic_promotion_run_approvals SET source_checksum=repeat('b',64) WHERE run_id=$1").bind(calc.run.id).execute(&pool).await.is_err());
}
