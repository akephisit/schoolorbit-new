use super::*;
use crate::modules::academic::lifecycle::models::{
    ExecutePromotionRunInput, PromotionDecisionOutcome,
};
use school_permissions::registry::codes;

#[tokio::test]
async fn year_reopening_inflight_execution_blocks_before_any_receipt_exists() {
    let (pool, manager, calc, run) =
        super::promotion_execution_tests::started_run_fixture("year_reopening_inflight").await;
    let year = calc.run.source_year_id;
    super::year_reopening_tests::set_closed(&pool, year).await;
    let reader = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    let pending = get_year_reopening_workspace(&pool, &reader, year)
        .await
        .unwrap();
    assert!(!pending.can_reopen);
    assert!(pending
        .findings
        .iter()
        .any(|finding| finding.code == "year.reopen_execution_pending"));
    sqlx::query(
        "UPDATE academic_promotion_runs SET status='failed',row_version=row_version+1 WHERE id=$1",
    )
    .bind(run.id)
    .execute(&pool)
    .await
    .unwrap();
    let failed = get_year_reopening_workspace(&pool, &reader, year)
        .await
        .unwrap();
    assert!(
        failed.can_reopen,
        "a failed attempt without committed items does not invent a completed dependency"
    );
    assert_ne!(failed.source_checksum, pending.source_checksum);
}

#[tokio::test]
async fn year_reopening_completed_hold_preserves_a_blocking_dependency() {
    let (pool, executor, _, _, calc, approved) =
        super::promotion_execution_tests::approved_fixture(
            "year_reopening_executed",
            PromotionDecisionOutcome::Hold,
        )
        .await;
    execute_run(
        &pool,
        &executor,
        calc.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 1,
        },
    )
    .await
    .unwrap();
    let year = calc.run.source_year_id;
    super::year_reopening_tests::set_closed(&pool, year).await;
    let reader = ActorContext {
        user_id: executor.user_id,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    let workspace = get_year_reopening_workspace(&pool, &reader, year)
        .await
        .unwrap();
    assert!(!workspace.can_reopen);
    let finding = workspace
        .findings
        .iter()
        .find(|finding| finding.code == "year.reopen_promotion")
        .unwrap();
    assert_eq!(finding.count, 1);
    assert!(finding.resolution_url.is_none());
    let office = ActorContext {
        user_id: reader.user_id,
        permissions: vec![
            codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
        ],
    };
    let office_view = get_year_reopening_workspace(&pool, &office, year)
        .await
        .unwrap();
    assert_eq!(workspace.source_checksum, office_view.source_checksum);
    assert!(office_view
        .findings
        .iter()
        .find(|finding| finding.code == "year.reopen_promotion")
        .unwrap()
        .resolution_url
        .is_some());
}
