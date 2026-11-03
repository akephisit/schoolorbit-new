use super::*;
use crate::integration_tests::academic::{core, cutover_test_support::apply_migrations_through};
use school_permissions::registry::codes;

async fn legacy(name: &str) -> (PgPool, ActorContext, PromotionRunCalculation) {
    let pool = core::services_tests::prepare_core_fixture_through(name, 151).await;
    let actor = ActorContext {
        user_id: sqlx::query_scalar(
            "SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        permissions: vec![codes::WILDCARD.into()],
    };
    let (source,grade,program):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT academic_year_id,grade_level_id,study_program_id FROM student_academic_years WHERE status='active' ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();
    let next: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE id<>$1 ORDER BY id LIMIT 1")
            .bind(grade)
            .fetch_one(&pool)
            .await
            .unwrap();
    let target:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,school_days,status) SELECT max(year)+1,'Legacy roster fixture','MON','planning' FROM academic_years RETURNING id").fetch_one(&pool).await.unwrap();
    let policy:Uuid=sqlx::query_scalar("INSERT INTO academic_promotion_policy_versions(id,name,rules,progression_row_version,reviewed_by) VALUES(uuidv7(),'Legacy policy',$1,1,$2) RETURNING id")
        .bind(sqlx::types::Json(vec![PromotionRuleInput{from_grade_level_id:grade,from_study_program_id:program,target_grade_level_id:Some(next),target_study_program_id:Some(program),success_outcome:PromotionSuccessOutcome::Promote,minimum_earned_credits:"0".into(),require_no_exceptional_outcomes:false,require_activities_passed:false,minimum_learner_level:0}])).bind(actor.user_id).fetch_one(&pool).await.unwrap();
    let run = create_run(
        &pool,
        &actor,
        CreatePromotionRunInput {
            request_id: Uuid::new_v4(),
            source_year_id: source,
            target_year_id: target,
            target_placement_starts_on: chrono::Utc::now().date_naive(),
            policy_id: policy,
        },
    )
    .await
    .unwrap();
    let calculation = calculate_run(
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
    (pool, actor, calculation)
}

async fn facts(pool: &PgPool) -> serde_json::Value {
    sqlx::query_scalar::<_,sqlx::types::Json<serde_json::Value>>("SELECT jsonb_build_object('runs',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_promotion_runs r),'items',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_promotion_run_items r),'commands',(SELECT jsonb_agg(to_jsonb(r) ORDER BY request_id) FROM academic_promotion_run_commands r),'approvals',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_promotion_run_approvals r),'batches',(SELECT jsonb_agg(to_jsonb(r) ORDER BY request_id) FROM academic_promotion_execution_batches r),'receipts',(SELECT jsonb_agg(to_jsonb(r) ORDER BY item_id) FROM academic_promotion_execution_receipts r))").fetch_one(pool).await.unwrap().0
}

#[tokio::test]
async fn manual_year_rollover152_preserves_populated_legacy_history_and_retry_without_invented_evidence(
) {
    let (pool, _, calculation) = legacy("manual_rollover152_preservation").await;
    assert!(!calculation.items.is_empty());
    let before = facts(&pool).await;
    apply_migrations_through(&pool, 152).await.unwrap();
    assert_eq!(facts(&pool).await, before);
    apply_migrations_through(&pool, 152).await.unwrap();
    assert_eq!(facts(&pool).await, before);
    let nullable:bool=sqlx::query_scalar("SELECT NOT attnotnull FROM pg_attribute WHERE attrelid='academic_promotion_runs'::regclass AND attname='policy_id'").fetch_one(&pool).await.unwrap();
    assert!(nullable);
}

#[tokio::test]
async fn manual_year_rollover152_refuses_executing_and_partially_unreconciled_batches_atomically() {
    for partial in [false, true] {
        let (pool, actor, calculation) = legacy(if partial {
            "manual_rollover152_partial"
        } else {
            "manual_rollover152_executing"
        })
        .await;
        let approval = Uuid::new_v4();
        sqlx::query("INSERT INTO academic_promotion_run_approvals(id,run_id,approved_run_version,source_checksum,intent,approved_by) VALUES($1,$2,1,repeat('a',64),$3,$4)")
            .bind(approval).bind(calculation.run.id).bind(sqlx::types::Json(&calculation)).bind(actor.user_id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE academic_promotion_runs SET approval_id=$2,approved_by=$3,approved_at=now(),status=$4 WHERE id=$1")
            .bind(calculation.run.id).bind(approval).bind(actor.user_id).bind(if partial {"failed"}else{"executing"}).execute(&pool).await.unwrap();
        if partial {
            sqlx::query("INSERT INTO academic_promotion_execution_batches(request_id,run_id,approval_id,actor_user_id,request_checksum,item_ids) VALUES(uuidv7(),$1,$2,$3,repeat('a',64),$4)")
                .bind(calculation.run.id).bind(approval).bind(actor.user_id).bind(vec![calculation.items[0].id]).execute(&pool).await.unwrap();
        }
        let before = facts(&pool).await;
        let error = apply_migrations_through(&pool, 152).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("MANUAL_ROLLOVER_LEGACY_EXECUTION_UNRESOLVED"));
        assert_eq!(facts(&pool).await, before);
        let still_required:bool=sqlx::query_scalar("SELECT attnotnull FROM pg_attribute WHERE attrelid='academic_promotion_runs'::regclass AND attname='policy_id'").fetch_one(&pool).await.unwrap();
        assert!(still_required);
    }
}

#[tokio::test]
async fn annual_membership154_preserves_populated_history_and_interval_truth() {
    let (pool, actor, calculation) = legacy("annual_membership154_preservation").await;
    apply_migrations_through(&pool, 153).await.unwrap();
    let run = calculation.run.id;
    let approval = Uuid::now_v7();
    let batch = Uuid::now_v7();
    let item = &calculation.items[0];
    sqlx::query("INSERT INTO academic_promotion_run_approvals(id,run_id,approved_run_version,source_checksum,intent,approved_by) VALUES($1,$2,1,repeat('a',64),$3,$4)")
        .bind(approval).bind(run).bind(sqlx::types::Json(&calculation)).bind(actor.user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_promotion_execution_batches(request_id,run_id,approval_id,actor_user_id,request_checksum,item_ids) VALUES($1,$2,$3,$4,repeat('a',64),$5)")
        .bind(batch).bind(run).bind(approval).bind(actor.user_id).bind(vec![item.id]).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_promotion_execution_receipts(item_id,run_id,request_id,approval_id,source_year_id,target_year_id,student_id,source_row_version,executed_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(item.id).bind(run).bind(batch).bind(approval).bind(item.source_year_id).bind(item.target_year_id).bind(item.student_id).bind(item.source_row_version).bind(actor.user_id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_promotion_run_items SET status='executed',decision=jsonb_build_object('outcome','hold'),reviewed_by=$2,reviewed_at=now() WHERE id=$1")
        .bind(item.id).bind(actor.user_id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_promotion_runs SET approval_id=$2,approved_by=$3,approved_at=now(),status='completed',executed_by=$3,executed_at=now() WHERE id=$1")
        .bind(run).bind(approval).bind(actor.user_id).execute(&pool).await.unwrap();
    let before = facts(&pool).await;
    let placements: serde_json::Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(p) ORDER BY id),'[]') FROM homeroom_placements p",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    apply_migrations_through(&pool, 154).await.unwrap();
    apply_migrations_through(&pool, 154).await.unwrap();
    assert_eq!(facts(&pool).await, before);
    let after: serde_json::Value = sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(p)-'placement_scope' ORDER BY id),'[]') FROM homeroom_placements p")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(after, placements);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM homeroom_placements WHERE placement_scope<>'dated'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert!(sqlx::query("UPDATE academic_promotion_execution_receipts SET source_row_version=source_row_version+1 WHERE item_id=$1").bind(item.id).execute(&pool).await.is_err());
    assert!(sqlx::query(
        "UPDATE academic_promotion_runs SET target_placement_starts_on=NULL WHERE id=$1"
    )
    .bind(run)
    .execute(&pool)
    .await
    .is_err());
    assert_eq!(facts(&pool).await, before);
    let date = chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    for (scope, start, end) in [
        ("academic_year", None, None),
        ("academic_year", None, Some(date.pred_opt().unwrap())),
        ("academic_year", None, Some(date)),
        ("dated", Some(date), None),
        ("dated", Some(date.succ_opt().unwrap()), None),
    ] {
        let sql: bool = sqlx::query_scalar("SELECT academic_placement_applies_on($1,$2,$3,$4)")
            .bind(scope)
            .bind(start)
            .bind(end)
            .bind(date)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            sql,
            school_academic_core::services::placement_interval::contains_date(start, end, date)
        );
    }
}

#[tokio::test]
async fn annual_membership154_refuses_unresolved_execution_before_schema_changes() {
    for partial in [false, true] {
        let (pool, actor, calculation) = legacy(if partial {
            "annual154_partial"
        } else {
            "annual154_executing"
        })
        .await;
        apply_migrations_through(&pool, 153).await.unwrap();
        let approval = Uuid::now_v7();
        sqlx::query("INSERT INTO academic_promotion_run_approvals(id,run_id,approved_run_version,source_checksum,intent,approved_by) VALUES($1,$2,1,repeat('a',64),$3,$4)")
            .bind(approval).bind(calculation.run.id).bind(sqlx::types::Json(&calculation)).bind(actor.user_id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE academic_promotion_runs SET approval_id=$2,approved_by=$3,approved_at=now(),status=$4 WHERE id=$1")
            .bind(calculation.run.id).bind(approval).bind(actor.user_id).bind(if partial { "failed" } else { "executing" }).execute(&pool).await.unwrap();
        if partial {
            sqlx::query("INSERT INTO academic_promotion_execution_batches(request_id,run_id,approval_id,actor_user_id,request_checksum,item_ids) VALUES(uuidv7(),$1,$2,$3,repeat('a',64),$4)")
                .bind(calculation.run.id).bind(approval).bind(actor.user_id).bind(vec![calculation.items[0].id]).execute(&pool).await.unwrap();
        }
        let before = facts(&pool).await;
        assert!(apply_migrations_through(&pool, 154)
            .await
            .unwrap_err()
            .to_string()
            .contains("ACADEMIC_MEMBERSHIP154_EXECUTION_UNRESOLVED"));
        assert_eq!(facts(&pool).await, before);
        assert!(!sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid='homeroom_placements'::regclass AND attname='placement_scope' AND NOT attisdropped)").fetch_one(&pool).await.unwrap());
    }
}
