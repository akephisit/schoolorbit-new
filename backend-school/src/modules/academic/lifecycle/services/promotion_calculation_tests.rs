use super::super::promotion_runs::{create_run, tests::fixture};
use super::*;
use crate::permissions::registry::codes;

#[tokio::test]
async fn promotion_calculation_keeps_missing_results_visible_and_replays_without_duplicate_items() {
    let (pool, actor, create) = fixture("promotion_calculation_missing").await;
    let run = create_run(&pool, &actor, create.clone()).await.unwrap();
    let before:String=sqlx::query_scalar("SELECT md5(coalesce(string_agg(row_to_json(s)::text,'' ORDER BY id),'')) FROM student_academic_years s").fetch_one(&pool).await.unwrap();
    let input = CalculatePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: run.row_version,
    };
    let calculated = calculate_run(&pool, &actor, run.id, input.clone())
        .await
        .unwrap();
    assert_eq!(calculated.run.status, PromotionRunStatus::Calculated);
    assert_eq!(calculated.run.row_version, 2);
    assert!(!calculated.items.is_empty());
    for item in &calculated.items {
        assert_eq!(item.run_id, run.id);
        assert_eq!(item.source_year_id, create.source_year_id);
        assert_eq!(item.target_year_id, create.target_year_id);
        assert!(item.annual_revision_id.is_none());
        assert!(item.recommendation.suggested_outcome.is_none());
        assert!(!item.recommendation.findings.is_empty());
        assert!(item.decision.is_none() && item.reviewed_by.is_none());
        assert_eq!(item.status, PromotionItemStatus::Calculated);
    }
    let replay = calculate_run(&pool, &actor, run.id, input.clone())
        .await
        .unwrap();
    assert_eq!(replay.run.row_version, 2);
    assert_eq!(
        replay.items.iter().map(|i| i.id).collect::<Vec<_>>(),
        calculated.items.iter().map(|i| i.id).collect::<Vec<_>>()
    );
    let after:String=sqlx::query_scalar("SELECT md5(coalesce(string_agg(row_to_json(s)::text,'' ORDER BY id),'')) FROM student_academic_years s").fetch_one(&pool).await.unwrap();
    assert_eq!(after, before);
    let mut stale = input.clone();
    stale.request_id = Uuid::new_v4();
    assert!(matches!(
        calculate_run(&pool, &actor, run.id, stale).await,
        Err(AppError::Conflict(_))
    ));
    let mut changed = input.clone();
    changed.row_version = 2;
    assert!(matches!(
        calculate_run(&pool, &actor, run.id, changed).await,
        Err(AppError::Conflict(_))
    ));
    let denied = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL.into()],
    };
    assert!(matches!(
        calculate_run(&pool, &denied, run.id, input).await,
        Err(AppError::Forbidden(_))
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_promotion_run_items WHERE run_id=$1")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, calculated.items.len() as i64);
}

#[tokio::test]
async fn promotion_calculation_rechecks_target_ownership_and_rolls_back_audit_failure() {
    let (pool, actor, create) = fixture("promotion_calculation_atomic").await;
    let run = create_run(&pool, &actor, create.clone()).await.unwrap();
    let student:Uuid=sqlx::query_scalar("SELECT id FROM student_academic_years WHERE academic_year_id=$1 AND status='active' ORDER BY id LIMIT 1").bind(create.source_year_id).fetch_one(&pool).await.unwrap();
    let target:Uuid=sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT gen_random_uuid(),student_id,$1,grade_level_id,study_program_id,'planned' FROM student_academic_years WHERE id=$2 RETURNING id").bind(create.target_year_id).bind(student).fetch_one(&pool).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_promotion_calc_audit() RETURNS trigger AS $$ BEGIN IF NEW.event_code='promotion_run.calculated' THEN RAISE EXCEPTION 'test audit failure'; END IF; RETURN NEW; END; $$ LANGUAGE plpgsql; CREATE TRIGGER test_promotion_calc_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_promotion_calc_audit();").execute(&pool).await.unwrap();
    let input = CalculatePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: 1,
    };
    assert!(matches!(
        calculate_run(&pool, &actor, run.id, input.clone()).await,
        Err(AppError::DbError(_))
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_promotion_run_items WHERE run_id=$1")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let version: i64 =
        sqlx::query_scalar("SELECT row_version FROM academic_promotion_runs WHERE id=$1")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, 1);
    sqlx::query("DROP TRIGGER test_promotion_calc_audit ON academic_audit_events")
        .execute(&pool)
        .await
        .unwrap();
    let calculated = calculate_run(&pool, &actor, run.id, input).await.unwrap();
    let item = calculated
        .items
        .iter()
        .find(|item| item.student_academic_year_id == student)
        .unwrap();
    assert_eq!(item.existing_target_student_year_id, Some(target));
    assert!(item.decision.is_none());
    let target_state: (String, i64) =
        sqlx::query_as("SELECT status,row_version FROM student_academic_years WHERE id=$1")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(target_state, ("planned".into(), 1));
}
