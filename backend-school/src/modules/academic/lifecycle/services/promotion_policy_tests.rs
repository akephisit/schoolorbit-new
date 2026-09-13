use super::*;
use crate::{
    modules::academic::{
        core, cutover_test_support::apply_migrations_through, lifecycle::models::*,
    },
    permissions::registry::codes,
};
use uuid::Uuid;

async fn fixture(name: &str) -> (PgPool, ActorContext, PromotionPolicyInput) {
    let pool = core::services_tests::prepare_core_fixture(name).await;
    apply_migrations_through(&pool, 72).await.unwrap();
    let user = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (program, grade, next): (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT program.id,progression.from_grade_level_id,progression.to_grade_level_id FROM study_programs program CROSS JOIN grade_level_progressions progression WHERE progression.transition_kind='promote' AND progression.to_grade_level_id IS NOT NULL ORDER BY program.id,progression.id LIMIT 1"
    ).fetch_one(&pool).await.unwrap();
    sqlx::query("DELETE FROM grade_level_progressions")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO grade_level_progressions (from_grade_level_id,to_grade_level_id,transition_kind) VALUES ($1,$2,'promote')")
        .bind(grade).bind(next).execute(&pool).await.unwrap();
    (
        pool,
        ActorContext {
            user_id: user,
            permissions: vec![codes::WILDCARD.into()],
        },
        PromotionPolicyInput {
            name: "Reviewed policy".into(),
            rules: vec![PromotionRuleInput {
                from_grade_level_id: grade,
                from_study_program_id: program,
                target_grade_level_id: Some(next),
                target_study_program_id: Some(program),
                success_outcome: PromotionSuccessOutcome::Promote,
                minimum_earned_credits: "10.50".into(),
                require_no_exceptional_outcomes: true,
                require_activities_passed: true,
                minimum_learner_level: 1,
            }],
        },
    )
}

#[tokio::test]
async fn promotion_policy_storage_requires_separate_read_manage_and_approve_permissions() {
    let (pool, actor, input) = fixture("promotion_policy_permissions").await;
    for grants in [
        vec![],
        vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL],
        vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL,
            codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL,
        ],
        vec![
            codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL,
            codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL,
        ],
        vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL,
            codes::ACADEMIC_PROMOTION_EXECUTE_SCHOOL,
        ],
    ] {
        let other = ActorContext {
            user_id: actor.user_id,
            permissions: grants.iter().map(|code| (*code).into()).collect(),
        };
        assert!(matches!(
            create_promotion_policy(&pool, &other, input.clone()).await,
            Err(AppError::Forbidden(_))
        ));
    }
    let teacher = ActorContext {
        user_id: actor.user_id,
        permissions: vec![],
    };
    assert!(matches!(
        list_promotion_policies(&pool, &teacher).await,
        Err(AppError::Forbidden(_))
    ));
    let reader = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL.into()],
    };
    assert!(list_promotion_policies(&pool, &reader)
        .await
        .unwrap()
        .is_empty());
    let grant_leaks: i64 = sqlx::query_scalar("SELECT count(*) FROM role_permissions grant_row JOIN roles role ON role.id=grant_row.role_id JOIN permissions permission ON permission.id=grant_row.permission_id WHERE permission.module='academic_promotion' AND NOT (role.is_system AND role.code='ADMIN')").fetch_one(&pool).await.unwrap();
    assert_eq!(grant_leaks, 0);
    let authorized = ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL.into(),
        ],
    };
    assert!(create_promotion_policy(&pool, &authorized, input)
        .await
        .is_ok());
}

#[tokio::test]
async fn promotion_policy_storage_is_immutable_and_does_not_move_students() {
    let (pool, actor, input) = fixture("promotion_policy_history").await;
    let before: String = sqlx::query_scalar(
        "SELECT md5(string_agg(to_jsonb(s)::text,',' ORDER BY id)) FROM student_academic_years s",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let first = create_promotion_policy(&pool, &actor, input.clone())
        .await
        .unwrap();
    assert_eq!(first.name, "Reviewed policy");
    assert_eq!(first.rules[0].minimum_earned_credits, "10.50");
    assert_eq!(first.reviewed_by, actor.user_id);
    assert_eq!(first.progression_row_version, 1);
    sqlx::query("DELETE FROM grade_level_progressions")
        .execute(&pool)
        .await
        .unwrap();
    let listed = list_promotion_policies(&pool, &actor).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, first.id);
    assert_eq!(
        listed[0].rules[0].target_grade_level_id,
        input.rules[0].target_grade_level_id
    );
    assert!(
        sqlx::query("UPDATE academic_promotion_policy_versions SET name='changed'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM academic_promotion_policy_versions")
            .execute(&pool)
            .await
            .is_err()
    );
    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='promotion_policy.approved'").bind(first.id).fetch_one(&pool).await.unwrap();
    assert_eq!(audit, 1);
    let after: String = sqlx::query_scalar(
        "SELECT md5(string_agg(to_jsonb(s)::text,',' ORDER BY id)) FROM student_academic_years s",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn promotion_policy_storage_rejects_missing_or_ambiguous_core_mappings() {
    let (pool, actor, input) = fixture("promotion_policy_mapping").await;
    for field in 0..4 {
        let mut invalid = input.clone();
        match field {
            0 => invalid.rules[0].from_grade_level_id = Uuid::new_v4(),
            1 => invalid.rules[0].from_study_program_id = Uuid::new_v4(),
            2 => invalid.rules[0].target_grade_level_id = Some(Uuid::new_v4()),
            _ => invalid.rules[0].target_study_program_id = Some(Uuid::new_v4()),
        }
        assert!(matches!(
            create_promotion_policy(&pool, &actor, invalid).await,
            Err(AppError::ValidationError(_))
        ));
    }
    sqlx::query("INSERT INTO grade_level_progressions (from_grade_level_id,to_grade_level_id,transition_kind) SELECT from_grade_level_id,to_grade_level_id,transition_kind FROM grade_level_progressions LIMIT 1").execute(&pool).await.unwrap();
    assert!(matches!(
        create_promotion_policy(&pool, &actor, input.clone()).await,
        Err(AppError::ValidationError(_))
    ));
    sqlx::query("DELETE FROM grade_level_progressions")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        create_promotion_policy(&pool, &actor, input.clone()).await,
        Err(AppError::ValidationError(_))
    ));
    let mut graduate = input;
    graduate.rules[0].success_outcome = PromotionSuccessOutcome::Graduate;
    graduate.rules[0].target_grade_level_id = None;
    graduate.rules[0].target_study_program_id = None;
    assert!(matches!(
        create_promotion_policy(&pool, &actor, graduate.clone()).await,
        Err(AppError::ValidationError(_))
    ));
    sqlx::query("INSERT INTO grade_level_progressions (from_grade_level_id,transition_kind,curriculum_id) SELECT $1,'graduate',version.curriculum_id FROM study_programs program JOIN curriculum_versions version ON version.id=program.curriculum_version_id WHERE program.id=$2").bind(graduate.rules[0].from_grade_level_id).bind(graduate.rules[0].from_study_program_id).execute(&pool).await.unwrap();
    assert!(create_promotion_policy(&pool, &actor, graduate)
        .await
        .is_ok());
}

#[tokio::test]
async fn promotion_policy_storage_audit_failure_rolls_back_approval() {
    let (pool, actor, input) = fixture("promotion_policy_atomic").await;
    sqlx::raw_sql("CREATE FUNCTION fail_policy_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'POLICY_TEST_AUDIT_FAILURE'; END $$; CREATE TRIGGER fail_policy_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_policy_audit();").execute(&pool).await.unwrap();
    assert!(matches!(
        create_promotion_policy(&pool, &actor, input).await,
        Err(AppError::DbError(_))
    ));
    assert!(list_promotion_policies(&pool, &actor)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn promotion_policy_storage_does_not_borrow_another_curriculums_progression() {
    let (pool, actor, input) = fixture("promotion_policy_curriculum_scope").await;
    let foreign_curriculum = Uuid::new_v4();
    // A separate draft curriculum is a real FK target, not a dangling synthetic ID.
    sqlx::query("INSERT INTO curricula (id,code,identity_key,name_th,grade_level_ids) VALUES ($1,'E2E-PROMOTION-FOREIGN','e2e-promotion-foreign','Foreign curriculum','[]'::jsonb)")
        .bind(foreign_curriculum).execute(&pool).await.unwrap();
    sqlx::query("UPDATE grade_level_progressions SET curriculum_id=$1")
        .bind(foreign_curriculum)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        create_promotion_policy(&pool, &actor, input.clone()).await,
        Err(AppError::ValidationError(_))
    ));
    sqlx::query("UPDATE grade_level_progressions SET curriculum_id=NULL,is_active=false")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        create_promotion_policy(&pool, &actor, input.clone()).await,
        Err(AppError::ValidationError(_))
    ));
    sqlx::query("UPDATE grade_level_progressions SET is_active=true,transition_kind='repeat'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        create_promotion_policy(&pool, &actor, input).await,
        Err(AppError::ValidationError(_))
    ));
}

#[tokio::test]
async fn promotion_policy_storage_waits_for_a_committing_progression_revision() {
    let (pool, actor, input) = fixture("promotion_policy_revision_lock").await;
    let mut editor = pool.begin().await.unwrap();
    let editor_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *editor)
        .await
        .unwrap();
    sqlx::query("UPDATE grade_level_progression_sets SET row_version=row_version+1 WHERE id=1")
        .execute(&mut *editor)
        .await
        .unwrap();
    let worker_pool = pool.clone();
    let worker =
        tokio::spawn(async move { create_promotion_policy(&worker_pool, &actor, input).await });
    let mut blocked = false;
    for _ in 0..200 {
        blocked = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
        )
        .bind(editor_pid)
        .fetch_one(&pool)
        .await
        .unwrap();
        if blocked {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    // Release the editor even on a regression so the worker cannot hang the suite.
    editor.commit().await.unwrap();
    let result = worker.await.unwrap().unwrap();
    assert!(
        blocked,
        "policy approval must wait for the authoritative progression-set lock"
    );
    assert_eq!(result.progression_row_version, 2);
}

#[tokio::test]
async fn promotion_policy_storage_readers_can_resolve_names_without_curriculum_manage_grants() {
    let (pool, actor, input) = fixture("promotion_policy_reference_scope").await;
    let reader = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL.into()],
    };
    let options = get_promotion_policy_options(&pool, &reader).await.unwrap();
    assert!(options
        .grades
        .iter()
        .any(|grade| grade.id == input.rules[0].from_grade_level_id));
    let program = options
        .programs
        .iter()
        .find(|program| program.id == input.rules[0].from_study_program_id)
        .unwrap();
    assert!(!program.name.is_empty() && !program.curriculum_name.is_empty());
    assert_eq!(options.progression_set.row_version, 1);
    assert_eq!(options.progression_set.progressions.len(), 1);
    let teacher = ActorContext {
        user_id: actor.user_id,
        permissions: vec![],
    };
    assert!(matches!(
        get_promotion_policy_options(&pool, &teacher).await,
        Err(AppError::Forbidden(_))
    ));
}
