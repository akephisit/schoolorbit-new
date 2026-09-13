use super::super::{
    promotion_run_review::{
        review_item,
        tests::{hold, ready_run},
    },
    promotion_runs::{create_run, tests::fixture},
};
use super::*;
use crate::permissions::registry::codes;

#[tokio::test]
async fn promotion_workspace_reader_lists_exact_years_and_cannot_borrow_a_foreign_cursor() {
    let (pool, actor, create) = fixture("promotion_workspace_list").await;
    let run = create_run(&pool, &actor, create.clone()).await.unwrap();
    let reader = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL.into()],
    };
    let query = || PromotionRunListQuery {
        source_year_id: run.source_year_id,
        target_year_id: Some(run.target_year_id),
        before_id: None,
    };
    let denied = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL.into()],
    };
    assert!(matches!(
        list_runs(&pool, &denied, query()).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        get_run_workspace(&pool, &denied, run.id).await,
        Err(AppError::Forbidden(_))
    ));
    let list = list_runs(&pool, &reader, query()).await.unwrap();
    assert_eq!(list.runs.len(), 1);
    assert_eq!(list.runs[0].id, run.id);
    assert!(list.next_cursor.is_none());
    let mut missing = query();
    missing.before_id = Some(Uuid::new_v4());
    assert!(matches!(
        list_runs(&pool, &reader, missing).await,
        Err(AppError::NotFound(_))
    ));
    let mut other = query();
    other.source_year_id = run.target_year_id;
    other.target_year_id = None;
    other.before_id = Some(run.id);
    assert!(matches!(
        list_runs(&pool, &reader, other.clone()).await,
        Err(AppError::NotFound(_))
    ));
    other.before_id = None;
    assert!(list_runs(&pool, &reader, other)
        .await
        .unwrap()
        .runs
        .is_empty());
    let workspace = get_run_workspace(&pool, &reader, run.id).await.unwrap();
    assert_eq!(workspace.source_year.academic_year_id, run.source_year_id);
    assert_eq!(workspace.target_year.academic_year_id, run.target_year_id);
    assert!(workspace.students.is_empty());
    assert!(matches!(
        get_run_workspace(&pool, &reader, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn promotion_workspace_pagination_keeps_equal_timestamp_runs_distinct() {
    let (pool, actor, create) = fixture("promotion_workspace_pages").await;
    let run = create_run(&pool, &actor, create).await.unwrap();
    sqlx::query("INSERT INTO academic_promotion_runs(source_year_id,target_year_id,policy_id,request_id,request_checksum,created_by) SELECT source_year_id,target_year_id,policy_id,gen_random_uuid(),request_checksum,created_by FROM academic_promotion_runs CROSS JOIN generate_series(1,55) WHERE id=$1")
        .bind(run.id).execute(&pool).await.unwrap();
    let query = PromotionRunListQuery {
        source_year_id: run.source_year_id,
        target_year_id: None,
        before_id: None,
    };
    let first = list_runs(&pool, &actor, query.clone()).await.unwrap();
    assert_eq!(first.runs.len(), 50);
    let second = list_runs(
        &pool,
        &actor,
        PromotionRunListQuery {
            before_id: first.next_cursor,
            ..query
        },
    )
    .await
    .unwrap();
    assert_eq!(second.runs.len(), 6);
    assert!(second.next_cursor.is_none());
    let ids: std::collections::BTreeSet<_> = first
        .runs
        .iter()
        .chain(&second.runs)
        .map(|row| row.id)
        .collect();
    assert_eq!(ids.len(), 56);
}

#[tokio::test]
async fn promotion_workspace_rehydrates_student_names_and_exact_approval_intent_without_writes() {
    let (pool, reviewer, _, _, calc) = ready_run("promotion_workspace_detail").await;
    let reader = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL.into()],
    };
    let workspace = get_run_workspace(&pool, &reader, calc.run.id)
        .await
        .unwrap();
    assert_eq!(workspace.students.len(), 1);
    assert!(!workspace.students[0].student_name.is_empty());
    assert!(workspace.students[0].annual_result_current);
    assert!(!workspace.students[0].needs_recalculation);
    assert!(workspace.students[0].receipt.is_none());
    assert_eq!(
        workspace.approval_checksum,
        super::super::promotion_approval::intent_checksum(&calc.run, &calc.items).unwrap()
    );
    let review = review_item(&pool, &reviewer, calc.run.id, calc.items[0].id, hold(1))
        .await
        .unwrap();
    let after = get_run_workspace(&pool, &reader, calc.run.id)
        .await
        .unwrap();
    assert_eq!(after.run.row_version, review.run.row_version);
    assert_eq!(after.students[0].item.row_version, review.item.row_version);
    assert_ne!(after.approval_checksum, workspace.approval_checksum);
    let serialized = serde_json::to_value(after).unwrap();
    assert!(!serialized.to_string().contains("nationalId"));
    assert!(serde_json::from_value::<PromotionRunListQuery>(
        serde_json::json!({"source_year_id":calc.run.source_year_id})
    )
    .is_err());
}
