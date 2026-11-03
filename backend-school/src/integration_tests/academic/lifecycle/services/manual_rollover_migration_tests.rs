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
