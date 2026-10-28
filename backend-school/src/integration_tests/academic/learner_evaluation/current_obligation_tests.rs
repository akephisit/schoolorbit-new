use super::*;
use chrono::{Duration, NaiveDate};
use school_academic_assessment::{
    evidence,
    ports::{ResultLockPort, CURRENT_OBLIGATION_GROUP_SCAN_LIMIT},
};
use school_academic_delivery::{
    models as dm,
    services::{change_sets, versions},
};
use serde_json::Value;

fn result_locks() -> &'static dyn ResultLockPort {
    &crate::adapters::academic_result_locks::RESULT_LOCKS
}

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
    let pool = create_named_test_pool_with_max_connections(name, 4).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 61).await.unwrap();
    let (year,term,group,subject,teacher):(Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT g.academic_year_id,g.academic_term_id,g.id,d.subject_id,t.teacher_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id WHERE t.role='primary' AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1").fetch_one(&pool).await.unwrap();
    let unused = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status,created_at,updated_at) SELECT $1,learning_offering_id,academic_term_id,academic_year_id,$1::text,'Unused learner room','draft','draft',created_at,updated_at FROM learning_groups WHERE id=$2")
        .bind(unused).bind(group).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,created_at) SELECT gen_random_uuid(),$1,academic_term_id,academic_year_id,teacher_id,role,starts_on,created_at FROM learning_group_teachers WHERE learning_group_id=$2 AND role='primary' ORDER BY id LIMIT 1")
        .bind(unused).bind(group).execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 144).await.unwrap();
    let today: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2) WHERE id=$1")
        .bind(year)
        .bind(today + Duration::days(60))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE academic_terms SET planned_end_date=GREATEST(planned_end_date,$2) WHERE id=$1",
    )
    .bind(term)
    .bind(today + Duration::days(30))
    .execute(&pool)
    .await
    .unwrap();
    let actor = ActorContext {
        user_id: teacher,
        permissions: vec![
            codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL.into(),
        ],
    };
    (
        pool,
        actor,
        EvaluationContext {
            academic_year_id: year,
            academic_term_id: term,
        },
        group,
        subject,
        unused,
    )
}

async fn omit(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
    groups: &[Uuid],
    future: bool,
) {
    let mut revision = change_sets::create_change_set(
        pool,
        actor.user_id,
        dm::CreateAcademicTermChangeSetRequest {
            academic_term_id: ctx.academic_term_id,
            reason: "Review unused learner groups".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    if future {
        revision = change_sets::update_change_set(
            pool,
            actor.user_id,
            revision.id,
            dm::UpdateAcademicTermChangeSetRequest {
                row_version: revision.row_version,
                reference_date: revision.reference_date + Duration::days(1),
                reason: revision.reason.clone(),
            },
        )
        .await
        .unwrap();
    }
    for group in groups {
        let target = versions::get_version(pool, revision.target_delivery_version_id)
            .await
            .unwrap();
        revision = change_sets::remove_draft_group(
            pool,
            actor.user_id,
            revision.id,
            *group,
            dm::RemoveDeliveryDraftResourceRequest {
                change_set_row_version: revision.row_version,
                target_delivery_version_row_version: target.row_version,
            },
        )
        .await
        .unwrap();
    }
    let preview =
        change_sets::preview_change_set_at(pool, revision.id, Some(revision.reference_date))
            .await
            .unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|f| f.severity == dm::AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        pool,
        actor.user_id,
        revision.id,
        dm::PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: preview
                .findings
                .iter()
                .filter(|f| f.severity == dm::AcademicChangeFindingSeverity::Warning)
                .map(|f| f.code)
                .collect(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
}

async fn facts(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('criteria',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM subject_term_evaluation_criteria t),'responses',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_evaluations t),'confirmations',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_evaluation_confirmations t),'memberships',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_students t),'scores',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_scores t),'configurations',(SELECT jsonb_agg(to_jsonb(t) ORDER BY subject_id,academic_term_id,domain) FROM subject_term_evaluation_configurations t))")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn learner_current_obligation_ready_retained_rooms_lock_without_unused_omitted_room() {
    let (pool, actor, ctx, group, subject, unused) =
        scoped_fixture("learner_current_obligation_ready").await;
    let rooms:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT g.id,t.teacher_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id AND t.role='primary' WHERE d.subject_id=$1 AND g.academic_term_id=$2 AND g.status<>'closed' AND g.id<>$3 ORDER BY g.id")
        .bind(subject).bind(ctx.academic_term_id).bind(unused).fetch_all(&pool).await.unwrap();
    for (room, teacher) in rooms {
        let teacher = ActorContext {
            user_id: teacher,
            permissions: actor.permissions.clone(),
        };
        let ws = fill(&pool, &teacher, &ctx, room, DC, 3).await;
        confirm_group(&pool, &teacher, room, DC, &ctx, confirmation(&ws))
            .await
            .unwrap();
    }
    let baseline = read_lock_readiness(&pool, result_locks(), &actor, &ctx)
        .await
        .unwrap();
    assert!(
        !baseline
            .iter()
            .find(|r| r.subject_id == subject && r.domain == DC)
            .unwrap()
            .ready
    );
    omit(&pool, &actor, &ctx, &[unused], false).await;
    let before = facts(&pool).await;
    let ready = read_lock_readiness(&pool, result_locks(), &actor, &ctx)
        .await
        .unwrap();
    let ready = ready
        .iter()
        .find(|r| r.subject_id == subject && r.domain == DC)
        .unwrap();
    assert!(ready.ready);
    assert!(ready.groups.iter().all(|g| g.learning_group_id != unused));
    assert!(ready.groups.iter().any(|g| g.learning_group_id == group));
    assert_eq!(before, facts(&pool).await, "readiness must be read-only");
    let locked = lock_subject(&pool, result_locks(), &actor, subject, DC, &ctx)
        .await
        .unwrap();
    assert!(locked.blockers.is_empty());
    assert!(locked.lock.is_some());
    assert_eq!(
        before,
        facts(&pool).await,
        "locking keeps criteria and established source facts exact"
    );
}

#[tokio::test]
async fn learner_current_obligation_future_scope_and_invalidated_confirmation_remain_requirements()
{
    let (pool, actor, ctx, _, subject, unused) =
        scoped_fixture("learner_current_obligation_history").await;
    let before = facts(&pool).await;
    omit(&pool, &actor, &ctx, &[unused], true).await;
    let rows = read_lock_readiness(&pool, result_locks(), &actor, &ctx)
        .await
        .unwrap();
    assert!(rows
        .iter()
        .find(|r| r.subject_id == subject && r.domain == DC)
        .unwrap()
        .groups
        .iter()
        .any(|g| g.learning_group_id == unused));
    assert_eq!(before, facts(&pool).await);
    let empty = get_workspace(&pool, &actor, unused, DC, &ctx)
        .await
        .unwrap();
    assert!(empty.students.is_empty());
    confirm_group(&pool, &actor, unused, DC, &ctx, confirmation(&empty))
        .await
        .unwrap();
    sqlx::query("UPDATE learning_group_evaluation_confirmations SET source_snapshot=jsonb_set(source_snapshot,'{invalidated}','true') WHERE learning_group_id=$1")
        .bind(unused).execute(&pool).await.unwrap();
    let before = facts(&pool).await;
    let mut connection = pool.acquire().await.unwrap();
    let evidence = evidence::groups_with_established_history_on_connection(
        &mut connection,
        ctx.academic_term_id,
        ctx.academic_year_id,
        &[unused],
    )
    .await
    .unwrap();
    assert!(evidence.contains(&unused));
    drop(connection);
    assert_eq!(
        before,
        facts(&pool).await,
        "invalidated evidence stays an established obligation"
    );
}

#[tokio::test]
async fn learner_current_obligation_all_omitted_refuses_before_lazy_configuration_and_preserves_browse(
) {
    let (pool, actor, ctx, _, _, unused) =
        scoped_fixture("learner_current_obligation_no_lazy").await;
    // Select the genuinely unused group under a separate subject/offering whose
    // history-free setup is captured through a normal Delivery revision.
    let offering: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id=$1")
            .bind(unused)
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject: Uuid = sqlx::query_scalar(
        "SELECT subject_id FROM course_offering_details WHERE learning_offering_id=$1",
    )
    .bind(offering)
    .fetch_one(&pool)
    .await
    .unwrap();
    // Every ordinary sibling already has membership evidence. Use the classifier
    // port contract to exercise the all-omitted lock path without altering history.
    struct NoCurrent;
    #[async_trait::async_trait]
    impl ResultLockPort for NoCurrent {
        async fn require_course_offering_unlocked(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _offering: Uuid,
            _term: Uuid,
            _year: Uuid,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn current_obligation_group_ids(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _term: Uuid,
            _year: Uuid,
            _date: NaiveDate,
            _groups: &[Uuid],
        ) -> Result<Vec<Uuid>, AppError> {
            Ok(vec![])
        }
    }
    let before = facts(&pool).await;
    assert!(matches!(
        lock_subject(&pool, &NoCurrent, &actor, subject, RTW, &ctx).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(before, facts(&pool).await);
    // General browsing keeps its original lazy initialization and scope behavior.
    assert!(get_workspace(&pool, &actor, unused, RTW, &ctx)
        .await
        .is_ok());
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
        assert!(evidence.contains(&unused),"a subject/domain lock conservatively protects its context even without normalized evaluations or readable group references");
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
