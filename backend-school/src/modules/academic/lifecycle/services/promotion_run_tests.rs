use super::*;
use crate::{
    modules::academic::{
        core,
        cutover_test_support::apply_migrations_through,
        lifecycle::models::{PromotionRuleInput, PromotionRunStatus, PromotionSuccessOutcome},
    },
    permissions::registry::codes,
};
use uuid::Uuid;

pub(crate) async fn fixture(name: &str) -> (PgPool, ActorContext, CreatePromotionRunInput) {
    let pool = core::services_tests::prepare_core_fixture(name).await;
    apply_migrations_through(&pool, 76).await.unwrap();
    let actor = ActorContext {
        user_id: sqlx::query_scalar(
            "SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL.into(),
        ],
    };
    let source_year_id = sqlx::query_scalar(
        "SELECT id FROM academic_years WHERE status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let target_year_id = sqlx::query_scalar("INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) SELECT max(year)+1,'E2E-LIFECYCLE-next',max(end_date)+1,max(end_date)+366,'MON,TUE,WED,THU,FRI','planning' FROM academic_years RETURNING id").fetch_one(&pool).await.unwrap();
    let (grade, next): (Uuid,Uuid) = sqlx::query_as("SELECT from_grade_level_id,to_grade_level_id FROM grade_level_progressions WHERE transition_kind='promote' AND to_grade_level_id IS NOT NULL LIMIT 1").fetch_one(&pool).await.unwrap();
    let program = sqlx::query_scalar("SELECT id FROM study_programs ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let rule = PromotionRuleInput {
        from_grade_level_id: grade,
        from_study_program_id: program,
        target_grade_level_id: Some(next),
        target_study_program_id: Some(program),
        success_outcome: PromotionSuccessOutcome::Promote,
        minimum_earned_credits: "10".into(),
        require_no_exceptional_outcomes: true,
        require_activities_passed: true,
        minimum_learner_level: 1,
    };
    let policy_id = sqlx::query_scalar("INSERT INTO academic_promotion_policy_versions(id,name,rules,progression_row_version,reviewed_by) VALUES (gen_random_uuid(),'E2E-LIFECYCLE-policy',$1,1,$2) RETURNING id")
        .bind(sqlx::types::Json(vec![rule])).bind(actor.user_id).fetch_one(&pool).await.unwrap();
    (
        pool,
        actor,
        CreatePromotionRunInput {
            request_id: Uuid::new_v4(),
            source_year_id,
            target_year_id,
            policy_id,
        },
    )
}

#[tokio::test]
async fn promotion_run_creation_replays_only_its_actor_and_intent_without_student_writes() {
    let (pool, actor, input) = fixture("promotion_run_creation").await;
    let before: String = sqlx::query_scalar("SELECT md5(coalesce(string_agg(row_to_json(s)::text,'' ORDER BY id),'')) FROM student_academic_years s").fetch_one(&pool).await.unwrap();
    let first = create_run(&pool, &actor, input.clone()).await.unwrap();
    assert_eq!(first.status, PromotionRunStatus::Draft);
    assert_eq!(first.row_version, 1);
    assert_eq!(first.source_year_id, input.source_year_id);
    assert_eq!(first.target_year_id, input.target_year_id);
    assert_eq!(first.policy_id, input.policy_id);
    let again = create_run(&pool, &actor, input.clone()).await.unwrap();
    assert_eq!(again.id, first.id);
    let mut changed = input.clone();
    changed.policy_id = Uuid::new_v4();
    assert!(matches!(
        create_run(&pool, &actor, changed).await,
        Err(AppError::Conflict(_))
    ));
    let mut other = actor.clone();
    other.user_id = Uuid::new_v4();
    assert!(matches!(
        create_run(&pool, &other, input.clone()).await,
        Err(AppError::Conflict(_))
    ));
    let after: String = sqlx::query_scalar("SELECT md5(coalesce(string_agg(row_to_json(s)::text,'' ORDER BY id),'')) FROM student_academic_years s").fetch_one(&pool).await.unwrap();
    assert_eq!(after, before);
    let audits:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE event_code='promotion_run.created' AND entity_id=$1").bind(first.id).fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 1);
    assert!(sqlx::query(
        "UPDATE academic_promotion_runs SET request_checksum=repeat('b',64) WHERE id=$1"
    )
    .bind(first.id)
    .execute(&pool)
    .await
    .is_err());
    assert!(
        sqlx::query("DELETE FROM academic_promotion_runs WHERE id=$1")
            .bind(first.id)
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn promotion_run_creation_rejects_missing_scope_and_invalid_context() {
    let (pool, actor, input) = fixture("promotion_run_context").await;
    for grants in [
        vec![],
        vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL],
        vec![codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL],
        vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL,
            codes::ACADEMIC_PROMOTION_EXECUTE_SCHOOL,
        ],
    ] {
        let denied = ActorContext {
            user_id: actor.user_id,
            permissions: grants.into_iter().map(str::to_owned).collect(),
        };
        assert!(matches!(
            create_run(&pool, &denied, input.clone()).await,
            Err(AppError::Forbidden(_))
        ));
    }
    for field in 0..7 {
        let mut bad = input.clone();
        match field {
            0 => bad.source_year_id = bad.target_year_id,
            1 => bad.request_id = Uuid::nil(),
            2 => bad.source_year_id = Uuid::new_v4(),
            3 => bad.target_year_id = Uuid::new_v4(),
            4 => bad.policy_id = Uuid::new_v4(),
            5 => std::mem::swap(&mut bad.source_year_id, &mut bad.target_year_id),
            _ => bad.target_year_id = Uuid::nil(),
        }
        assert!(create_run(&pool, &actor, bad).await.is_err());
    }
    sqlx::query("UPDATE academic_years SET status='ready' WHERE id=$1")
        .bind(input.target_year_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        create_run(&pool, &actor, input).await,
        Err(AppError::Conflict(_))
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_promotion_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn promotion_run_creation_rolls_back_if_audit_fails_and_serializes_retries() {
    let (pool, actor, input) = fixture("promotion_run_atomic").await;
    sqlx::raw_sql("CREATE FUNCTION fail_promotion_run_audit() RETURNS trigger AS $$ BEGIN IF NEW.event_code='promotion_run.created' THEN RAISE EXCEPTION 'test audit failure'; END IF; RETURN NEW; END; $$ LANGUAGE plpgsql; CREATE TRIGGER test_promotion_run_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_promotion_run_audit();").execute(&pool).await.unwrap();
    assert!(matches!(
        create_run(&pool, &actor, input.clone()).await,
        Err(AppError::DbError(_))
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_promotion_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER test_promotion_run_audit ON academic_audit_events")
        .execute(&pool)
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        create_run(&pool, &actor, input.clone()),
        create_run(&pool, &actor, input)
    );
    assert_eq!(left.unwrap().id, right.unwrap().id);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_promotion_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
