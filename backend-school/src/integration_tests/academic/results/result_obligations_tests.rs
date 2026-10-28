use super::*;
use school_academic_delivery::{
    models as dm,
    services::{change_sets, versions},
};

#[tokio::test]
async fn results_current_obligation_omitted_unused_group_does_not_block_ready_course() {
    let (pool, actor, ctx, group, unused) =
        fixture_with_unused_group("results_current_obligation_unused", true).await;
    let unused = unused.unwrap();
    // Keep this runtime closing fixture writable on the execution date without changing
    // historical published source graphs or bypassing Delivery's effective-date guard.
    let today = (chrono::Utc::now() + chrono::Duration::hours(7)).date_naive();
    let end = today + chrono::Duration::days(30);
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2) WHERE id=$1")
        .bind(ctx.academic_year_id)
        .bind(end)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE academic_terms SET planned_end_date=GREATEST(planned_end_date,$2) WHERE id=$1",
    )
    .bind(ctx.academic_term_id)
    .bind(end)
    .execute(&pool)
    .await
    .unwrap();
    let (offering,subject):(Uuid,Uuid)=sqlx::query_as("SELECT g.learning_offering_id,d.subject_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$1").bind(group).fetch_one(&pool).await.unwrap();
    let old_source:Uuid=sqlx::query_scalar("SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' AND EXISTS(SELECT 1 FROM jsonb_array_elements(snapshot->'offerings') o CROSS JOIN jsonb_array_elements(o->'groups') g WHERE (g->>'id')::uuid=$2) ORDER BY effective_from DESC,id DESC LIMIT 1").bind(ctx.academic_term_id).bind(unused).fetch_one(&pool).await.expect("087 must capture the pre-cutover unused group");
    let old_bytes: String = sqlx::query_scalar(
        "SELECT to_jsonb(v)::text FROM academic_delivery_versions v WHERE id=$1",
    )
    .bind(old_source)
    .fetch_one(&pool)
    .await
    .unwrap();
    for (candidate, teacher) in course_subject_rooms(&pool, &ctx, subject)
        .await
        .into_iter()
        .filter(|(id, _)| *id != unused)
    {
        let teacher = ActorContext {
            user_id: teacher,
            permissions: actor.permissions.clone(),
        };
        prepare_phases(&pool, &teacher, &ctx, candidate).await;
        let workspace = get_course_workspace(&pool, &teacher, &ctx, candidate)
            .await
            .unwrap();
        let confirmed =
            confirm_group_results(&pool, &teacher, &ctx, candidate, confirm_input(&workspace))
                .await
                .unwrap();
        assert!(
            confirmed.blockers.is_empty(),
            "retained group must really be ready: {:?}",
            confirmed.blockers
        );
        assert!(confirmed.confirmation_is_current);
    }
    let absence:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM learning_group_students WHERE learning_group_id=$1) AND NOT EXISTS(SELECT 1 FROM learning_group_score_items WHERE learning_group_id=$1) AND NOT EXISTS(SELECT 1 FROM learning_group_phase_confirmations WHERE learning_group_id=$1) AND NOT EXISTS(SELECT 1 FROM learning_group_result_confirmations WHERE learning_group_id=$1)").bind(unused).fetch_one(&pool).await.unwrap();
    assert!(absence, "unused group has no learner or grade obligation");
    let revision = change_sets::create_change_set(
        &pool,
        actor.user_id,
        dm::CreateAcademicTermChangeSetRequest {
            academic_term_id: ctx.academic_term_id,
            reason: "Remove unused group from current scope".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let target = versions::get_version(&pool, revision.target_delivery_version_id)
        .await
        .unwrap();
    change_sets::remove_draft_group(
        &pool,
        actor.user_id,
        revision.id,
        unused,
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
            .any(|f| f.severity == dm::AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
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
                .filter(|f| f.severity == dm::AcademicChangeFindingSeverity::Warning)
                .map(|f| f.code)
                .collect(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let selected = versions::get_version(&pool, target.id).await.unwrap();
    let included = selected
        .snapshot
        .offerings
        .iter()
        .find(|o| o.id == offering)
        .unwrap();
    assert!(included.groups.iter().all(|g| g.id != unused));
    assert!(included.groups.iter().any(|g| g.id == group));
    let outcome = lock_course_subject(&pool, &academic_affairs_actor(&actor), &ctx, subject)
        .await
        .unwrap();
    assert!(
        outcome.lock.is_some(),
        "ready included M6 group must lock despite unused omitted M4 group: {:?}",
        outcome.groups
    );
    assert!(outcome.groups.iter().all(|g| g.learning_group_id != unused));
    let after: String = sqlx::query_scalar(
        "SELECT to_jsonb(v)::text FROM academic_delivery_versions v WHERE id=$1",
    )
    .bind(old_source)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old_bytes, after);
}

struct ScopedResults {
    pool: sqlx::PgPool,
    actor: ActorContext,
    context: ResultContext,
    kept: Uuid,
    omitted: Uuid,
    offering: Uuid,
    subject: Uuid,
    source: Uuid,
    date: NaiveDate,
    activity: Option<Uuid>,
    omitted_activity: Option<Uuid>,
}

async fn scoped_results(
    name: &str,
    prepare: bool,
    activity: bool,
    published_unused: bool,
    future: bool,
) -> ScopedResults {
    let (pool, actor, context, kept, omitted, omitted_activity) =
        fixture_with_obligation_groups(name, true, activity, published_unused).await;
    let omitted = omitted.unwrap();
    let today = (chrono::Utc::now() + chrono::Duration::hours(7)).date_naive();
    let end = today + chrono::Duration::days(90);
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2) WHERE id=$1")
        .bind(context.academic_year_id)
        .bind(end)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE academic_terms SET planned_end_date=GREATEST(planned_end_date,$2) WHERE id=$1",
    )
    .bind(context.academic_term_id)
    .bind(end)
    .execute(&pool)
    .await
    .unwrap();
    let (offering,subject):(Uuid,Uuid)=sqlx::query_as("SELECT g.learning_offering_id,d.subject_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$1").bind(kept).fetch_one(&pool).await.unwrap();
    if prepare {
        for (group, teacher) in course_subject_rooms(&pool, &context, subject)
            .await
            .into_iter()
            .filter(|(id, _)| *id != omitted)
        {
            let teacher = ActorContext {
                user_id: teacher,
                permissions: actor.permissions.clone(),
            };
            prepare_phases(&pool, &teacher, &context, group).await;
            let ws = get_course_workspace(&pool, &teacher, &context, group)
                .await
                .unwrap();
            let ws = confirm_group_results(&pool, &teacher, &context, group, confirm_input(&ws))
                .await
                .unwrap();
            assert!(ws.blockers.is_empty());
            assert!(ws.confirmation_is_current);
        }
    }
    if published_unused {
        let preview = delivery_groups::preview_roster(&pool, omitted)
            .await
            .unwrap();
        assert!(
            preview.students.is_empty(),
            "unused split group has no source-room candidates"
        );
        let group = delivery_groups::get(&pool, omitted).await.unwrap();
        let group = delivery_groups::apply_roster(
            &pool,
            actor.user_id,
            omitted,
            dm::ApplyRosterRequest {
                row_version: group.row_version,
                source_hash: preview.source_hash,
                overrides: vec![],
            },
        )
        .await
        .unwrap();
        delivery_groups::publish_roster(
            &pool,
            actor.user_id,
            omitted,
            dm::PublishRosterRequest {
                row_version: group.row_version,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .unwrap();
    }
    let activity = if activity {
        Some(sqlx::query_scalar("SELECT g.id FROM learning_groups g JOIN activity_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.academic_term_id=$1 AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1").bind(context.academic_term_id).fetch_one(&pool).await.unwrap())
    } else {
        None
    };
    let mut revision = change_sets::create_change_set(
        &pool,
        actor.user_id,
        dm::CreateAcademicTermChangeSetRequest {
            academic_term_id: context.academic_term_id,
            reason: "Scoped result obligation fixture".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    if future {
        revision = change_sets::update_change_set(
            &pool,
            actor.user_id,
            revision.id,
            dm::UpdateAcademicTermChangeSetRequest {
                row_version: revision.row_version,
                reference_date: today + chrono::Duration::days(1),
                reason: "Future scope boundary".into(),
            },
        )
        .await
        .unwrap();
    }
    let initial_source = versions::get_version(&pool, revision.target_delivery_version_id)
        .await
        .unwrap();
    for intended in std::iter::once(omitted).chain(omitted_activity) {
        assert!(initial_source.snapshot.offerings.iter().flat_map(|offering|&offering.groups).any(|group|group.id==intended),"fixture prerequisite: every intended removed group must exist in the canonical selected source before removal: {intended}");
    }
    for group in std::iter::once(omitted).chain(omitted_activity) {
        let current = change_sets::get_change_set(&pool, revision.id)
            .await
            .unwrap();
        let target = versions::get_version(&pool, current.target_delivery_version_id)
            .await
            .unwrap();
        change_sets::remove_draft_group(
            &pool,
            actor.user_id,
            current.id,
            group,
            dm::RemoveDeliveryDraftResourceRequest {
                change_set_row_version: current.row_version,
                target_delivery_version_row_version: target.row_version,
            },
        )
        .await
        .unwrap();
    }
    let preview =
        change_sets::preview_change_set_at(&pool, revision.id, Some(revision.reference_date))
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
                .filter(|f| f.severity == dm::AcademicChangeFindingSeverity::Warning)
                .map(|f| f.code)
                .collect(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    ScopedResults {
        pool,
        actor,
        context,
        kept,
        omitted,
        offering,
        subject,
        source: revision.target_delivery_version_id,
        date: revision.reference_date,
        activity,
        omitted_activity,
    }
}

async fn classify(f: &ScopedResults, date: NaiveDate, ids: &[Uuid]) -> Result<Vec<Uuid>, AppError> {
    let mut connection = f.pool.acquire().await.unwrap();
    current_obligation_group_ids_on_connection(
        &mut connection,
        f.context.academic_term_id,
        f.context.academic_year_id,
        date,
        ids,
    )
    .await
}
async fn obligation_facts(connection: &mut sqlx::PgConnection) -> serde_json::Value {
    sqlx::query_scalar("SELECT jsonb_build_object('members',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_students t),'overrides',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_result_overrides t),'confirms',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_result_confirmations t),'activityValues',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_activity_evaluations t),'activityConfirms',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_activity_result_confirmations t),'courseResults',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_course_results t),'activityResults',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_activity_results t),'courseLocks',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_course_result_locks t),'activityLocks',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_activity_result_locks t),'published',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_delivery_versions t WHERE status='published'))").fetch_one(connection).await.unwrap()
}

#[tokio::test]
async fn results_current_obligation_classifier_context_dates_empty_and_legacy() {
    let f = scoped_results("results_obligation_context", false, false, false, true).await;
    let before = f.date - chrono::Duration::days(1);
    let mut expected = vec![f.kept, f.omitted];
    expected.sort_unstable();
    assert_eq!(
        classify(&f, before, &[f.omitted, f.kept, f.omitted])
            .await
            .unwrap(),
        expected,
        "future restriction must not activate early; IDs normalize"
    );
    assert_eq!(
        classify(&f, f.date, &[f.omitted, f.kept]).await.unwrap(),
        vec![f.kept]
    );
    assert!(classify(&f, f.date, &[]).await.unwrap().is_empty());
    assert!(matches!(
        classify(&f, f.date, &[f.kept, Uuid::new_v4()]).await,
        Err(AppError::Conflict(_))
    ));
    let mut connection = f.pool.acquire().await.unwrap();
    assert!(matches!(
        current_obligation_group_ids_on_connection(
            &mut connection,
            f.context.academic_term_id,
            Uuid::new_v4(),
            f.date,
            &[f.kept]
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let too_many: Vec<_> = (0..10_001).map(|_| Uuid::new_v4()).collect();
    assert!(matches!(
        current_obligation_group_ids_on_connection(
            &mut connection,
            f.context.academic_term_id,
            f.context.academic_year_id,
            f.date,
            &too_many
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    drop(connection);
    let revision = change_sets::create_change_set(
        &f.pool,
        f.actor.user_id,
        dm::CreateAcademicTermChangeSetRequest {
            academic_term_id: f.context.academic_term_id,
            reason: "Explicit empty audience".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let target = versions::get_version(&f.pool, revision.target_delivery_version_id)
        .await
        .unwrap();
    change_sets::remove_draft_offering(
        &f.pool,
        f.actor.user_id,
        revision.id,
        f.offering,
        dm::RemoveDeliveryDraftResourceRequest {
            change_set_row_version: revision.row_version,
            target_delivery_version_row_version: target.row_version,
        },
    )
    .await
    .unwrap();
    let preview =
        change_sets::preview_change_set_at(&f.pool, revision.id, Some(revision.reference_date))
            .await
            .unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|x| x.severity == dm::AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        &f.pool,
        f.actor.user_id,
        revision.id,
        dm::PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: preview
                .findings
                .iter()
                .filter(|x| x.severity == dm::AcademicChangeFindingSeverity::Warning)
                .map(|x| x.code)
                .collect(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let mut connection = f.pool.acquire().await.unwrap();
    let scope = school_academic_delivery::services::audiences::dated_constraint_on_connection(
        &mut connection,
        f.offering,
        revision.reference_date,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(scope.offering.is_none());
    drop(connection);
    assert_eq!(
        classify(&f, revision.reference_date, &[f.kept, f.omitted])
            .await
            .unwrap(),
        vec![f.kept],
        "empty audience omits unused but preserves established membership obligations"
    );
}

#[tokio::test]
async fn results_current_obligation_activity_ready_bulk_and_direct_scope_agree() {
    let f = scoped_results("results_obligation_activity", false, true, false, false).await;
    let kept = f.activity.unwrap();
    let omitted = f.omitted_activity.unwrap();
    let teacher:Uuid=sqlx::query_scalar("SELECT teacher_id FROM learning_group_teachers WHERE learning_group_id=$1 AND role='primary' ORDER BY id LIMIT 1").bind(kept).fetch_one(&f.pool).await.unwrap();
    let teacher = ActorContext {
        user_id: teacher,
        permissions: f.actor.permissions.clone(),
    };
    let prepared = prepare_activity_group(&f.pool, &teacher, &f.context, kept).await;
    assert!(prepared.blockers.is_empty());
    assert!(prepared.confirmation_is_current);
    let mut connection = f.pool.acquire().await.unwrap();
    let before = obligation_facts(&mut connection).await;
    drop(connection);
    let error = lock_activity_group(
        &f.pool,
        &academic_affairs_actor(&f.actor),
        &f.context,
        omitted,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, AppError::ValidationError(_)));
    let mut connection = f.pool.acquire().await.unwrap();
    assert_eq!(
        before,
        obligation_facts(&mut connection).await,
        "omitted empty direct lock inserts no lock/result or history"
    );
    drop(connection);
    let queue = readiness(&f.pool, &academic_affairs_actor(&f.actor), &f.context)
        .await
        .unwrap();
    assert!(queue
        .activities
        .iter()
        .any(|g| g.learning_group_id == kept && g.ready));
    assert!(queue
        .activities
        .iter()
        .all(|g| g.learning_group_id != omitted));
    let locked = lock_all_ready_activities(&f.pool, &academic_affairs_actor(&f.actor), &f.context)
        .await
        .unwrap();
    assert!(locked.locked.iter().any(|g| g.learning_group_id == kept));
    assert!(locked.locked.iter().all(|g| g.learning_group_id != omitted));
    assert!(locked
        .skipped
        .iter()
        .all(|g| g.learning_group_id != omitted));
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_activity_result_locks WHERE learning_group_id=$1",
    )
    .bind(omitted)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

async fn seed_unused_groups(f: &ScopedResults, count: i32) -> Vec<Uuid> {
    sqlx::query_scalar("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) SELECT id,$1,$2,$3,id::text,'Synthetic unused group','draft','draft' FROM (SELECT gen_random_uuid() id FROM generate_series(1,$4)) ids RETURNING id")
        .bind(f.offering).bind(f.context.academic_term_id).bind(f.context.academic_year_id).bind(count).fetch_all(&f.pool).await.unwrap()
}

#[tokio::test]
async fn results_current_obligation_limits_apply_after_filter_and_raw_budget_refuses() {
    let f = scoped_results("results_obligation_limits", true, false, false, false).await;
    // Seed after publication so the canonical source stays small. These raw groups
    // have no recorded learner facts and are absent from the explicit scope.
    let omitted = seed_unused_groups(&f, 5001).await;
    assert!(classify(&f, f.date, &omitted).await.unwrap().is_empty());
    let queue = readiness(&f.pool, &academic_affairs_actor(&f.actor), &f.context)
        .await
        .unwrap();
    assert!(queue
        .courses
        .iter()
        .flat_map(|s| &s.groups)
        .all(|g| !omitted.contains(&g.learning_group_id)));
    let locked = lock_course_subject(
        &f.pool,
        &academic_affairs_actor(&f.actor),
        &f.context,
        f.subject,
    )
    .await
    .unwrap();
    assert!(
        locked.lock.is_some(),
        "more than500 omitted groups cannot consume the500 normal course lock limit"
    );
    assert!(locked.groups.len() < 500);
    // Select the legitimate old registry-semantic source through the existing
    // term-clamped operation date. All synthetic groups now remain eligible.
    let legacy = f.date - chrono::Duration::days(1);
    sqlx::query("UPDATE academic_terms SET planned_end_date=$2 WHERE id=$1")
        .bind(f.context.academic_term_id)
        .bind(legacy)
        .execute(&f.pool)
        .await
        .unwrap();
    let mut connection = f.pool.acquire().await.unwrap();
    assert!(
        school_academic_delivery::services::audiences::dated_constraint_on_connection(
            &mut connection,
            f.offering,
            legacy
        )
        .await
        .unwrap()
        .is_none()
    );
    drop(connection);
    let error = lock_course_subject(
        &f.pool,
        &academic_affairs_actor(&f.actor),
        &f.context,
        f.subject,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error,AppError::ValidationError(message) if message.contains("500 active learning groups")),
        "{error:?}"
    );
    let error = readiness(&f.pool, &academic_affairs_actor(&f.actor), &f.context)
        .await
        .unwrap_err();
    assert!(
        matches!(&error,AppError::ValidationError(message) if message.contains("5000 groups")),
        "{error:?}"
    );
    seed_unused_groups(&f, 5001).await;
    let error = readiness(&f.pool, &academic_affairs_actor(&f.actor), &f.context)
        .await
        .unwrap_err();
    assert!(
        matches!(&error,AppError::ValidationError(message) if message.contains("10000 groups")),
        "raw scan must refuse, never truncate: {error:?}"
    );
}

#[tokio::test]
async fn results_current_obligation_isolated_result_evidence_preserves_bytes() {
    let f = scoped_results("results_obligation_evidence", false, true, false, false).await;
    let activity = f.omitted_activity.unwrap();
    let student: Uuid = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(f.context.academic_year_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    let policy: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_grading_policy_versions WHERE lifecycle='active'",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert!(classify(&f, f.date, &[f.omitted, activity])
        .await
        .unwrap()
        .is_empty());
    for branch in [
        "removed_member",
        "override",
        "invalidated_course_confirmation",
        "course_lock_empty",
        "course_lock_unknown",
        "course_result_with_required_lock",
        "activity_outcome",
        "invalidated_activity_confirmation",
        "activity_lock_empty",
        "activity_result_with_required_lock",
    ] {
        let mut tx = f.pool.begin().await.unwrap();
        let target =
            if branch.starts_with("activity") || branch == "invalidated_activity_confirmation" {
                activity
            } else {
                f.omitted
            };
        match branch {
            "removed_member" => {
                sqlx::query("INSERT INTO learning_group_students(id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,left_at,published_at) SELECT gen_random_uuid(),$1,$2,$3,id,student_id,'removed','operational_change',$5,$5,now() FROM student_academic_years WHERE id=$4").bind(target).bind(f.context.academic_term_id).bind(f.context.academic_year_id).bind(student).bind(f.date-chrono::Duration::days(1)).execute(&mut *tx).await.unwrap();
            }
            "override" => {
                sqlx::query("INSERT INTO learning_group_result_overrides(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,student_academic_year_id,outcome) VALUES($1,$2,$3,$4,$5,$6,'manual_zero')").bind(target).bind(f.offering).bind(f.context.academic_term_id).bind(f.context.academic_year_id).bind(f.subject).bind(student).execute(&mut *tx).await.unwrap();
            }
            "invalidated_course_confirmation" => {
                sqlx::query("INSERT INTO learning_group_result_confirmations(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,policy_version_id,roster_checksum,source_checksum,source_snapshot,confirmed_by) VALUES($1,$2,$3,$4,$5,$6,repeat('a',64),repeat('b',64),'{\"invalidated\":true}'::jsonb,$7)").bind(target).bind(f.offering).bind(f.context.academic_term_id).bind(f.context.academic_year_id).bind(f.subject).bind(policy).bind(f.actor.user_id).execute(&mut *tx).await.unwrap();
            }
            "course_lock_empty" | "course_lock_unknown" | "course_result_with_required_lock" => {
                let source = if branch == "course_lock_unknown" {
                    serde_json::json!({"legacyUnknown":true})
                } else {
                    serde_json::json!({"groups":[{"learningGroupId":target,"students":[]}]})
                };
                let lock:Uuid=sqlx::query_scalar("INSERT INTO academic_course_result_locks(subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES($1,$2,$3,$4,'{}'::jsonb,repeat('a',64),repeat('b',64),$5,$6) RETURNING id").bind(f.subject).bind(f.context.academic_term_id).bind(f.context.academic_year_id).bind(policy).bind(source).bind(f.actor.user_id).fetch_one(&mut *tx).await.unwrap();
                if branch == "course_result_with_required_lock" {
                    sqlx::query("INSERT INTO academic_course_results(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,course_result_lock_id,student_academic_year_id,calculated_score,calculated_grade,outcome,numeric_grade,selection_source) VALUES($1,$2,$3,$4,$5,$6,$7,70,3,'numeric',3,'derived')").bind(target).bind(f.offering).bind(f.context.academic_term_id).bind(f.context.academic_year_id).bind(f.subject).bind(lock).bind(student).execute(&mut *tx).await.unwrap();
                }
            }
            "activity_outcome" => {
                sqlx::query("INSERT INTO academic_activity_evaluations(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,student_academic_year_id,outcome) SELECT id,learning_offering_id,academic_term_id,academic_year_id,$2,'pass' FROM learning_groups WHERE id=$1").bind(target).bind(student).execute(&mut *tx).await.unwrap();
            }
            "invalidated_activity_confirmation" => {
                sqlx::query("INSERT INTO academic_activity_result_confirmations(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,roster_checksum,source_checksum,source_snapshot,confirmed_by) SELECT id,learning_offering_id,academic_term_id,academic_year_id,repeat('a',64),repeat('b',64),'{\"invalidated\":true}'::jsonb,$2 FROM learning_groups WHERE id=$1").bind(target).bind(f.actor.user_id).execute(&mut *tx).await.unwrap();
            }
            "activity_lock_empty" | "activity_result_with_required_lock" => {
                let lock:Uuid=sqlx::query_scalar("INSERT INTO academic_activity_result_locks(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,roster_checksum,source_checksum,source_snapshot,locked_by) SELECT id,learning_offering_id,academic_term_id,academic_year_id,repeat('a',64),repeat('b',64),jsonb_build_object('learningGroupId',id,'outcomes','[]'::jsonb),$2 FROM learning_groups WHERE id=$1 RETURNING id").bind(target).bind(f.actor.user_id).fetch_one(&mut *tx).await.unwrap();
                if branch == "activity_result_with_required_lock" {
                    sqlx::query("INSERT INTO academic_activity_results(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,activity_result_lock_id,student_academic_year_id,outcome) SELECT id,learning_offering_id,academic_term_id,academic_year_id,$2,$3,'pass' FROM learning_groups WHERE id=$1").bind(target).bind(lock).bind(student).execute(&mut *tx).await.unwrap();
                }
            }
            _ => panic!("unknown evidence branch"),
        }
        let before = obligation_facts(&mut tx).await;
        let retained = current_obligation_group_ids_on_connection(
            &mut tx,
            f.context.academic_term_id,
            f.context.academic_year_id,
            f.date,
            &[f.omitted, activity],
        )
        .await
        .unwrap();
        assert_eq!(retained,vec![target],"isolated evidence branch {branch}; result rows necessarily retain their FK-required locks");
        assert_eq!(
            before,
            obligation_facts(&mut tx).await,
            "classification must never rewrite {branch}"
        );
        tx.rollback().await.unwrap();
        assert!(
            classify(&f, f.date, &[f.omitted, activity])
                .await
                .unwrap()
                .is_empty(),
            "no evidence from previous branch survives rollback"
        );
    }
}

async fn first_item(f: &ScopedResults) -> (Uuid, gm::GradebookContext) {
    let phase:Uuid=sqlx::query_scalar("SELECT phase.id FROM course_assessment_phases phase JOIN course_assessment_plans p ON p.id=phase.plan_id WHERE p.learning_offering_id=$1 ORDER BY phase.id LIMIT 1").bind(f.offering).fetch_one(&f.pool).await.unwrap();
    (
        phase,
        gm::GradebookContext {
            academic_year_id: f.context.academic_year_id,
            academic_term_id: f.context.academic_term_id,
        },
    )
}
async fn gate_writer(pool: &sqlx::PgPool, table: &str, field: &str, target: Uuid, key: i64) {
    // Only test-generated identifiers and UUIDs enter this disposable trigger.
    assert!(matches!(
        (table, field),
        ("learning_group_score_items", "learning_group_id")
            | ("academic_course_result_locks", "subject_id")
    ));
    let sql=format!("CREATE FUNCTION obligation_test_gate() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN IF NEW.{field}='{target}'::uuid THEN PERFORM pg_advisory_xact_lock({key}); END IF; RETURN NEW; END $$; CREATE TRIGGER obligation_test_gate BEFORE INSERT ON {table} FOR EACH ROW EXECUTE FUNCTION obligation_test_gate();");
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
        .execute(pool)
        .await
        .unwrap();
}
async fn wait_for_blocked(pool: &sqlx::PgPool, fragment: &str) {
    for _ in 0..200 {
        let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock' AND query LIKE '%'||$1||'%')").bind(fragment).fetch_one(pool).await.unwrap();
        if blocked {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("real owner writer did not reach expected blocked phase: {fragment}");
}
async fn source_bytes(f: &ScopedResults) -> String {
    sqlx::query_scalar("SELECT to_jsonb(v)::text FROM academic_delivery_versions v WHERE id=$1")
        .bind(f.source)
        .fetch_one(&f.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn results_current_obligation_history_first_real_gradebook_writer_is_retained() {
    let f = scoped_results(
        "results_obligation_history_first",
        true,
        false,
        false,
        false,
    )
    .await;
    let before = source_bytes(&f).await;
    let (phase, context) = first_item(&f).await;
    let key = 5_177_001_i64;
    gate_writer(
        &f.pool,
        "learning_group_score_items",
        "learning_group_id",
        f.omitted,
        key,
    )
    .await;
    let mut gate = f.pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(key)
        .execute(&mut *gate)
        .await
        .unwrap();
    let writer_pool = f.pool.clone();
    let writer_actor = f.actor.clone();
    let omitted = f.omitted;
    let writer = tokio::spawn(async move {
        gradebook_adapter::create_item(
            &writer_pool,
            &writer_actor,
            omitted,
            phase,
            &context,
            gm::ItemInput {
                name: "First real history".into(),
                max_score: "1.00".into(),
                display_order: 0,
                row_version: None,
            },
        )
        .await
    });
    wait_for_blocked(&f.pool, "INSERT INTO learning_group_score_items").await;
    let lock_pool = f.pool.clone();
    let lock_actor = academic_affairs_actor(&f.actor);
    let lock_context = f.context.clone();
    let subject = f.subject;
    let mut locking = tokio::spawn(async move {
        lock_course_subject(&lock_pool, &lock_actor, &lock_context, subject).await
    });
    wait_for_blocked(&f.pool, "FOR UPDATE OF offering").await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut locking)
            .await
            .is_err()
    );
    let released: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    assert!(released);
    drop(gate);
    let item = writer.await.unwrap().unwrap();
    let outcome = locking.await.unwrap().unwrap();
    assert!(outcome.lock.is_none());
    assert!(
        outcome
            .groups
            .iter()
            .any(|g| g.learning_group_id == f.omitted && !g.ready),
        "first committed grade history must restore the omitted group's normal obligations"
    );
    let retained = classify(&f, f.date, &[f.omitted]).await.unwrap();
    assert_eq!(retained, vec![f.omitted]);
    let row: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_score_items t WHERE id=$1")
            .bind(item.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(row["name"], "First real history");
    assert_eq!(before, source_bytes(&f).await);
}

#[tokio::test]
async fn results_current_obligation_lock_first_gradebook_refuses_but_backdated_roster_stays_fact() {
    let f = scoped_results("results_obligation_lock_first", true, false, true, false).await;
    let before = source_bytes(&f).await;
    let (phase, context) = first_item(&f).await;
    let student:Uuid=sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active' ORDER BY student_academic_year_id LIMIT 1").bind(f.kept).fetch_one(&f.pool).await.unwrap();
    let unused = delivery_groups::get(&f.pool, f.omitted).await.unwrap();
    assert_eq!(unused.status, dm::LearningOfferingStatus::Published);
    assert_eq!(unused.roster_status, dm::RosterStatus::Published);
    let key = 5_177_002_i64;
    gate_writer(
        &f.pool,
        "academic_course_result_locks",
        "subject_id",
        f.subject,
        key,
    )
    .await;
    let mut gate = f.pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(key)
        .execute(&mut *gate)
        .await
        .unwrap();
    let lock_pool = f.pool.clone();
    let lock_actor = academic_affairs_actor(&f.actor);
    let lock_context = f.context.clone();
    let subject = f.subject;
    let locking = tokio::spawn(async move {
        lock_course_subject(&lock_pool, &lock_actor, &lock_context, subject).await
    });
    wait_for_blocked(&f.pool, "INSERT INTO academic_course_result_locks").await;
    let writer_pool = f.pool.clone();
    let writer_actor = f.actor.clone();
    let omitted = f.omitted;
    let mut writer = tokio::spawn(async move {
        gradebook_adapter::create_item(
            &writer_pool,
            &writer_actor,
            omitted,
            phase,
            &context,
            gm::ItemInput {
                name: "Must be refused after lock".into(),
                max_score: "1.00".into(),
                display_order: 0,
                row_version: None,
            },
        )
        .await
    });
    wait_for_blocked(&f.pool, "FOR UPDATE").await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut writer)
            .await
            .is_err()
    );
    let member_pool = f.pool.clone();
    let actor = f.actor.user_id;
    let joined = f.date - chrono::Duration::days(1);
    let mut membership = tokio::spawn(async move {
        roster_memberships::add_membership(
            &member_pool,
            actor,
            omitted,
            dm::AddDatedRosterMembershipRequest {
                group_row_version: unused.row_version,
                student_academic_year_id: student,
                joined_at: joined,
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut membership)
            .await
            .is_err()
    );
    let released: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    assert!(released);
    drop(gate);
    let lock = locking
        .await
        .unwrap()
        .unwrap()
        .lock
        .expect("retained ready groups lock first");
    assert!(
        matches!(writer.await.unwrap(), Err(AppError::Conflict(_))),
        "Gradebook rechecks existing Results lock after waiting"
    );
    let membership = membership.await.unwrap().expect(
        "backdated roster request keeps existing admission policy; no new post-lock prohibition",
    );
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM academic_course_result_locks t WHERE id=$1")
            .bind(lock.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert!(stored["source_snapshot"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .all(|g| g["learningGroupId"] != serde_json::json!(f.omitted)));
    assert_eq!(
        classify(&f, f.date, &[f.omitted]).await.unwrap(),
        vec![f.omitted]
    );
    let member: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_students t WHERE id=$1")
            .bind(membership.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(member["roster_source"], "operational_change");
    assert_eq!(member["joined_at"], serde_json::json!(joined));
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM academic_course_result_locks t WHERE id=$1")
            .bind(lock.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(stored, after);
    assert_eq!(before, source_bytes(&f).await);
    let items: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_score_items WHERE learning_group_id=$1",
    )
    .bind(f.omitted)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(items, 0);
}
