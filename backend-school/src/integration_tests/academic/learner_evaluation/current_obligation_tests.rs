use super::*;
use chrono::NaiveDate;
use school_academic_assessment::{evidence, ports::CURRENT_OBLIGATION_GROUP_SCAN_LIMIT};
use serde_json::Value;

async fn scoped_fixture(
    name: &str,
) -> (
    sqlx::PgPool,
    ActorContext,
    EvaluationContext,
    Uuid,
    Uuid,
    Uuid,
) {
    let (pool, actor, ctx, group, subject) = fixture(name).await;
    let unused = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_groups SELECT (jsonb_populate_record(NULL::learning_groups,to_jsonb(g)||jsonb_build_object('id',$2::uuid,'code',$2::text,'name','Current empty learner group'))).* FROM learning_groups g WHERE id=$1")
        .bind(group).bind(unused).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers SELECT (jsonb_populate_record(NULL::learning_group_teachers,to_jsonb(t)||jsonb_build_object('id',uuidv7(),'learning_group_id',$2::uuid))).* FROM learning_group_teachers t WHERE learning_group_id=$1")
        .bind(group).bind(unused).execute(&pool).await.unwrap();
    (pool, actor, ctx, group, subject, unused)
}
async fn facts(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('criteria',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM subject_term_evaluation_criteria t),'responses',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_evaluations t),'confirmations',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_evaluation_confirmations t),'memberships',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_students t),'scores',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_scores t),'configurations',(SELECT jsonb_agg(to_jsonb(t) ORDER BY subject_id,academic_term_id,domain) FROM subject_term_evaluation_configurations t))")
    .fetch_one(pool).await.unwrap()
}
#[tokio::test]
async fn learner_current_empty_group_remains_an_obligation_without_publication_or_history() {
    let (pool, actor, ctx, group, subject, empty) =
        scoped_fixture("learner_current_empty_required").await;
    let before = facts(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    for day in [
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2030, 1, 1).unwrap(),
    ] {
        let mut expected = vec![group, empty];
        expected.sort();
        assert_eq!(
            school_academic_results::services::current_obligation_group_ids_on_connection(
                &mut conn,
                ctx.academic_term_id,
                ctx.academic_year_id,
                day,
                &[empty, group, empty]
            )
            .await
            .unwrap(),
            expected
        );
    }
    assert!(
        evidence::groups_with_established_history_on_connection(
            &mut conn,
            ctx.academic_term_id,
            ctx.academic_year_id,
            &[empty]
        )
        .await
        .unwrap()
        .is_empty()
    );
    drop(conn);
    let queue = read_lock_readiness(
        &pool,
        &crate::adapters::academic_result_locks::RESULT_LOCKS,
        &actor,
        &ctx,
    )
    .await
    .unwrap();
    let row = queue
        .iter()
        .find(|r| r.subject_id == subject && r.domain == DC)
        .unwrap();
    assert!(
        row.groups
            .iter()
            .any(|r| r.learning_group_id == empty && !r.ready)
    );
    assert_eq!(
        before,
        facts(&pool).await,
        "readiness cannot lazily create or rewrite evaluation facts"
    );
}
#[tokio::test]
async fn learner_current_teacher_change_preserves_responses_and_revokes_previous_teacher() {
    use school_academic_delivery::{models as dm, services::groups};
    let (pool, actor, ctx, group, _) = fixture("learner_current_teacher_revoke").await;
    let filled = fill(&pool, &actor, &ctx, group, DC, 2).await;
    confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&filled))
        .await
        .unwrap();
    let responses:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_group_student_evaluations r WHERE learning_group_id=$1").bind(group).fetch_one(&pool).await.unwrap();
    let replacement:Uuid=sqlx::query_scalar("INSERT INTO users(username,password_hash,first_name,last_name,user_type,status) VALUES('learner-current-'||uuidv7()::text,'not-login','Current','Teacher','staff','active') RETURNING id").fetch_one(&pool).await.unwrap();
    let current = groups::get(&pool, group).await.unwrap();
    groups::replace_teachers(
        &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
        &pool,
        actor.user_id,
        group,
        dm::ReplaceLearningGroupTeachersRequest {
            row_version: current.row_version,
            teachers: vec![dm::TeacherAssignmentInput {
                teacher_id: replacement,
                role: dm::LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let mut previous = actor.clone();
    previous.permissions = vec![codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED.into()];
    assert!(matches!(
        get_workspace(&pool, &previous, group, DC, &ctx).await,
        Err(AppError::Forbidden(_))
    ));
    let changed = get_workspace(&pool, &actor, group, DC, &ctx).await.unwrap();
    assert!(!changed.confirmation_is_current);
    assert_eq!(responses,sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_group_student_evaluations r WHERE learning_group_id=$1").bind(group).fetch_one(&pool).await.unwrap());
}
#[tokio::test]
async fn learner_current_obligation_leaf_refuses_unknown_context_and_hard_budget() {
    let (pool, _, ctx, group, _, _) = scoped_fixture("learner_current_obligation_context").await;
    let mut connection = pool.acquire().await.unwrap();
    assert!(matches!(
        evidence::groups_with_established_history_on_connection(
            &mut connection,
            ctx.academic_term_id,
            Uuid::new_v4(),
            &[group]
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        evidence::groups_with_established_history_on_connection(
            &mut connection,
            ctx.academic_term_id,
            ctx.academic_year_id,
            &[Uuid::new_v4()]
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let too_many = (0..=CURRENT_OBLIGATION_GROUP_SCAN_LIMIT)
        .map(|_| Uuid::new_v4())
        .collect::<Vec<_>>();
    assert!(matches!(
        evidence::groups_with_established_history_on_connection(
            &mut connection,
            ctx.academic_term_id,
            ctx.academic_year_id,
            &too_many
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
}

#[tokio::test]
async fn learner_current_obligation_zero_result_and_unknown_lock_snapshots_retain_context() {
    let (pool, actor, ctx, _, subject, unused) =
        scoped_fixture("learner_current_obligation_lock_history").await;
    for (domain, snapshot) in [
        (DC, serde_json::json!({"groups":[]})),
        (RTW, serde_json::json!({"historicalShape":"unknown"})),
    ] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO subject_term_evaluation_locks(subject_id,academic_term_id,academic_year_id,domain,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES($1,$2,$3,$4,$5,$5,$6,$7)")
            .bind(subject).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(domain.as_str())
            .bind("a".repeat(64)).bind(sqlx::types::Json(&snapshot)).bind(actor.user_id).execute(&mut *tx).await.unwrap();
        let evidence = evidence::groups_with_established_history_on_connection(
            &mut tx,
            ctx.academic_term_id,
            ctx.academic_year_id,
            &[unused],
        )
        .await
        .unwrap();
        assert!(
            evidence.contains(&unused),
            "a subject/domain lock conservatively protects its context even without normalized evaluations or readable group references"
        );
        let finals: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM subject_term_student_evaluations WHERE learning_group_id=$1",
        )
        .bind(unused)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(finals, 0);
        tx.rollback().await.unwrap();
    }
}
