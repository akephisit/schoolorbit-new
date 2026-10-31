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
    scoped_fixture_with_catalog(name, false).await.0
}

// Only the real empty-subject regression needs a distinct historical subject.
// Seed after Phase B reconciliation, before 052 creates canonical legacy targets;
// 087 then captures the graph normally. Existing runtime fixtures stay unchanged.
async fn seed_unused_subject_predecessor(pool: &sqlx::PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    use school_test_db::cutover_test_support::{
        record_passing_phase_a_reconciliation_marker, seed_academic_cutover_fixture, CutoverFixture,
    };
    apply_migrations_through(pool, 40).await.unwrap();
    seed_academic_cutover_fixture(pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(pool, 44).await.unwrap();
    record_passing_phase_a_reconciliation_marker(pool)
        .await
        .unwrap();
    apply_migrations_through(pool, 51).await.unwrap();
    let base:Uuid=sqlx::query_scalar("SELECT g.id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1")
        .fetch_one(pool).await.unwrap();
    let subject = Uuid::new_v4();
    let version = Uuid::new_v4();
    let offering = Uuid::new_v4();
    let group = Uuid::new_v4();
    let code = format!("LU-{:016x}", subject.as_u128() as u64);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO subjects SELECT (jsonb_populate_record(NULL::subjects,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'code',$2::text,'identity_key',lower($2::text)))).* FROM subjects source JOIN course_offering_details d ON d.subject_id=source.id JOIN learning_groups g ON g.learning_offering_id=d.learning_offering_id WHERE g.id=$3")
        .bind(subject).bind(&code).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO subject_versions SELECT (jsonb_populate_record(NULL::subject_versions,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'subject_id',$2::uuid,'code',$3::text,'version_no',1,'effective_until',NULL,'migration_provenance','{}'::jsonb))).* FROM subject_versions source JOIN course_offering_details d ON d.subject_version_id=source.id JOIN learning_groups g ON g.learning_offering_id=d.learning_offering_id WHERE g.id=$4")
        .bind(version).bind(subject).bind(&code).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO subject_version_grade_levels(subject_id,grade_level_id,created_at) SELECT $1,link.grade_level_id,link.created_at FROM subject_version_grade_levels link JOIN course_offering_details d ON d.subject_version_id=link.subject_id JOIN learning_groups g ON g.learning_offering_id=d.learning_offering_id WHERE g.id=$2")
        .bind(version).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'code_snapshot',$2::text,'name_snapshot','Unused historical learner subject','source_requirement_kind',NULL,'source_requirement_id',NULL,'status','draft','published_at',NULL,'publish_idempotency_key',NULL,'migration_provenance','{}'::jsonb))).* FROM learning_offerings source JOIN learning_groups g ON g.learning_offering_id=source.id WHERE g.id=$3")
        .bind(offering).bind(&code).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details SELECT (jsonb_populate_record(NULL::course_offering_details,to_jsonb(source)||jsonb_build_object('learning_offering_id',$1::uuid,'subject_version_id',$2::uuid,'subject_id',$3::uuid,'curriculum_course_requirement_id',NULL,'migration_provenance','{}'::jsonb))).* FROM course_offering_details source JOIN learning_groups g ON g.learning_offering_id=source.learning_offering_id WHERE g.id=$4")
        .bind(offering).bind(version).bind(subject).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_offering_targets SELECT (jsonb_populate_record(NULL::learning_offering_targets,to_jsonb(source)||jsonb_build_object('id',gen_random_uuid(),'learning_offering_id',$1::uuid,'migration_provenance','{}'::jsonb))).* FROM learning_offering_targets source JOIN learning_groups g ON g.learning_offering_id=source.learning_offering_id WHERE g.id=$2")
        .bind(offering).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_groups SELECT (jsonb_populate_record(NULL::learning_groups,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'learning_offering_id',$2::uuid,'code',$3::text,'name','Unused historical learner group','status','draft','roster_status','draft','roster_published_at',NULL,'roster_source_hash',NULL,'roster_publish_idempotency_key',NULL,'migration_provenance','{}'::jsonb))).* FROM learning_groups source WHERE source.id=$4")
        .bind(group).bind(offering).bind(&code).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_group_homerooms SELECT (jsonb_populate_record(NULL::learning_group_homerooms,to_jsonb(source)||jsonb_build_object('id',gen_random_uuid(),'learning_group_id',$1::uuid,'migration_provenance','{}'::jsonb))).* FROM learning_group_homerooms source WHERE source.learning_group_id=$2")
        .bind(group).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers SELECT (jsonb_populate_record(NULL::learning_group_teachers,to_jsonb(source)||jsonb_build_object('id',gen_random_uuid(),'learning_group_id',$1::uuid,'migration_provenance','{}'::jsonb))).* FROM learning_group_teachers source WHERE source.learning_group_id=$2 AND source.role='primary' ORDER BY source.id LIMIT 1")
        .bind(group).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE learning_groups SET status='published' WHERE id=$1")
        .bind(group)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE learning_offerings SET status='published',published_at=created_at WHERE id=$1",
    )
    .bind(offering)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    (subject, version, offering, group)
}

async fn scoped_fixture_with_catalog(
    name: &str,
    seed_catalog: bool,
) -> (
    (
        sqlx::PgPool,
        ActorContext,
        EvaluationContext,
        Uuid,
        Uuid,
        Uuid,
    ),
    Option<(Uuid, Uuid, Uuid, Uuid)>,
) {
    let pool = create_named_test_pool_with_max_connections(name, 4).await;
    let catalog = if seed_catalog {
        Some(seed_unused_subject_predecessor(&pool).await)
    } else {
        seed_release_two_predecessor(&pool).await.unwrap();
        None
    };
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
        ),
        catalog,
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
        &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
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
    let mut populated_original_seen = false;
    for (room, teacher) in rooms {
        let teacher = ActorContext {
            user_id: teacher,
            permissions: actor.permissions.clone(),
        };
        let workspace = get_workspace(&pool, &teacher, room, DC, &ctx)
            .await
            .unwrap();
        let active_criteria = workspace
            .criteria
            .iter()
            .filter(|criterion| criterion.lifecycle == "active")
            .count();
        eprintln!("learner ready fixture room={room} active_students={} active_criteria={active_criteria}", workspace.students.len());
        assert!(
            active_criteria > 0,
            "retained room {room} must have active criteria, not a vacuously ready fixture"
        );
        if room == group {
            assert!(
                !workspace.students.is_empty(),
                "the original retained learner group must keep its populated roster"
            );
            populated_original_seen = true;
        }
        let ws = if workspace.students.is_empty() {
            // An included empty room still requires its ordinary confirmation;
            // saving an empty response batch is rejected by the entry service.
            workspace
        } else {
            assert!(
                workspace.students.len() * active_criteria <= 500,
                "retained room {room} exceeds the response fixture's supported batch size"
            );
            fill(&pool, &teacher, &ctx, room, DC, 3).await
        };
        confirm_group(&pool, &teacher, room, DC, &ctx, confirmation(&ws))
            .await
            .unwrap();
    }
    assert!(
        populated_original_seen,
        "the original populated retained group must participate in readiness and locking"
    );
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
    let baseline_subject = baseline
        .iter()
        .find(|row| row.subject_id == subject && row.domain == DC)
        .unwrap();
    assert!(
        baseline_subject
            .groups
            .iter()
            .filter(|room| room.learning_group_id != unused)
            .all(|room| room.ready),
        "all retained rooms must genuinely be ready before the scope removal"
    );
    assert!(
        !baseline_subject
            .groups
            .iter()
            .find(|room| room.learning_group_id == unused)
            .unwrap()
            .ready,
        "the unused group must be the original readiness blocker"
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
        async fn course_offering_is_locked(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _offering: Uuid,
            _term: Uuid,
            _year: Uuid,
        ) -> Result<bool, AppError> {
            Ok(false)
        }

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

#[tokio::test]
async fn learner_current_obligation_real_all_omitted_subject_refuses_without_lazy_writes() {
    let ((pool, actor, ctx, _, _, _), catalog) =
        scoped_fixture_with_catalog("learner_current_obligation_real_empty", true).await;
    let (subject, _, offering, group) = catalog.unwrap();
    let source:Uuid=sqlx::query_scalar("SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1")
        .bind(ctx.academic_term_id).fetch_one(&pool).await.unwrap();
    let published = versions::get_version(&pool, source).await.unwrap();
    assert!(published.snapshot.offerings.iter().any(|entry|entry.id==offering && entry.groups.iter().any(|entry|entry.id==group)),
        "normal052/087 migrations must establish this unused group in the published baseline before removal");
    let published_bytes: Value =
        sqlx::query_scalar("SELECT to_jsonb(v) FROM academic_delivery_versions v WHERE id=$1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    let group_before: Value =
        sqlx::query_scalar("SELECT to_jsonb(g) FROM learning_groups g WHERE id=$1")
            .bind(group)
            .fetch_one(&pool)
            .await
            .unwrap();
    let revision = change_sets::create_change_set(
        &pool,
        actor.user_id,
        dm::CreateAcademicTermChangeSetRequest {
            academic_term_id: ctx.academic_term_id,
            reason: "Omit every room of the unused learner subject".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let target = versions::get_version(&pool, revision.target_delivery_version_id)
        .await
        .unwrap();
    // The predecessor can target more rooms than this group's coverage. Omit the
    // complete offering deliberately; group removal preserves uncovered rooms.
    change_sets::remove_draft_offering(
        &pool,
        actor.user_id,
        revision.id,
        offering,
        dm::RemoveDeliveryDraftResourceRequest {
            change_set_row_version: revision.row_version,
            target_delivery_version_row_version: target.row_version,
        },
    )
    .await
    .unwrap();
    let preview =
        change_sets::preview_change_set_at(&pool, revision.id, Some(revision.reference_date))
            .await
            .unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|finding| finding.severity == dm::AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
        &pool,
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
                .filter(|finding| finding.severity == dm::AcademicChangeFindingSeverity::Warning)
                .map(|finding| finding.code)
                .collect(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let group_after: Value =
        sqlx::query_scalar("SELECT to_jsonb(g) FROM learning_groups g WHERE id=$1")
            .bind(group)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        group_before, group_after,
        "whole-offering omission preserves the unused registry group"
    );
    let published_after: Value =
        sqlx::query_scalar("SELECT to_jsonb(v) FROM academic_delivery_versions v WHERE id=$1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        published_bytes, published_after,
        "current omission must keep its prior published baseline exact"
    );
    let mut connection = pool.acquire().await.unwrap();
    let day:NaiveDate=sqlx::query_scalar("SELECT LEAST(GREATEST((now() AT TIME ZONE 'Asia/Bangkok')::date,t.start_date),COALESCE(t.closed_on,t.planned_end_date,y.end_date)) FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1")
        .bind(ctx.academic_term_id).fetch_one(&mut *connection).await.unwrap();
    let scope = school_academic_delivery::services::audiences::dated_constraint_on_connection(
        &mut connection,
        offering,
        day,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        scope.offering.is_none(),
        "selected empty scope must exist, not legacy None"
    );
    assert!(
        school_academic_results::services::current_obligation_group_ids_on_connection(
            &mut connection,
            ctx.academic_term_id,
            ctx.academic_year_id,
            day,
            &[group]
        )
        .await
        .unwrap()
        .is_empty()
    );
    drop(connection);
    let term = ctx.academic_term_id;
    let count = |pool: sqlx::PgPool| async move {
        sqlx::query_as::<_,(i64,i64)>("SELECT (SELECT count(*) FROM subject_term_evaluation_configurations WHERE subject_id=$1 AND academic_term_id=$2),(SELECT count(*) FROM subject_term_evaluation_locks WHERE subject_id=$1 AND academic_term_id=$2)")
            .bind(subject).bind(term).fetch_one(&pool).await.unwrap()
    };
    assert_eq!(count(pool.clone()).await, (0, 0));
    let before = facts(&pool).await;
    let queue = read_lock_readiness(&pool, result_locks(), &actor, &ctx)
        .await
        .unwrap();
    assert!(queue.iter().all(|row| row.subject_id != subject));
    assert!(matches!(
        lock_subject(&pool, result_locks(), &actor, subject, DC, &ctx).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(count(pool.clone()).await, (0, 0));
    assert_eq!(
        before,
        facts(&pool).await,
        "real omitted-only subject cannot lazily initialize or rewrite other learner facts"
    );
}

#[tokio::test]
async fn learner_current_obligation_current_omitted_invalidated_confirmation_is_retained() {
    let (pool, actor, ctx, _, subject, unused) =
        scoped_fixture("learner_current_obligation_current_invalidated").await;
    omit(&pool, &actor, &ctx, &[unused], false).await;
    let mut connection = pool.acquire().await.unwrap();
    let day: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert!(
        school_academic_results::services::current_obligation_group_ids_on_connection(
            &mut connection,
            ctx.academic_term_id,
            ctx.academic_year_id,
            day,
            &[unused]
        )
        .await
        .unwrap()
        .is_empty()
    );
    drop(connection);
    let empty = get_workspace(&pool, &actor, unused, DC, &ctx)
        .await
        .unwrap();
    assert!(empty.students.is_empty());
    confirm_group(&pool, &actor, unused, DC, &ctx, confirmation(&empty))
        .await
        .unwrap();
    sqlx::query("UPDATE learning_group_evaluation_confirmations SET source_snapshot=jsonb_set(source_snapshot,'{invalidated}','true') WHERE learning_group_id=$1")
        .bind(unused).execute(&pool).await.unwrap();
    let no_locks:i64=sqlx::query_scalar("SELECT count(*) FROM subject_term_evaluation_locks WHERE subject_id=$1 AND academic_term_id=$2")
        .bind(subject).bind(ctx.academic_term_id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        no_locks, 0,
        "confirmation evidence must not be masked by conservative subject locks"
    );
    let before = facts(&pool).await;
    let mut connection = pool.acquire().await.unwrap();
    let retained = school_academic_results::services::current_obligation_group_ids_on_connection(
        &mut connection,
        ctx.academic_term_id,
        ctx.academic_year_id,
        day,
        &[unused],
    )
    .await
    .unwrap();
    assert_eq!(retained, vec![unused]);
    drop(connection);
    let queue = read_lock_readiness(&pool, result_locks(), &actor, &ctx)
        .await
        .unwrap();
    let room = queue
        .iter()
        .find(|row| row.subject_id == subject && row.domain == DC)
        .unwrap()
        .groups
        .iter()
        .find(|group| group.learning_group_id == unused)
        .unwrap();
    assert!(!room.ready);
    assert!(room
        .blockers
        .iter()
        .any(|reason| reason == "stale_group_confirmation"));
    assert_eq!(before,facts(&pool).await,"invalidated confirmation remains byte-exact through current classification and readonly learner queue");
}

#[tokio::test]
async fn learner_current_obligation_phase_and_response_evidence_are_independent_and_coupled_facts_preserved(
) {
    let (pool, actor, ctx, base, subject, unused) =
        scoped_fixture("learner_current_obligation_leaf_branches").await;
    omit(&pool, &actor, &ctx, &[unused], false).await;
    let configuration = get_configuration(&pool, &actor, subject, DC, &ctx)
        .await
        .unwrap();
    let criterion = configuration
        .criteria
        .iter()
        .find(|criterion| criterion.lifecycle == "active")
        .unwrap()
        .id;
    let (offering,plan,phase,phase_version):(Uuid,Uuid,Uuid,i64)=sqlx::query_as("SELECT g.learning_offering_id,p.id,phase.id,phase.row_version FROM learning_groups g JOIN course_assessment_plans p ON p.learning_offering_id=g.learning_offering_id JOIN course_assessment_phases phase ON phase.plan_id=p.id WHERE g.id=$1 ORDER BY phase.id LIMIT 1")
        .bind(unused).fetch_one(&pool).await.unwrap();
    let pupil:Uuid=sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 ORDER BY id LIMIT 1")
        .bind(base).fetch_one(&pool).await.unwrap();
    let day: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&pool)
        .await
        .unwrap();
    for branch in [
        "phase_only",
        "response_only",
        "score_with_required_item",
        "final_with_required_lock",
    ] {
        let mut tx = pool.begin().await.unwrap();
        assert!(evidence::groups_with_established_history_on_connection(
            &mut tx,
            ctx.academic_term_id,
            ctx.academic_year_id,
            &[unused]
        )
        .await
        .unwrap()
        .is_empty());
        assert!(
            school_academic_results::services::current_obligation_group_ids_on_connection(
                &mut tx,
                ctx.academic_term_id,
                ctx.academic_year_id,
                day,
                &[unused]
            )
            .await
            .unwrap()
            .is_empty(),
            "every rollback branch starts as omitted with zero evidence"
        );
        match branch {
            "phase_only" => {
                sqlx::query("INSERT INTO learning_group_phase_confirmations(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,course_assessment_plan_id,assessment_phase_id,phase_row_version,roster_checksum,source_checksum,source_snapshot,confirmed_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$8,'{\"invalidated\":true}',$9)")
                    .bind(unused).bind(offering).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(plan).bind(phase).bind(phase_version).bind("a".repeat(64)).bind(actor.user_id).execute(&mut *tx).await.unwrap();
            }
            "response_only" => {
                sqlx::query("INSERT INTO learning_group_student_evaluations(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,domain,subject_term_criterion_id,student_academic_year_id,quality_level,updated_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,2,$9)")
                    .bind(unused).bind(offering).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(subject).bind(DC.as_str()).bind(criterion).bind(pupil).bind(actor.user_id).execute(&mut *tx).await.unwrap();
            }
            "score_with_required_item" => {
                let item = Uuid::new_v4();
                sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) VALUES($1,$2,$3,$4,$5,$6,$7,'Required score companion',10)")
                    .bind(item).bind(unused).bind(offering).bind(plan).bind(phase).bind(ctx.academic_term_id).bind(ctx.academic_year_id).execute(&mut *tx).await.unwrap();
                sqlx::query("INSERT INTO learning_group_student_scores(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score,updated_by) VALUES($1,$2,$3,$4,$5,$6,7.25,$7)")
                    .bind(unused).bind(offering).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(item).bind(pupil).bind(actor.user_id).execute(&mut *tx).await.unwrap();
            }
            "final_with_required_lock" => {
                let lock = Uuid::new_v4();
                sqlx::query("INSERT INTO subject_term_evaluation_locks(id,subject_id,academic_term_id,academic_year_id,domain,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES($1,$2,$3,$4,$5,$6,$6,'{\"groups\":[]}',$7)")
                    .bind(lock).bind(subject).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(DC.as_str()).bind("b".repeat(64)).bind(actor.user_id).execute(&mut *tx).await.unwrap();
                sqlx::query("INSERT INTO subject_term_student_evaluations(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,domain,subject_term_criterion_id,evaluation_lock_id,student_academic_year_id,quality_level) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,3)")
                    .bind(unused).bind(offering).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(subject).bind(DC.as_str()).bind(criterion).bind(lock).bind(pupil).execute(&mut *tx).await.unwrap();
            }
            _ => unreachable!(),
        }
        let counts:(i64,i64,i64,i64,i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM learning_group_students WHERE learning_group_id=$1),(SELECT count(*) FROM learning_group_score_items WHERE learning_group_id=$1),(SELECT count(*) FROM learning_group_student_scores WHERE learning_group_id=$1),(SELECT count(*) FROM learning_group_phase_confirmations WHERE learning_group_id=$1),(SELECT count(*) FROM learning_group_student_evaluations WHERE learning_group_id=$1),(SELECT count(*) FROM learning_group_evaluation_confirmations WHERE learning_group_id=$1),(SELECT count(*) FROM subject_term_student_evaluations WHERE learning_group_id=$1),(SELECT count(*) FROM subject_term_evaluation_locks WHERE subject_id=$2 AND academic_term_id=$3)")
            .bind(unused).bind(subject).bind(ctx.academic_term_id).fetch_one(&mut *tx).await.unwrap();
        let expected = match branch {
            "phase_only" => (0, 0, 0, 1, 0, 0, 0, 0),
            "response_only" => (0, 0, 0, 0, 1, 0, 0, 0),
            "score_with_required_item" => (0, 1, 1, 0, 0, 0, 0, 0),
            "final_with_required_lock" => (0, 0, 0, 0, 0, 0, 1, 1),
            _ => unreachable!(),
        };
        assert_eq!(counts, expected, "masking evidence count for {branch}");
        // Scores cannot exist without their item FK, nor final evaluations without
        // their immutable lock FK. These two cases deliberately verify coupled
        // preservation; only phase/response cases assert isolated leaf coverage.
        let snapshot_sql="SELECT jsonb_build_object('items',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_score_items t WHERE learning_group_id=$1),'scores',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_scores t WHERE learning_group_id=$1),'phase',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_phase_confirmations t WHERE learning_group_id=$1),'responses',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_evaluations t WHERE learning_group_id=$1),'finals',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM subject_term_student_evaluations t WHERE learning_group_id=$1),'locks',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM subject_term_evaluation_locks t WHERE subject_id=$2 AND academic_term_id=$3))";
        let before: Value = sqlx::query_scalar(snapshot_sql)
            .bind(unused)
            .bind(subject)
            .bind(ctx.academic_term_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        let leaf = evidence::groups_with_established_history_on_connection(
            &mut tx,
            ctx.academic_term_id,
            ctx.academic_year_id,
            &[unused],
        )
        .await
        .unwrap();
        assert!(
            leaf.contains(&unused),
            "Assessment leaf must retain {branch}"
        );
        let retained =
            school_academic_results::services::current_obligation_group_ids_on_connection(
                &mut tx,
                ctx.academic_term_id,
                ctx.academic_year_id,
                day,
                &[unused],
            )
            .await
            .unwrap();
        assert_eq!(
            retained,
            vec![unused],
            "public classifier must retain {branch}"
        );
        let after: Value = sqlx::query_scalar(snapshot_sql)
            .bind(unused)
            .bind(subject)
            .bind(ctx.academic_term_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(
            before, after,
            "established {branch} bytes must remain exact"
        );
        tx.rollback().await.unwrap();
    }
}
