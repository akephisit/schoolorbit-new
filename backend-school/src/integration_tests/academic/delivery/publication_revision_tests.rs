use super::*;
use school_academic_delivery::{
    models::{versions::DeliverySnapshot, AcademicTermChangeSet, AcademicTermChangeSetPreview},
    services::{publication as delivery_publication, versions as delivery_versions},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

async fn rows(pool: &PgPool, table: &str) -> Value {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COALESCE(jsonb_agg(to_jsonb(row) ORDER BY to_jsonb(row)::text),'[]'::jsonb) FROM {table} row"
    )))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn publication_state(pool: &PgPool) -> BTreeMap<&'static str, Value> {
    let mut state = BTreeMap::new();
    for table in [
        "academic_delivery_versions",
        "academic_term_change_sets",
        "academic_term_change_items",
        "learning_group_teachers",
        "academic_delivery_teacher_cutoffs",
        "academic_delivery_offering_effectivity",
        "learning_offerings",
        "learning_groups",
        "learning_group_students",
        "academic_timetable_versions",
        "academic_timetable_blocks",
        "academic_timetable_block_groups",
        "academic_timetable_block_group_instructors",
        "course_assessment_plans",
        "course_assessment_phases",
        "learning_group_score_items",
        "learning_group_student_scores",
        "learning_group_phase_confirmations",
        "learning_group_result_confirmations",
        "learning_group_evaluation_confirmations",
        "academic_activity_result_confirmations",
        "academic_audit_events",
        "attendance_sessions",
        "attendance_records",
    ] {
        state.insert(table, rows(pool, table).await);
    }
    state
}

async fn accepted_facts(pool: &PgPool) -> BTreeMap<&'static str, Value> {
    let mut state = BTreeMap::new();
    for table in [
        "learning_group_teachers",
        "learning_offerings",
        "academic_timetable_versions",
        "academic_timetable_blocks",
        "academic_timetable_block_groups",
        "academic_timetable_block_group_instructors",
    ] {
        state.insert(table, rows(pool, table).await);
    }
    state.insert("academic_delivery_versions", sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(v) ORDER BY id),'[]'::jsonb) FROM academic_delivery_versions v WHERE status='published'",
    ).fetch_one(pool).await.unwrap());
    state.insert("academic_term_change_sets", sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(receipt) ORDER BY id),'[]'::jsonb) FROM academic_term_change_sets receipt WHERE status='published'",
    ).fetch_one(pool).await.unwrap());
    state
}

async fn assert_original_facts(pool: &PgPool, original: &BTreeMap<&str, Value>) {
    for (table, facts) in original {
        let actual = rows(pool, table).await;
        for fact in facts.as_array().unwrap() {
            assert!(
                actual.as_array().unwrap().contains(fact),
                "publication rewrote accepted {table} row: {fact}"
            );
        }
    }
}

fn request(
    preview: &AcademicTermChangeSetPreview,
    key: Uuid,
) -> PublishAcademicTermChangeSetRequest {
    PublishAcademicTermChangeSetRequest {
        row_version: preview.change_set_row_version,
        target_delivery_version_row_version: preview.target_delivery_version_row_version,
        effective_from: preview.effective_from,
        preview_hash: preview.preview_hash.clone(),
        acknowledged_warning_codes: preview
            .findings
            .iter()
            .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
            .map(|finding| finding.code)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        idempotency_key: key,
    }
}

fn assert_ready(preview: &AcademicTermChangeSetPreview) {
    let blocking = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking)
        .collect::<Vec<_>>();
    assert!(
        blocking.is_empty(),
        "unexpected publication findings: {blocking:#?}"
    );
}

async fn publish(pool: &PgPool, actor: Uuid, revision: Uuid) -> AcademicTermChangeSet {
    let preview = publication_preview(pool, revision).await.unwrap();
    assert_ready(&preview);
    change_sets::publish_change_set(pool, actor, revision, request(&preview, Uuid::now_v7()))
        .await
        .unwrap()
}

async fn target_change(
    pool: &PgPool,
    context: &RuntimeContext,
    date: NaiveDate,
    target: i32,
) -> AcademicTermChangeSet {
    let draft = create_runtime_change_set_at(
        pool,
        context.teacher_id,
        context.term_id,
        date,
        &Uuid::now_v7().to_string(),
    )
    .await;
    let base = delivery_versions::get_version(pool, draft.base_delivery_version_id.unwrap())
        .await
        .unwrap();
    let offering = base.snapshot.offerings.first().unwrap().id;
    change_sets::upsert_change_item(
        pool,
        context.teacher_id,
        draft.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: draft.row_version,
            item_row_version: None,
            learning_offering_id: offering,
            weekly_period_target: target,
        },
    )
    .await
    .unwrap()
}

async fn applicable(pool: &PgPool, term: Uuid, date: NaiveDate) -> Uuid {
    let mut connection = pool.acquire().await.unwrap();
    delivery_publication::applicable_on_connection(&mut connection, term, date)
        .await
        .unwrap()
        .unwrap()
        .id
}

async fn offering_available(pool: &PgPool, offering: Uuid, date: NaiveDate) -> bool {
    sqlx::query_scalar("SELECT academic_delivery_offering_available($1,$2)")
        .bind(offering)
        .bind(date)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn snapshot(pool: &PgPool, revision: &AcademicTermChangeSet) -> DeliverySnapshot {
    delivery_versions::get_version(pool, revision.target_delivery_version_id)
        .await
        .unwrap()
        .snapshot
}

async fn wait_for_term_writers(pool: &PgPool, expected: i64) {
    tokio::time::timeout(std::time::Duration::from_secs(15),async {
        loop {
            let waiting:i64=sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock' AND query LIKE 'SELECT status FROM academic_terms WHERE id=%'",
            ).fetch_one(pool).await.unwrap();
            if waiting>=expected {break;}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("both publishers must finish preflight and wait on the locked term");
}

#[tokio::test]
async fn same_day_publications_replay_under_lock_and_earlier_replacement_never_reactivates_future_plans(
) {
    let pool =
        prepare_concurrent_delivery_runtime_fixture("publication151_service_chronology").await;
    let context = operational_change_runtime_context(&pool).await;
    let original = accepted_facts(&pool).await;
    let today = runtime_operation_date();
    let first = target_change(&pool, &context, today + Duration::days(10), 4).await;
    let base = first.base_delivery_version_id.unwrap();
    let preview = publication_preview(&pool, first.id).await.unwrap();
    assert_ready(&preview);
    let accepted_request = request(&preview, Uuid::now_v7());
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM academic_terms WHERE id=$1 FOR UPDATE")
        .bind(context.term_id)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let mut publishers = Vec::new();
    for _ in 0..2 {
        let pool = pool.clone();
        let request = accepted_request.clone();
        let actor = context.teacher_id;
        let id = first.id;
        publishers.push(tokio::spawn(async move {
            change_sets::publish_change_set(&pool, actor, id, request).await
        }));
    }
    wait_for_term_writers(&pool, 2).await;
    blocker.commit().await.unwrap();
    let a = publishers.remove(0).await.unwrap().unwrap();
    let replay = publishers.remove(0).await.unwrap().unwrap();
    assert_eq!(
        (
            a.id,
            a.target_delivery_version_id,
            a.publication_no,
            a.row_version
        ),
        (
            replay.id,
            replay.target_delivery_version_id,
            replay.publication_no,
            replay.row_version
        )
    );
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_term_change_set.published'").bind(a.id).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    let after_acceptance = publication_state(&pool).await;
    let changed_retry = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        a.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: today + Duration::days(9),
            ..accepted_request.clone()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(changed_retry, AppError::Conflict(_)));
    assert_eq!(publication_state(&pool).await, after_acceptance);
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(context.term_id)
        .execute(&pool)
        .await
        .unwrap();
    let closed_replay =
        change_sets::publish_change_set(&pool, context.teacher_id, a.id, accepted_request)
            .await
            .unwrap();
    assert_eq!(closed_replay.publication_no, a.publication_no);
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(context.term_id)
        .execute(&pool)
        .await
        .unwrap();
    let second = target_change(&pool, &context, today + Duration::days(10), 5).await;
    let b = publish(&pool, context.teacher_id, second.id).await;
    assert_ne!(a.target_delivery_version_id, b.target_delivery_version_id);
    assert_eq!(b.publication_no, a.publication_no.map(|number| number + 1));
    let third = target_change(&pool, &context, today + Duration::days(20), 6).await;
    let c = publish(&pool, context.teacher_id, third.id).await;
    assert_eq!(
        applicable(&pool, context.term_id, today + Duration::days(4)).await,
        base
    );
    assert_eq!(
        applicable(&pool, context.term_id, today + Duration::days(19)).await,
        b.target_delivery_version_id
    );
    let earlier = target_change(&pool, &context, today + Duration::days(5), 7).await;
    assert_eq!(
        earlier.base_delivery_version_id,
        Some(c.target_delivery_version_id)
    );
    let d = publish(&pool, context.teacher_id, earlier.id).await;
    for offset in [5, 10, 19, 20, 50] {
        assert_eq!(
            applicable(&pool, context.term_id, today + Duration::days(offset)).await,
            d.target_delivery_version_id
        );
    }
    let superseded = delivery_versions::get_version(&pool, c.target_delivery_version_id)
        .await
        .unwrap();
    assert!(superseded.is_superseded);
    assert_eq!(superseded.effective_until, Some(today + Duration::days(4)));
    let future = target_change(&pool, &context, today + Duration::days(15), 8).await;
    let e = publish(&pool, context.teacher_id, future.id).await;
    assert_eq!(
        applicable(&pool, context.term_id, today + Duration::days(14)).await,
        d.target_delivery_version_id
    );
    for offset in [15, 20, 50] {
        assert_eq!(
            applicable(&pool, context.term_id, today + Duration::days(offset)).await,
            e.target_delivery_version_id
        );
    }
    assert_original_facts(&pool, &original).await;
}

async fn future_replacement(
    pool: &PgPool,
    context: &RuntimeContext,
    date: NaiveDate,
) -> (AcademicTermChangeSet, Uuid, Uuid, Uuid, Uuid) {
    let draft = create_runtime_change_set_at(
        pool,
        context.teacher_id,
        context.term_id,
        date,
        &Uuid::now_v7().to_string(),
    )
    .await;
    let base = delivery_versions::get_version(pool, draft.base_delivery_version_id.unwrap())
        .await
        .unwrap();
    let group = base
        .snapshot
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .find(|group| {
            group
                .teachers
                .iter()
                .any(|teacher| teacher.role == LearningTeacherRole::Primary)
        })
        .unwrap();
    let previous = group
        .teachers
        .iter()
        .find(|teacher| teacher.role == LearningTeacherRole::Primary)
        .unwrap();
    let replacement = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$1::text,'fixture-not-a-login','Future','Teacher','staff','active')")
        .bind(replacement).execute(pool).await.unwrap();
    let added = change_sets::upsert_change_item(
        pool,
        context.teacher_id,
        draft.id,
        UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
            change_set_row_version: draft.row_version,
            item_row_version: None,
            learning_group_id: group.id,
            teacher_id: replacement,
            teacher_role: LearningTeacherRole::Primary,
        },
    )
    .await
    .unwrap();
    let stopped = change_sets::upsert_change_item(
        pool,
        context.teacher_id,
        draft.id,
        UpsertAcademicTermChangeItemRequest::StopGroupTeacher {
            change_set_row_version: added.row_version,
            item_row_version: None,
            learning_group_id: group.id,
            learning_group_teacher_id: previous.assignment_id,
            teacher_id: previous.teacher_id,
        },
    )
    .await
    .unwrap();
    let accepted = publish(pool, context.teacher_id, stopped.id).await;
    (
        accepted,
        group.id,
        previous.assignment_id,
        previous.teacher_id,
        replacement,
    )
}

#[tokio::test]
async fn earlier_plan_carries_the_exact_future_base_teacher_and_materializes_its_deterministic_episode(
) {
    let pool =
        prepare_concurrent_delivery_runtime_fixture("publication151_future_teacher_carry").await;
    let context = operational_change_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let (future, group, _, old_teacher, replacement) =
        future_replacement(&pool, &context, today + Duration::days(10)).await;
    let source = snapshot(&pool, &future).await;
    let source_teacher = source
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .find(|row| row.id == group)
        .unwrap()
        .teachers
        .iter()
        .find(|teacher| teacher.teacher_id == replacement)
        .unwrap();
    let original = accepted_facts(&pool).await;
    let earlier = target_change(&pool, &context, today + Duration::days(5), 4).await;
    let carried = snapshot(&pool, &earlier).await;
    let teachers = &carried
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .find(|row| row.id == group)
        .unwrap()
        .teachers;
    let teacher = teachers
        .iter()
        .find(|teacher| teacher.teacher_id == replacement)
        .unwrap();
    assert_eq!(teacher.role, source_teacher.role);
    assert_eq!(
        teacher.assignment_id,
        Uuid::new_v5(
            &earlier.id,
            format!("carried-teacher-episode:{}", source_teacher.assignment_id).as_bytes()
        )
    );
    assert!(!teachers
        .iter()
        .any(|teacher| teacher.teacher_id == old_teacher));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_group_teachers WHERE id=$1")
        .bind(teacher.assignment_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "preview must not materialize a prospective carried teacher"
    );
    publish(&pool, context.teacher_id, earlier.id).await;
    let episode:(Uuid,String,NaiveDate,Option<NaiveDate>,Uuid)=sqlx::query_as("SELECT teacher_id,role,starts_on,ends_on,started_by_change_set_id FROM learning_group_teachers WHERE id=$1")
        .bind(teacher.assignment_id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        episode,
        (
            replacement,
            "primary".into(),
            today + Duration::days(5),
            None,
            earlier.id
        )
    );
    assert_original_facts(&pool, &original).await;
}

#[tokio::test]
async fn prospective_carried_teacher_ids_accept_stop_and_adjust_but_absent_raw_assignments_are_stale(
) {
    for stop in [false, true] {
        let pool = prepare_concurrent_delivery_runtime_fixture(if stop {
            "publication151_prospective_stop"
        } else {
            "publication151_prospective_adjust"
        })
        .await;
        let context = operational_change_runtime_context(&pool).await;
        let today = runtime_operation_date();
        let (future, group, old_assignment, old_teacher, replacement) =
            future_replacement(&pool, &context, today + Duration::days(10)).await;
        let source = snapshot(&pool, &future).await;
        let source_assignment = source
            .offerings
            .iter()
            .flat_map(|offering| &offering.groups)
            .find(|row| row.id == group)
            .unwrap()
            .teachers
            .iter()
            .find(|teacher| teacher.teacher_id == replacement)
            .unwrap()
            .assignment_id;
        let draft = target_change(&pool, &context, today + Duration::days(5), 4).await;
        let prospective = Uuid::new_v5(
            &draft.id,
            format!("carried-teacher-episode:{source_assignment}").as_bytes(),
        );
        let raw = rows(&pool, "learning_group_teachers").await;
        let stale = change_sets::upsert_change_item(
            &pool,
            context.teacher_id,
            draft.id,
            UpsertAcademicTermChangeItemRequest::StopGroupTeacher {
                change_set_row_version: draft.row_version,
                item_row_version: None,
                learning_group_id: group,
                learning_group_teacher_id: old_assignment,
                teacher_id: old_teacher,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(stale, AppError::Conflict(_)));
        let with_primary = change_sets::upsert_change_item(
            &pool,
            context.teacher_id,
            draft.id,
            UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
                change_set_row_version: draft.row_version,
                item_row_version: None,
                learning_group_id: group,
                teacher_id: old_teacher,
                teacher_role: LearningTeacherRole::Primary,
            },
        )
        .await
        .unwrap();
        let command = if stop {
            UpsertAcademicTermChangeItemRequest::StopGroupTeacher {
                change_set_row_version: with_primary.row_version,
                item_row_version: None,
                learning_group_id: group,
                learning_group_teacher_id: prospective,
                teacher_id: replacement,
            }
        } else {
            UpsertAcademicTermChangeItemRequest::AdjustGroupTeacherRole {
                change_set_row_version: with_primary.row_version,
                item_row_version: None,
                learning_group_id: group,
                learning_group_teacher_id: prospective,
                teacher_id: replacement,
                teacher_role: LearningTeacherRole::Assistant,
            }
        };
        let changed = change_sets::upsert_change_item(&pool, context.teacher_id, draft.id, command)
            .await
            .unwrap();
        assert_eq!(
            rows(&pool, "learning_group_teachers").await,
            raw,
            "editing a prospective ID must preserve raw episodes"
        );
        let accepted = publish(&pool, context.teacher_id, changed.id).await;
        let graph = snapshot(&pool, &accepted).await;
        let teachers = &graph
            .offerings
            .iter()
            .flat_map(|offering| &offering.groups)
            .find(|row| row.id == group)
            .unwrap()
            .teachers;
        if stop {
            assert!(!teachers
                .iter()
                .any(|teacher| teacher.teacher_id == replacement));
        } else {
            assert!(teachers
                .iter()
                .any(|teacher| teacher.teacher_id == replacement
                    && teacher.role == LearningTeacherRole::Assistant));
        }
        for fact in raw.as_array().unwrap() {
            assert!(rows(&pool, "learning_group_teachers")
                .await
                .as_array()
                .unwrap()
                .contains(fact));
        }
    }
}

#[tokio::test]
async fn earlier_retention_overrides_future_offering_stop_and_later_stop_creates_only_its_real_gap()
{
    let pool =
        prepare_concurrent_delivery_runtime_fixture("publication151_offering_retention").await;
    let context = operational_change_runtime_context(&pool).await;
    let today = runtime_operation_date();
    // B removes an offering in the future; C explicitly retains the same stable
    // registry ID through the ordinary reviewed curriculum preparation flow.
    let stop = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        today + Duration::days(10),
        "publication151-stop-B",
    )
    .await;
    let base = delivery_versions::get_version(&pool, stop.base_delivery_version_id.unwrap())
        .await
        .unwrap();
    let curriculum = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            starts_on: today + Duration::days(5),
            delivery_version_id: Some(stop.target_delivery_version_id),
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    let offering = curriculum
        .proposals
        .iter()
        .filter_map(|proposal| proposal.existing_offering_id)
        .find(|id| {
            base.snapshot
                .offerings
                .iter()
                .any(|offering| offering.id == *id)
        })
        .expect("the existing curriculum pipeline must expose a retained base offering");
    let stopped = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        stop.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: stop.row_version,
            item_row_version: None,
            learning_offering_id: offering,
        },
    )
    .await
    .unwrap();
    let original = rows(&pool, "learning_offerings").await;
    let b = publish(&pool, context.teacher_id, stopped.id).await;
    assert!(offering_available(&pool, offering, today + Duration::days(9)).await);
    assert!(!offering_available(&pool, offering, today + Duration::days(10)).await);
    let c = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        today + Duration::days(5),
        "publication151-retain-C",
    )
    .await;
    let curriculum = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            starts_on: today + Duration::days(5),
            delivery_version_id: Some(c.target_delivery_version_id),
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    assert!(curriculum
        .proposals
        .iter()
        .any(|proposal| proposal.existing_offering_id == Some(offering)));
    let choices = curriculum
        .proposals
        .iter()
        .map(|proposal| CurriculumPreparationChoice {
            proposal_id: proposal.proposal_id.clone(),
            action: if proposal.existing_offering_id == Some(offering) {
                PreparationAction::DeferGroups
            } else {
                PreparationAction::Skip
            },
            groups: vec![],
        })
        .collect();
    let applied = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            starts_on: today + Duration::days(5),
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
            delivery_version_id: Some(c.target_delivery_version_id),
            source_hash: curriculum.source_hash,
            idempotency_key: Uuid::now_v7(),
            choices,
        },
    )
    .await
    .unwrap();
    assert!(applied.offering_ids.contains(&offering));
    let restored = change_sets::get_change_set(&pool, c.id).await.unwrap();
    let c = publish(&pool, context.teacher_id, restored.id).await;
    assert!(c.publication_no > b.publication_no);
    for offset in [5, 9, 10, 20] {
        assert!(offering_available(&pool, offering, today + Duration::days(offset)).await);
    }
    let d = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        today + Duration::days(15),
        "publication151-stop-D",
    )
    .await;
    let d = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        d.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: d.row_version,
            item_row_version: None,
            learning_offering_id: offering,
        },
    )
    .await
    .unwrap();
    publish(&pool, context.teacher_id, d.id).await;
    assert!(offering_available(&pool, offering, today + Duration::days(14)).await);
    assert!(!offering_available(&pool, offering, today + Duration::days(15)).await);
    assert_eq!(rows(&pool, "learning_offerings").await, original);
}

#[tokio::test]
async fn concurrent_distinct_drafts_serialize_and_the_loser_retains_its_unallocated_number() {
    let pool = prepare_concurrent_delivery_runtime_fixture("publication151_competing_drafts").await;
    let context = operational_change_runtime_context(&pool).await;
    let date = runtime_operation_date() + Duration::days(5);
    let a = target_change(&pool, &context, date, 4).await;
    let b = target_change(&pool, &context, date, 5).await;
    assert_eq!(a.base_delivery_version_id, b.base_delivery_version_id);
    let mut requests = Vec::new();
    for draft in [&a, &b] {
        let preview = publication_preview(&pool, draft.id).await.unwrap();
        assert_ready(&preview);
        requests.push(request(&preview, Uuid::now_v7()));
    }
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM academic_terms WHERE id=$1 FOR UPDATE")
        .bind(context.term_id)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for (draft, request) in [a.id, b.id].into_iter().zip(requests) {
        let pool = pool.clone();
        let actor = context.teacher_id;
        tasks.push(tokio::spawn(async move {
            change_sets::publish_change_set(&pool, actor, draft, request).await
        }));
    }
    wait_for_term_writers(&pool, 2).await;
    blocker.commit().await.unwrap();
    let a_result = tasks.remove(0).await.unwrap();
    let b_result = tasks.remove(0).await.unwrap();
    assert_ne!(a_result.is_ok(), b_result.is_ok());
    let loser = if a_result.is_ok() { b } else { a };
    let error = if let Err(error) = a_result {
        error
    } else {
        b_result.unwrap_err()
    };
    assert!(matches!(
        error,
        AppError::Conflict(_) | AppError::ValidationError(_)
    ));
    let state = change_sets::get_change_set(&pool, loser.id).await.unwrap();
    assert_eq!(state.status, AcademicTermChangeSetStatus::Draft);
    assert_eq!(state.publication_no, None);
    let preview = publication_preview(&pool, loser.id).await.unwrap();
    assert!(preview
        .findings
        .iter()
        .any(|finding| finding.code == AcademicChangeFindingCode::BaseDeliveryVersionStale));
    let partial:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM academic_delivery_teacher_cutoffs WHERE change_set_id=$1)+(SELECT count(*) FROM academic_delivery_offering_effectivity WHERE change_set_id=$1)+(SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_term_change_set.published')")
        .bind(loser.id).fetch_one(&pool).await.unwrap();
    assert_eq!(partial, 0);
}

async fn confirmed_teacher_change(
    pool: &PgPool,
    context: &RuntimeContext,
) -> (AcademicTermChangeSet, Uuid) {
    use school_academic_assessment::gradebook::{
        models::{ConfirmInput, GradebookContext, ItemInput},
        services::{confirm_phase, create_item, get_group_phase_workspace},
    };
    use school_permissions::registry::codes;

    let (group,teacher,phase):(Uuid,Uuid,Uuid)=sqlx::query_as(
        "SELECT allocation.learning_group_id,allocation.teacher_id,phase.id \
         FROM academic_delivery_latest_published_teacher_allocations allocation \
         JOIN course_assessment_plans plan ON plan.learning_offering_id=allocation.learning_offering_id \
         JOIN course_assessment_phases phase ON phase.plan_id=plan.id \
         WHERE allocation.academic_term_id=$1 AND allocation.role='primary' AND phase.phase_code='before_midterm' \
           AND NOT EXISTS(SELECT 1 FROM learning_group_score_items item WHERE item.learning_group_id=allocation.learning_group_id AND item.assessment_phase_id=phase.id AND item.lifecycle='active') \
         ORDER BY allocation.learning_group_id,allocation.teacher_id LIMIT 1",
    ).bind(context.term_id).fetch_one(pool).await.unwrap();
    let actor = school_authorization::ActorContext {
        user_id: teacher,
        permissions: vec![
            codes::ACADEMIC_GRADEBOOK_MANAGE_SCHOOL.into(),
            codes::ACADEMIC_GRADEBOOK_MANAGE_ASSIGNED.into(),
        ],
    };
    let gradebook = GradebookContext {
        academic_year_id: context.year_id,
        academic_term_id: context.term_id,
    };
    let initial = get_group_phase_workspace(pool, &actor, group, phase, &gradebook)
        .await
        .unwrap();
    assert!(initial.items.iter().all(|item| item.lifecycle != "active"));
    create_item(
        &crate::adapters::academic_result_locks::RESULT_LOCKS,
        pool,
        &actor,
        group,
        phase,
        &gradebook,
        ItemInput {
            name: "Accepted complete phase".into(),
            max_score: initial.phase_max_score,
            display_order: 0,
            row_version: None,
        },
    )
    .await
    .unwrap();
    let ready = get_group_phase_workspace(pool, &actor, group, phase, &gradebook)
        .await
        .unwrap();
    let confirmation = confirm_phase(
        &crate::adapters::academic_result_locks::RESULT_LOCKS,
        pool,
        &actor,
        group,
        phase,
        &gradebook,
        ConfirmInput {
            source_checksum: ready.source_checksum,
            roster_checksum: ready.roster_checksum,
            row_version: None,
        },
    )
    .await
    .unwrap();
    assert!(!confirmation.invalidated);
    let replacement = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$1::text,'fixture-not-a-login','Confirmation','Teacher','staff','active')")
        .bind(replacement).execute(pool).await.unwrap();
    let draft = create_runtime_change_set_at(
        pool,
        context.teacher_id,
        context.term_id,
        runtime_operation_date() + Duration::days(5),
        &Uuid::now_v7().to_string(),
    )
    .await;
    let changed = change_sets::upsert_change_item(
        pool,
        context.teacher_id,
        draft.id,
        UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
            change_set_row_version: draft.row_version,
            item_row_version: None,
            learning_group_id: group,
            teacher_id: replacement,
            teacher_role: LearningTeacherRole::Secondary,
        },
    )
    .await
    .unwrap();
    (changed, confirmation.id)
}

#[tokio::test]
async fn assessment_audit_and_terminal_failures_restore_every_publication_fact_and_retry_the_same_number(
) {
    for phase in ["assessment", "audit", "terminal"] {
        let pool = prepare_concurrent_delivery_runtime_fixture(&format!(
            "publication151_rollback_{phase}"
        ))
        .await;
        let context = operational_change_runtime_context(&pool).await;
        let (draft, confirmation_id) = confirmed_teacher_change(&pool, &context).await;
        let original_confirmation: Value = sqlx::query_scalar(
            "SELECT to_jsonb(c) FROM learning_group_phase_confirmations c WHERE id=$1",
        )
        .bind(confirmation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            original_confirmation["source_snapshot"]["invalidated"],
            false
        );
        let preview = publication_preview(&pool, draft.id).await.unwrap();
        assert_ready(&preview);
        let request = request(&preview, Uuid::now_v7());
        let expected:i64=sqlx::query_scalar("SELECT max(publication_no)+1 FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published'").bind(context.term_id).fetch_one(&pool).await.unwrap();
        let before = publication_state(&pool).await;
        if phase != "assessment" {
            let (table, condition) = if phase == "audit" {
                (
                    "academic_audit_events",
                    "NEW.event_code='academic_term_change_set.published'",
                )
            } else {
                ("academic_term_change_sets", "NEW.status='published'")
            };
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE FUNCTION fixture_publication_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF {condition} THEN RAISE EXCEPTION 'PUBLICATION151_FIXTURE_FAILURE' USING ERRCODE='check_violation'; END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_publication_failure BEFORE INSERT OR UPDATE ON {table} FOR EACH ROW EXECUTE FUNCTION fixture_publication_failure();"
            ))).execute(&pool).await.unwrap();
        }
        let failure = if phase == "assessment" {
            school_academic_delivery::services::change_sets::publish_change_set(
                &crate::adapters::publication_history::PUBLICATION_HISTORY,
                &RejectAssessmentPublication,
                &pool,
                context.teacher_id,
                draft.id,
                request.clone(),
            )
            .await
            .unwrap_err()
        } else {
            change_sets::publish_change_set(&pool, context.teacher_id, draft.id, request.clone())
                .await
                .unwrap_err()
        };
        if phase == "assessment" {
            assert!(matches!(failure, AppError::InternalServerError(_)));
        } else {
            assert!(matches!(failure, AppError::DbError(_)));
        }
        assert_eq!(
            publication_state(&pool).await,
            before,
            "{phase} failure left partial publication evidence"
        );
        if phase != "assessment" {
            let table = if phase == "audit" {
                "academic_audit_events"
            } else {
                "academic_term_change_sets"
            };
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP TRIGGER fixture_publication_failure ON {table}; DROP FUNCTION fixture_publication_failure();"))).execute(&pool).await.unwrap();
        }
        let accepted =
            change_sets::publish_change_set(&pool, context.teacher_id, draft.id, request)
                .await
                .unwrap();
        assert_eq!(accepted.publication_no, Some(expected));
        let invalidated: Value = sqlx::query_scalar(
            "SELECT to_jsonb(c) FROM learning_group_phase_confirmations c WHERE id=$1",
        )
        .bind(confirmation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            invalidated["row_version"].as_i64(),
            original_confirmation["row_version"]
                .as_i64()
                .map(|version| version + 1)
        );
        assert_eq!(invalidated["source_snapshot"]["invalidated"], true);
        let mut expected_confirmation = original_confirmation;
        expected_confirmation["row_version"] = invalidated["row_version"].clone();
        expected_confirmation["source_snapshot"]["invalidated"] = Value::Bool(true);
        assert_eq!(invalidated,expected_confirmation,"successful teacher publication must invalidate exactly the accepted confirmation revision");
    }
}

#[tokio::test]
async fn persisted_lesson_and_current_finalization_block_typed_history_while_arrival_and_old_generation_do_not(
) {
    let pool = prepare_concurrent_delivery_runtime_fixture("publication151_consumed_history").await;
    let context = operational_change_runtime_context(&pool).await;
    let date = runtime_operation_date() + Duration::days(5);
    let draft = target_change(&pool, &context, date, 4).await;
    sqlx::query("INSERT INTO attendance_sessions(academic_term_id,date,kind,source_key,title,teacher_ids,start_time,end_time,saved_at,saved_by) VALUES($1,$2,'arrival','publication151-arrival','Accepted arrival',ARRAY[$3::uuid],'08:00','08:30',now(),$3)")
        .bind(context.term_id).bind(date).bind(context.teacher_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_finalized_dates(generation,academic_term_id,date,processed_at) VALUES($1,$2,$3,now())")
        .bind(Uuid::now_v7()).bind(context.term_id).bind(date).execute(&pool).await.unwrap();
    let unblocked = publication_preview(&pool, draft.id).await.unwrap();
    assert_ready(&unblocked);
    let lesson:Uuid=sqlx::query_scalar("INSERT INTO attendance_sessions(academic_term_id,date,kind,source_key,title,teacher_ids,start_time,end_time,saved_at,saved_by) VALUES($1,$2,'lesson','publication151-lesson','Accepted lesson',ARRAY[$3::uuid],'08:30','09:20',now(),$3) RETURNING id")
        .bind(context.term_id).bind(date).bind(context.teacher_id).fetch_one(&pool).await.unwrap();
    let before = publication_state(&pool).await;
    let blocked = publication_preview(&pool, draft.id).await.unwrap();
    assert!(blocked.findings.iter().any(|finding| finding.code
        == AcademicChangeFindingCode::ConsumedHistoryConflict
        && finding.severity == AcademicChangeFindingSeverity::Blocking));
    let denied = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        draft.id,
        request(&blocked, Uuid::now_v7()),
    )
    .await
    .unwrap_err();
    assert!(matches!(denied, AppError::ValidationError(_)));
    assert_eq!(publication_state(&pool).await, before);
    sqlx::query("DELETE FROM attendance_sessions WHERE id=$1")
        .bind(lesson)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO attendance_finalized_dates(generation,academic_term_id,date,processed_at) SELECT generation,$1,$2,now() FROM attendance_module_state WHERE singleton")
        .bind(context.term_id).bind(date).execute(&pool).await.unwrap();
    let finalized = publication_preview(&pool, draft.id).await.unwrap();
    assert!(finalized
        .findings
        .iter()
        .any(|finding| finding.code == AcademicChangeFindingCode::ConsumedHistoryConflict));
    let after = date + Duration::days(1);
    let clear = change_sets::preview_change_set_at(&pool, draft.id, Some(after))
        .await
        .unwrap();
    assert_ready(&clear);
    let accepted = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        draft.id,
        request(&clear, Uuid::now_v7()),
    )
    .await
    .unwrap();
    assert_eq!(accepted.effective_from, Some(after));
}

#[tokio::test]
async fn retained_finite_legacy_teacher_gets_a_new_episode_that_remains_selected_after_the_original_end(
) {
    let pool =
        prepare_delivery_runtime_fixture_through("publication151_finite_legacy_carry", 150).await;
    let existing = operational_change_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let term = Uuid::now_v7();
    let offering = Uuid::now_v7();
    let group = Uuid::now_v7();
    let assignment = Uuid::now_v7();
    let base = Uuid::now_v7();
    let source:Uuid=sqlx::query_scalar("SELECT detail.learning_offering_id FROM course_offering_details detail JOIN learning_offering_targets target ON target.learning_offering_id=detail.learning_offering_id WHERE detail.academic_term_id=$1 AND target.homeroom_id=$2 ORDER BY detail.learning_offering_id LIMIT 1")
        .bind(existing.term_id).bind(existing.homeroom_id).fetch_one(&pool).await.unwrap();
    // Import a finite legacy episode while its group is still draft, exactly as
    // the provider's teacher-assignment fixture does. No immutable guard is disabled.
    let mut seed = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT $1,academic_year_id,'Finite legacy teacher',1000,'FINITE-LEGACY','custom',false,false,'planning',bell_schedule_id FROM academic_terms WHERE id=$2")
        .bind(term).bind(existing.term_id).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,status,starts_on,owning_organization_unit_id) SELECT $1,$2,academic_year_id,'course','FINITE-LEGACY','Finite legacy teacher','draft',$4,owning_organization_unit_id FROM learning_offerings WHERE id=$3")
        .bind(offering).bind(term).bind(source).bind(today-Duration::days(30)).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score) SELECT $1,$2,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$3")
        .bind(offering).bind(term).bind(source).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO learning_offering_targets(id,learning_offering_id,academic_term_id,academic_year_id,target_kind,homeroom_id,grade_level_id,study_program_id) SELECT uuidv7(),$1,$2,academic_year_id,target_kind,homeroom_id,grade_level_id,study_program_id FROM learning_offering_targets WHERE learning_offering_id=$3")
        .bind(offering).bind(term).bind(source).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) VALUES($1,$2,$3,$4,'FINITE-LEGACY','Finite legacy teacher','draft','draft')")
        .bind(group).bind(offering).bind(term).bind(existing.year_id).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO learning_group_homerooms(id,learning_group_id,academic_term_id,academic_year_id,homeroom_id,coverage_source) VALUES(uuidv7(),$1,$2,$3,$4,'manual')")
        .bind(group).bind(term).bind(existing.year_id).bind(existing.homeroom_id).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,ends_on) VALUES($1,$2,$3,$4,$5,'primary',$6,$7)")
        .bind(assignment).bind(group).bind(term).bind(existing.year_id).bind(existing.teacher_id).bind(today+Duration::days(1)).bind(today+Duration::days(9)).execute(&mut *seed).await.unwrap();
    sqlx::query("UPDATE learning_groups SET status='published' WHERE id=$1")
        .bind(group)
        .execute(&mut *seed)
        .await
        .unwrap();
    sqlx::query("UPDATE learning_offerings SET status='published',published_at=now() WHERE id=$1")
        .bind(offering)
        .execute(&mut *seed)
        .await
        .unwrap();
    sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,effective_from,status,snapshot,published_by,published_at,publication_idempotency_key,publication_request_hash) SELECT $1,$2,$3,$4,'published',academic_capture_delivery_snapshot($2,$4,jsonb_build_array(jsonb_build_object('id',$5::uuid,'weekly_period_target',1))),$6,now(),$1,repeat('a',64)")
        .bind(base).bind(term).bind(existing.year_id).bind(today+Duration::days(1)).bind(offering).bind(existing.teacher_id).execute(&mut *seed).await.unwrap();
    seed.commit().await.unwrap();
    school_test_db::run_test_migrations(&pool).await;
    let context = RuntimeContext {
        term_id: term,
        ..existing
    };
    let raw: Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_teachers t WHERE id=$1")
            .bind(assignment)
            .fetch_one(&pool)
            .await
            .unwrap();
    let draft = target_change(&pool, &context, today + Duration::days(5), 2).await;
    let graph = snapshot(&pool, &draft).await;
    let teacher = &graph.offerings[0].groups[0].teachers[0];
    let carried = Uuid::new_v5(
        &draft.id,
        format!("carried-teacher-episode:{assignment}").as_bytes(),
    );
    assert_eq!(
        (teacher.assignment_id, teacher.teacher_id, teacher.role),
        (carried, context.teacher_id, LearningTeacherRole::Primary)
    );
    let accepted = publish(&pool, context.teacher_id, draft.id).await;
    let after: Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_teachers t WHERE id=$1")
            .bind(assignment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, raw);
    let episode:(NaiveDate,Option<NaiveDate>,Uuid)=sqlx::query_as("SELECT starts_on,ends_on,started_by_change_set_id FROM learning_group_teachers WHERE id=$1")
        .bind(carried).fetch_one(&pool).await.unwrap();
    assert_eq!(episode, (today + Duration::days(5), None, accepted.id));
    assert_eq!(
        applicable(&pool, term, today + Duration::days(10)).await,
        accepted.target_delivery_version_id
    );
    let version = delivery_versions::get_version(&pool, accepted.target_delivery_version_id)
        .await
        .unwrap();
    assert_eq!(
        version.snapshot.offerings[0].groups[0].teachers[0].assignment_id,
        carried
    );
    let effective:bool=sqlx::query_scalar("SELECT starts_on<=$2 AND (effective_ends_on IS NULL OR effective_ends_on>=$2) FROM academic_delivery_effective_teacher_episodes WHERE id=$1")
        .bind(carried).bind(today+Duration::days(10)).fetch_one(&pool).await.unwrap();
    assert!(
        effective,
        "the new plan must retain the exact teacher after the imported episode's Nov9-style end"
    );
}

#[tokio::test]
async fn planning_catalog_start_and_active_no_retroactive_rules_are_typed_and_read_only() {
    let pool =
        prepare_concurrent_delivery_runtime_fixture("publication151_civil_date_refusals").await;
    let context = operational_change_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let draft = target_change(&pool, &context, today + Duration::days(5), 4).await;
    let graph = snapshot(&pool, &draft).await;
    let pinned_start:NaiveDate=sqlx::query_scalar("SELECT max(date) FROM (SELECT s.effective_from date FROM course_offering_details d JOIN subject_versions s ON s.id=d.subject_version_id WHERE d.learning_offering_id=ANY($1) UNION ALL SELECT a.effective_from FROM activity_offering_details d JOIN activity_versions a ON a.id=d.activity_version_id WHERE d.learning_offering_id=ANY($1)) starts")
        .bind(graph.offerings.iter().map(|offering|offering.id).collect::<Vec<_>>()).fetch_one(&pool).await.unwrap();
    let before = publication_state(&pool).await;
    let catalog =
        change_sets::preview_change_set_at(&pool, draft.id, Some(pinned_start - Duration::days(1)))
            .await
            .unwrap();
    assert!(catalog
        .findings
        .iter()
        .any(|finding| finding.code == AcademicChangeFindingCode::EffectiveDateInvalid));
    assert_eq!(publication_state(&pool).await, before);
    sqlx::query("UPDATE academic_terms SET status='active' WHERE id=$1")
        .bind(context.term_id)
        .execute(&pool)
        .await
        .unwrap();
    let retro =
        change_sets::preview_change_set_at(&pool, draft.id, Some(today - Duration::days(1)))
            .await
            .unwrap();
    assert!(retro
        .findings
        .iter()
        .any(|finding| finding.code == AcademicChangeFindingCode::EffectiveDateInvalid));
    let denied = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        draft.id,
        request(&retro, Uuid::now_v7()),
    )
    .await
    .unwrap_err();
    assert!(matches!(denied, AppError::ValidationError(_)));
    assert_eq!(publication_state(&pool).await, before);
}

#[tokio::test]
async fn timetable_same_day_numbers_are_independent_and_later_delivery_keeps_historical_pins() {
    use school_academic_timetable::{
        collaboration::TimetableMutationContext,
        models::{
            timetable_block::UpdateTimetableBlockRequest,
            timetable_publication::{
                PreviewTimetablePublicationRequest, PublishTimetableVersionRequest,
            },
        },
        services::timetable_lifecycle,
    };
    let pool =
        prepare_concurrent_delivery_runtime_fixture("publication151_timetable_sequences").await;
    let context = operational_change_runtime_context(&pool).await;
    let date = runtime_operation_date() + Duration::days(5);
    let opening_before = rows(&pool, "academic_delivery_versions").await;
    let mut accepted = Vec::new();
    for step in [1, 2] {
        let draft = independent_table_draft(&pool, &context).await;
        fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, draft.id).await;
        let block:Uuid=sqlx::query_scalar("SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND is_active ORDER BY id LIMIT 1")
            .bind(draft.id).fetch_one(&pool).await.unwrap();
        let block = timetable_block_service::get_block(&pool, block)
            .await
            .unwrap();
        let mutation = TimetableMutationContext::for_tests(&pool, context.teacher_id);
        timetable_block_service::update_block(
            &pool,
            &mutation,
            block.id,
            UpdateTimetableBlockRequest {
                timetable_version_id: draft.id,
                row_version: block.row_version,
                day_of_week: None,
                bell_schedule_period_id: None,
                title: None,
                clear_title: false,
                note: Some(format!("Same-day accepted plan {step}")),
                clear_note: false,
                room_id: None,
                clear_room: false,
                instructor_ids: None,
                teacher_ids: None,
            },
        )
        .await
        .unwrap();
        let current = timetable_version_service::get_version(&pool, draft.id, date)
            .await
            .unwrap();
        let preview = timetable_lifecycle::preview(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            &pool,
            draft.id,
            PreviewTimetablePublicationRequest {
                row_version: current.row_version,
                effective_from: date,
            },
        )
        .await
        .unwrap();
        assert!(preview.can_publish, "{:?}", preview.findings);
        let request = PublishTimetableVersionRequest {
            row_version: preview.row_version,
            effective_from: date,
            preview_hash: preview.preview_hash,
            idempotency_key: Uuid::now_v7(),
        };
        let published = timetable_lifecycle::publish(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            &pool,
            &mutation,
            draft.id,
            request.clone(),
        )
        .await
        .unwrap();
        let replay = timetable_lifecycle::publish(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            &pool,
            &mutation,
            draft.id,
            request,
        )
        .await
        .unwrap();
        assert_eq!(
            (
                published.id,
                published.publication_no,
                published.row_version
            ),
            (replay.id, replay.publication_no, replay.row_version)
        );
        accepted.push(published);
    }
    assert_ne!(accepted[0].id, accepted[1].id);
    assert_eq!(
        accepted[1].publication_no,
        accepted[0].publication_no.map(|number| number + 1)
    );
    assert_eq!(
        timetable_version_service::resolve_version_id_for_date(&pool, context.term_id, date)
            .await
            .unwrap(),
        accepted[1].id
    );
    assert_eq!(
        rows(&pool, "academic_delivery_versions").await,
        opening_before
    );
    let historical = accepted_facts(&pool).await;
    let revision = target_change(&pool, &context, date, 4).await;
    let newer = publish(&pool, context.teacher_id, revision.id).await;
    assert_ne!(
        newer.target_delivery_version_id,
        accepted[1].delivery_version_id
    );
    assert_original_facts(&pool, &historical).await;
    let old = timetable_version_service::get_version(&pool, accepted[1].id, date)
        .await
        .unwrap();
    assert_eq!(old.delivery_version_id, accepted[1].delivery_version_id);
}

#[tokio::test]
async fn actual_dml_runtime_publishes_teacher_and_offering_effectivity_with_the_real_adapters() {
    let owner = prepare_concurrent_delivery_runtime_fixture("publication151_runtime_happy").await;
    let context = operational_change_runtime_context(&owner).await;
    let original = accepted_facts(&owner).await;
    let (schema, database): (String, String) =
        sqlx::query_as("SELECT current_schema(),current_database()")
            .fetch_one(&owner)
            .await
            .unwrap();
    for identifier in [&schema, &database] {
        assert!(identifier
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'));
    }
    let role = format!("publication151_happy_{}", Uuid::now_v7().simple());
    // These grants are confined to the runner-owned disposable database. The
    // production-equivalent runtime has DML and function access, with no ownership,
    // schema creation, TEMP or privilege to alter the migration ledger.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        r#"CREATE ROLE "{role}" NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;
           REVOKE ALL ON DATABASE "{database}" FROM PUBLIC;
           GRANT CONNECT ON DATABASE "{database}" TO "{role}";
           GRANT USAGE ON SCHEMA "{schema}" TO "{role}";
           GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA "{schema}" TO "{role}";
           REVOKE INSERT,UPDATE,DELETE ON "{schema}"._sqlx_migrations FROM "{role}";
           GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA "{schema}" TO "{role}";"#,
    )))
    .execute(&owner)
    .await
    .unwrap();
    let runtime_role = role.clone();
    let runtime_schema = schema.clone();
    let runtime = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _| {
            let role = runtime_role.clone();
            let schema = runtime_schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(format!("\"{schema}\",public"))
                    .execute(&mut *connection)
                    .await?;
                sqlx::query(sqlx::AssertSqlSafe(format!("SET ROLE \"{role}\"")))
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(owner.connect_options().as_ref().clone())
        .await
        .unwrap();
    let (identity,is_owner,superuser,temporary,create_schema):(String,bool,bool,bool,bool)=sqlx::query_as(
        "SELECT current_user::text,(SELECT relowner=current_user::regrole FROM pg_class WHERE oid='academic_delivery_versions'::regclass),\
         (SELECT rolsuper FROM pg_roles WHERE rolname=current_user),has_database_privilege(current_user,current_database(),'TEMP'),has_schema_privilege(current_user,current_schema(),'CREATE')",
    ).fetch_one(&runtime).await.unwrap();
    assert_eq!(identity, role);
    assert!(!is_owner && !superuser && !temporary && !create_schema);
    let (accepted, group, _, _, replacement) = future_replacement(
        &runtime,
        &context,
        runtime_operation_date() + Duration::days(5),
    )
    .await;
    assert_eq!(accepted.status, AcademicTermChangeSetStatus::Published);
    let (episodes,cutoffs,offerings):(i64,i64,i64)=sqlx::query_as(
        "SELECT (SELECT count(*) FROM learning_group_teachers WHERE started_by_change_set_id=$1 AND learning_group_id=$2 AND teacher_id=$3),\
         (SELECT count(*) FROM academic_delivery_teacher_cutoffs WHERE change_set_id=$1),\
         (SELECT count(*) FROM academic_delivery_offering_effectivity WHERE change_set_id=$1)",
    ).bind(accepted.id).bind(group).bind(replacement).fetch_one(&runtime).await.unwrap();
    assert_eq!(episodes, 1);
    assert!(cutoffs > 0 && offerings > 0);
    assert_original_facts(&owner, &original).await;
    runtime.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP OWNED BY \"{role}\"; DROP ROLE \"{role}\";"
    )))
    .execute(&owner)
    .await
    .unwrap();
}

#[tokio::test]
async fn matching_timetable_publications_queued_on_the_same_runtime_replay_one_accepted_receipt() {
    use school_academic_timetable::{
        collaboration::TimetableMutationContext,
        models::timetable_publication::{
            PreviewTimetablePublicationRequest, PublishTimetableVersionRequest,
        },
        services::timetable_lifecycle,
    };
    let pool =
        prepare_concurrent_delivery_runtime_fixture("publication151_shared_runtime_replay").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, draft.id).await;
    let current = timetable_version_service::get_version(&pool, draft.id, runtime_operation_date())
        .await
        .unwrap();
    let preview = timetable_lifecycle::preview(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        draft.id,
        PreviewTimetablePublicationRequest {
            row_version: current.row_version,
            effective_from: runtime_operation_date() + Duration::days(5),
        },
    )
    .await
    .unwrap();
    assert!(preview.can_publish, "{:?}", preview.findings);
    let request = PublishTimetableVersionRequest {
        row_version: preview.row_version,
        effective_from: preview.effective_from,
        preview_hash: preview.preview_hash,
        idempotency_key: Uuid::now_v7(),
    };
    let mutation = TimetableMutationContext::for_tests(&pool, context.teacher_id);
    let room = mutation
        .runtime
        .load_room(
            mutation.tenant_id,
            &pool,
            draft.id,
            mutation.staff_directory,
        )
        .await
        .unwrap();
    let gate = room.mutation_gate().await;
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let pool = pool.clone();
        let mutation = mutation.clone();
        let request = request.clone();
        let id = draft.id;
        tasks.push(tokio::spawn(async move {
            timetable_lifecycle::publish(
                &crate::adapters::publication_history::PUBLICATION_HISTORY,
                &pool,
                &mutation,
                id,
                request,
            )
            .await
        }));
    }
    // The runtime owns one Arc and this test owns one. Two further references
    // prove both accepted calls loaded the exact room before its first invalidation.
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while std::sync::Arc::strong_count(&room) < 4 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("matching requests must queue on the same process-local draft gate");
    drop(gate);
    let first = tasks.remove(0).await.unwrap().unwrap();
    let second = tasks.remove(0).await.unwrap().unwrap();
    assert_eq!(
        (first.id, first.publication_no, first.row_version),
        (second.id, second.publication_no, second.row_version)
    );
    let audits:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_timetable_version.published'")
        .bind(first.id).fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 1);
    let before = publication_state(&pool).await;
    let mismatched = timetable_lifecycle::publish(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        &mutation,
        draft.id,
        PublishTimetableVersionRequest {
            effective_from: request.effective_from + Duration::days(1),
            ..request
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(mismatched, AppError::Conflict(_)));
    assert_eq!(publication_state(&pool).await, before);
}

#[tokio::test]
async fn delivery_date_only_earlier_correction_publishes_unchanged_graph_and_same_or_later_date_does_not(
) {
    let pool =
        prepare_concurrent_delivery_runtime_fixture("publication151_delivery_date_only").await;
    let context = operational_change_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let future_date = today + Duration::days(10);
    let earlier = today + Duration::days(5);
    let changed = target_change(&pool, &context, future_date, 4).await;
    let original = publish(&pool, context.teacher_id, changed.id).await;
    let original_graph = snapshot(&pool, &original).await;
    let teacher_ids = original_graph
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .flat_map(|group| &group.teachers)
        .map(|teacher| teacher.assignment_id)
        .collect::<Vec<_>>();
    let ineligible:i64=sqlx::query_scalar("SELECT count(*) FROM academic_delivery_effective_teacher_episodes WHERE id=ANY($1) AND (starts_on>$2 OR effective_ends_on IS NOT NULL)")
        .bind(teacher_ids).bind(earlier).fetch_one(&pool).await.unwrap();
    assert_eq!(
        ineligible, 0,
        "the date correction must not depend on a carried teacher ID changing the graph"
    );
    let accepted = accepted_facts(&pool).await;
    let draft = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        future_date,
        "publication151-date-only-delivery",
    )
    .await;
    assert!(draft.items.is_empty());
    assert_eq!(
        serde_json::to_value(snapshot(&pool, &draft).await).unwrap(),
        serde_json::to_value(&original_graph).unwrap()
    );
    let before = publication_state(&pool).await;
    for date in [future_date, future_date + Duration::days(1)] {
        let preview = change_sets::preview_change_set_at(&pool, draft.id, Some(date))
            .await
            .unwrap();
        assert!(preview.findings.iter().any(|finding| finding.code
            == AcademicChangeFindingCode::ChangeSetNoItems
            && finding.severity == AcademicChangeFindingSeverity::Blocking));
        let denied = change_sets::publish_change_set(
            &pool,
            context.teacher_id,
            draft.id,
            request(&preview, Uuid::now_v7()),
        )
        .await
        .unwrap_err();
        assert!(matches!(denied, AppError::ValidationError(_)));
        assert_eq!(publication_state(&pool).await, before);
    }
    let preview = change_sets::preview_change_set_at(&pool, draft.id, Some(earlier))
        .await
        .unwrap();
    assert_ready(&preview);
    assert_eq!(
        publication_state(&pool).await,
        before,
        "candidate date preview must be read-only"
    );
    let correction = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        draft.id,
        request(&preview, Uuid::now_v7()),
    )
    .await
    .unwrap();
    assert_ne!(
        correction.target_delivery_version_id,
        original.target_delivery_version_id
    );
    assert_eq!(
        correction.publication_no,
        original.publication_no.map(|number| number + 1)
    );
    assert_eq!(correction.effective_from, Some(earlier));
    assert_eq!(
        serde_json::to_value(snapshot(&pool, &correction).await).unwrap(),
        serde_json::to_value(original_graph).unwrap()
    );
    for date in [earlier, future_date, future_date + Duration::days(30)] {
        assert_eq!(
            applicable(&pool, context.term_id, date).await,
            correction.target_delivery_version_id
        );
    }
    let superseded = delivery_versions::get_version(&pool, original.target_delivery_version_id)
        .await
        .unwrap();
    assert!(superseded.is_superseded);
    assert_eq!(
        superseded.effective_until,
        Some(earlier - Duration::days(1))
    );
    assert_original_facts(&pool, &accepted).await;
}

async fn timetable_content(pool: &PgPool, version: Uuid) -> String {
    let source = timetable_version_service::get_version(pool, version, runtime_operation_date())
        .await
        .unwrap();
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 ORDER BY id",
    )
    .bind(version)
    .fetch_all(pool)
    .await
    .unwrap();
    let mut blocks = Vec::new();
    for id in ids {
        blocks.push(timetable_block_service::get_block(pool, id).await.unwrap());
    }
    school_academic_timetable::services::timetable_source::content_hash(
        source.delivery_version_id,
        &blocks,
    )
    .unwrap()
}

async fn all_placement_rows(pool: &PgPool) -> BTreeMap<&'static str, Value> {
    let mut result = BTreeMap::new();
    for table in [
        "academic_timetable_blocks",
        "academic_timetable_block_groups",
        "academic_timetable_block_group_instructors",
        "academic_timetable_block_homerooms",
        "academic_timetable_block_teachers",
        "academic_timetable_block_group_sync",
    ] {
        result.insert(table, rows(pool, table).await);
    }
    result
}

#[tokio::test]
async fn fresh_clone_and_existing_resume_use_latest_number_while_retaining_the_published_origin() {
    use school_academic_timetable::collaboration::TimetableMutationContext;
    let pool = prepare_concurrent_delivery_runtime_fixture("timetable_latest_clone_resume").await;
    let context = operational_change_runtime_context(&pool).await;
    let source_id:Uuid=sqlx::query_scalar("SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY publication_no DESC LIMIT 1").bind(context.term_id).fetch_one(&pool).await.unwrap();
    let source = timetable_version_service::get_version(&pool, source_id, runtime_operation_date())
        .await
        .unwrap();
    let original: Value =
        sqlx::query_scalar("SELECT to_jsonb(v) FROM academic_timetable_versions v WHERE id=$1")
            .bind(source_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (first, _, _, _, _) = future_replacement(
        &pool,
        &context,
        runtime_operation_date() + Duration::days(7),
    )
    .await;
    let mutation = TimetableMutationContext::for_tests(&pool, context.teacher_id);
    let draft = timetable_version_service::clone_draft(
        &pool,
        &mutation,
        source.id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(draft.delivery_version_id, first.target_delivery_version_id);
    assert_ne!(draft.delivery_version_id, source.delivery_version_id);
    assert_eq!(draft.source_version_id, Some(source.id));
    let children = all_placement_rows(&pool).await;
    let first_source = delivery_versions::get_version(&pool, first.target_delivery_version_id)
        .await
        .unwrap();
    let second_draft = target_change(
        &pool,
        &context,
        runtime_operation_date() + Duration::days(3),
        first_source.snapshot.offerings[0].weekly_period_target + 1,
    )
    .await;
    let second = publish(&pool, context.teacher_id, second_draft.id).await;
    let second_source = delivery_versions::get_version(&pool, second.target_delivery_version_id)
        .await
        .unwrap();
    assert!(second_source.publication_no > first_source.publication_no);
    assert!(second_source.effective_from < first_source.effective_from);
    let resumed = timetable_version_service::clone_draft(
        &pool,
        &mutation,
        source.id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: Some(draft.id),
            draft_row_version: Some(draft.row_version),
        },
    )
    .await
    .unwrap();
    assert_eq!(resumed.id, draft.id);
    assert_eq!(
        resumed.delivery_version_id,
        second.target_delivery_version_id
    );
    assert_eq!(resumed.source_version_id, Some(source.id));
    assert_eq!(resumed.row_version, draft.row_version + 1);
    assert_eq!(children, all_placement_rows(&pool).await);
    assert_eq!(
        original,
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(v) FROM academic_timetable_versions v WHERE id=$1"
        )
        .bind(source_id)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    let stale = timetable_version_service::clone_draft(
        &pool,
        &mutation,
        source.id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: Some(draft.id),
            draft_row_version: Some(draft.row_version),
        },
    )
    .await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));
    assert_eq!(children, all_placement_rows(&pool).await);
}

#[tokio::test]
async fn initial_draft_resume_aligns_an_existing_target_through_its_existing_permit() {
    use school_academic_timetable::{
        collaboration::TimetableMutationContext,
        models::timetable_version::CreateTimetableVersionRequest,
    };
    let pool = prepare_delivery_runtime_fixture("timetable_latest_initial_resume").await;
    let context = planning_runtime_context(&pool).await;
    let opening = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        runtime_operation_date(),
        "initial-latest-basis",
    )
    .await;
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(opening.target_delivery_version_id),
        course_request(&context),
    )
    .await
    .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "Initial latest source".into(),
            description: None,
            capacity: None,
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    let group = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();
    groups::replace_teachers(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let accepted = publish(&pool, context.teacher_id, opening.id).await;
    let mutation = TimetableMutationContext::for_tests(&pool, context.teacher_id);
    let request = CreateTimetableVersionRequest {
        academic_term_id: context.term_id,
    };
    let draft = timetable_version_service::create_initial_draft(&pool, &mutation, request.clone())
        .await
        .unwrap();
    assert_eq!(
        draft.delivery_version_id,
        accepted.target_delivery_version_id
    );
    assert_eq!(draft.source_version_id, None);
    let children = all_placement_rows(&pool).await;
    let source = delivery_versions::get_version(&pool, accepted.target_delivery_version_id)
        .await
        .unwrap();
    let next = target_change(
        &pool,
        &context,
        runtime_operation_date() + Duration::days(2),
        source.snapshot.offerings[0].weekly_period_target + 1,
    )
    .await;
    let next = publish(&pool, context.teacher_id, next.id).await;
    let resumed =
        timetable_version_service::create_initial_draft(&pool, &mutation, request.clone())
            .await
            .unwrap();
    assert_eq!(resumed.id, draft.id);
    assert_eq!(resumed.row_version, draft.row_version + 1);
    assert_eq!(resumed.delivery_version_id, next.target_delivery_version_id);
    assert_eq!(resumed.source_version_id, None);
    assert_eq!(children, all_placement_rows(&pool).await);
    let same = timetable_version_service::create_initial_draft(&pool, &mutation, request)
        .await
        .unwrap();
    assert_eq!(same.row_version, resumed.row_version);
    assert_eq!(same.id, resumed.id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_timetable_versions WHERE academic_term_id=$1"
        )
        .bind(context.term_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn latest_source_alignment_preserves_every_placement_and_blocks_a_stale_source_after_admission(
) {
    use school_academic_timetable::{
        collaboration::{DraftSignal, TimetableMutationContext},
        models::{
            timetable_block::*, timetable_publication::*,
            timetable_version::UpdateTimetableDeliverySourceRequest,
        },
        services::timetable_lifecycle,
    };
    let pool = prepare_concurrent_delivery_runtime_fixture("timetable_latest_alignment").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, draft.id).await;
    let draft = timetable_version_service::get_version(&pool, draft.id, runtime_operation_date())
        .await
        .unwrap();
    let mutation = TimetableMutationContext::for_tests(&pool, context.teacher_id);
    let room = mutation
        .runtime
        .load_room(
            mutation.tenant_id,
            &pool,
            draft.id,
            mutation.staff_directory,
        )
        .await
        .unwrap();
    let mut signals = room.subscribe();
    let before = all_placement_rows(&pool).await;
    let raw_draft: Value =
        sqlx::query_scalar("SELECT to_jsonb(v) FROM academic_timetable_versions v WHERE id=$1")
            .bind(draft.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let no_op = timetable_lifecycle::update_source(
        &pool,
        &mutation,
        draft.id,
        UpdateTimetableDeliverySourceRequest {
            row_version: draft.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(no_op.row_version, draft.row_version);
    assert_eq!(
        raw_draft,
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(v) FROM academic_timetable_versions v WHERE id=$1"
        )
        .bind(draft.id)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    assert!(std::sync::Arc::ptr_eq(
        &room,
        &mutation
            .runtime
            .load_room(
                mutation.tenant_id,
                &pool,
                draft.id,
                mutation.staff_directory
            )
            .await
            .unwrap()
    ));
    while let Ok(signal) = signals.try_recv() {
        assert!(!matches!(signal, DraftSignal::Invalidated));
    }

    let date = runtime_operation_date() + Duration::days(5);
    let (opening, group, _, old_teacher, _) = future_replacement(&pool, &context, date).await;
    assert_ne!(
        opening.target_delivery_version_id,
        draft.delivery_version_id
    );
    let old_published = sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_timetable_versions v WHERE status='published'").fetch_one(&pool).await.unwrap();
    let untouched = timetable_version_service::get_version(&pool, draft.id, date)
        .await
        .unwrap();
    assert_eq!(
        untouched.delivery_version_id, draft.delivery_version_id,
        "read-only retrieval cannot mutate a dormant source"
    );
    let stale = timetable_lifecycle::preview(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        draft.id,
        PreviewTimetablePublicationRequest {
            row_version: draft.row_version,
            effective_from: date,
        },
    )
    .await
    .unwrap();
    assert!(!stale.can_publish);
    assert!(stale
        .findings
        .iter()
        .any(|finding| finding.code == TimetablePublicationFindingCode::DeliverySourceOutdated));
    let block_id:Uuid=sqlx::query_scalar("SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND is_active ORDER BY id LIMIT 1").bind(draft.id).fetch_one(&pool).await.unwrap();
    let block = timetable_block_service::get_block(&pool, block_id)
        .await
        .unwrap();
    // Keep the warm room from before publication: no HTTP upstream signal has
    // refreshed it. The transaction guard must catch the newly accepted source.
    let denied = timetable_block_service::update_block(&pool,&mutation,block.id,serde_json::from_value(serde_json::json!({"timetableVersionId":draft.id,"rowVersion":block.row_version,"note":"must not save against an older source"})).unwrap()).await;
    assert!(matches!(denied, Err(AppError::Conflict(_))));
    fn latest_conflict<T>(result: Result<T, AppError>) {
        assert!(
            matches!(result,Err(AppError::Conflict(ref message)) if message.contains("รุ่นเปิดสอนล่าสุด")),
            "every new content owner must reject the obsolete source before changing content"
        );
    }
    use school_academic_timetable::{
        models::timetable::{ApplyTemplateRequest, ClearTimetableRequest, CreateTemplateRequest},
        services::timetable_template_service,
    };
    let template = timetable_template_service::create_template(
        &pool,
        context.teacher_id,
        CreateTemplateRequest {
            name: "Stale source template guard".into(),
            description: None,
        },
    )
    .await
    .unwrap();
    latest_conflict(
        timetable_template_service::apply_template(
            &pool,
            &mutation,
            template.id,
            ApplyTemplateRequest {
                timetable_version_id: draft.id,
                academic_term_id: context.term_id,
            },
        )
        .await,
    );
    latest_conflict(
        timetable_template_service::clear_timetable(
            &pool,
            &mutation,
            ClearTimetableRequest {
                timetable_version_id: draft.id,
                academic_term_id: context.term_id,
                entry_types: None,
            },
        )
        .await,
    );
    latest_conflict(
        timetable_block_service::retry_sync(
            &pool,
            &mutation,
            block.id,
            RetryTimetableBlockSyncRequest {
                timetable_version_id: draft.id,
                block_row_version: block.row_version,
                learning_group_ids: vec![],
            },
        )
        .await,
    );
    latest_conflict(
        school_academic_timetable::services::teacher_handoff::apply(
            &pool,
            &mutation,
            opening.id,
            ApplyTeacherHandoffRequest {
                timetable_version_id: draft.id,
                change_set_row_version: opening.row_version,
                target_timetable_version_row_version: draft.row_version,
                teacher_change_item_id: opening.items.iter().find_map(|item|match item {
                    school_academic_delivery::models::AcademicTermChangeItem::StopGroupTeacher {id,..} => Some(*id),
                    _ => None,
                }).unwrap(),
                entries: vec![TeacherHandoffEntryVersion {
                    entry_id: block.id,
                    row_version: block.row_version,
                }],
                mode: TeacherHandoffMode::Manual,
                instructor_ids: vec![context.teacher_id],
                preview_hash: "a".repeat(64),
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await,
    );
    let placement = timetable_block_service::preview_placement(
        &pool,
        TimetableBlockPlacementPreviewRequest {
            timetable_version_id: draft.id,
            academic_term_id: context.term_id,
            source: TimetableBlockPlacementSource::ExistingBlock {
                block_id: block.id,
                row_version: block.row_version,
            },
            candidate: TimetableBlockPlacementCandidate {
                block_kind: block.block_kind,
                learning_group_id: None,
                learning_offering_id: block.learning_offering_id,
                room_id: None,
                homeroom_ids: vec![],
                teacher_ids: vec![],
                instructor_ids: vec![],
            },
            target_day_of_week: block.day_of_week.clone(),
            target_bell_schedule_period_id: block.bell_schedule_period_id,
            expected_target_block_id: None,
            expected_target_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(placement.state, TimetableBlockPlacementState::Blocked);
    assert!(placement
        .conflicts
        .iter()
        .any(|finding| finding.code == "TIMETABLE_DELIVERY_SOURCE_OUTDATED"));
    let publish_denied = timetable_lifecycle::publish(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        &mutation,
        draft.id,
        PublishTimetableVersionRequest {
            row_version: draft.row_version,
            effective_from: date,
            preview_hash: stale.preview_hash,
            idempotency_key: Uuid::now_v7(),
        },
    )
    .await;
    assert!(matches!(publish_denied, Err(AppError::ValidationError(_))));
    assert_eq!(before, all_placement_rows(&pool).await);
    let aligned = timetable_lifecycle::update_source(
        &pool,
        &mutation,
        draft.id,
        UpdateTimetableDeliverySourceRequest {
            row_version: draft.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        aligned.delivery_version_id,
        opening.target_delivery_version_id
    );
    assert_eq!(aligned.source_version_id, draft.source_version_id);
    assert_eq!(aligned.row_version, draft.row_version + 1);
    assert_eq!(
        before,
        all_placement_rows(&pool).await,
        "all placement identities, teachers, provenance and child revisions survive alignment"
    );
    assert_eq!(old_published,sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_timetable_versions v WHERE status='published'").fetch_one(&pool).await.unwrap());
    let fresh = mutation
        .runtime
        .load_room(
            mutation.tenant_id,
            &pool,
            draft.id,
            mutation.staff_directory,
        )
        .await
        .unwrap();
    assert!(!std::sync::Arc::ptr_eq(&room, &fresh));
    let mut invalidated = false;
    while let Ok(signal) = signals.try_recv() {
        invalidated |= matches!(signal, DraftSignal::Invalidated);
    }
    assert!(invalidated);
    let preview = timetable_lifecycle::preview(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        draft.id,
        PreviewTimetablePublicationRequest {
            row_version: aligned.row_version,
            effective_from: date,
        },
    )
    .await
    .unwrap();
    assert!(!preview
        .findings
        .iter()
        .any(|finding| finding.code == TimetablePublicationFindingCode::DeliverySourceOutdated));
    assert!(
        preview
            .source_issues
            .iter()
            .any(|issue| issue.learning_group_id == Some(group)
                && issue.teacher_id == Some(old_teacher)),
        "alignment exposes real teacher mismatches without replacing the saved instructor"
    );
    let early = timetable_lifecycle::preview(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        draft.id,
        PreviewTimetablePublicationRequest {
            row_version: aligned.row_version,
            effective_from: date - Duration::days(1),
        },
    )
    .await
    .unwrap();
    assert!(early
        .findings
        .iter()
        .any(|finding| finding.code == TimetablePublicationFindingCode::DeliveryDateMismatch));
    assert!(!early.can_publish);
    let no_op = timetable_lifecycle::update_source(
        &pool,
        &mutation,
        draft.id,
        UpdateTimetableDeliverySourceRequest {
            row_version: aligned.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(no_op.row_version, aligned.row_version);
    assert!(std::sync::Arc::ptr_eq(
        &fresh,
        &mutation
            .runtime
            .load_room(
                mutation.tenant_id,
                &pool,
                draft.id,
                mutation.staff_directory
            )
            .await
            .unwrap()
    ));
    let evidence: (i64,Uuid,Uuid) = sqlx::query_as("SELECT count(*),min((payload->>'previousDeliveryVersionId')::text)::uuid,min((payload->>'deliveryVersionId')::text)::uuid FROM academic_audit_events WHERE event_code='academic_timetable_version.source_updated' AND entity_id=$1").bind(draft.id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        evidence,
        (
            1,
            draft.delivery_version_id,
            opening.target_delivery_version_id
        )
    );
}

#[tokio::test]
async fn timetable_date_only_earlier_correction_keeps_the_exact_delivery_pin_and_normalized_placements(
) {
    use school_academic_timetable::{
        collaboration::TimetableMutationContext,
        models::timetable_publication::{
            PreviewTimetablePublicationRequest, PublishTimetableVersionRequest,
            TimetablePublicationFindingCode,
        },
        services::timetable_lifecycle,
    };
    let pool =
        prepare_concurrent_delivery_runtime_fixture("publication151_timetable_date_only").await;
    let context = operational_change_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let future_date = today + Duration::days(10);
    let earlier = today + Duration::days(5);
    let initial = independent_table_draft(&pool, &context).await;
    fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, initial.id).await;
    let initial = timetable_version_service::get_version(&pool, initial.id, today)
        .await
        .unwrap();
    let preview = timetable_lifecycle::preview(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        initial.id,
        PreviewTimetablePublicationRequest {
            row_version: initial.row_version,
            effective_from: future_date,
        },
    )
    .await
    .unwrap();
    assert!(preview.can_publish, "{:?}", preview.findings);
    let original = timetable_lifecycle::publish(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        &TimetableMutationContext::for_tests(&pool, context.teacher_id),
        initial.id,
        PublishTimetableVersionRequest {
            row_version: preview.row_version,
            effective_from: future_date,
            preview_hash: preview.preview_hash,
            idempotency_key: Uuid::now_v7(),
        },
    )
    .await
    .unwrap();
    let accepted = accepted_facts(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    assert_eq!(draft.source_version_id, Some(original.id));
    assert_eq!(draft.delivery_version_id, original.delivery_version_id);
    assert_eq!(
        timetable_content(&pool, draft.id).await,
        timetable_content(&pool, original.id).await,
        "only the chosen date changes; placement identities are normalized by the owning provider"
    );
    let before = publication_state(&pool).await;
    let mutation = TimetableMutationContext::for_tests(&pool, context.teacher_id);
    for date in [future_date, future_date + Duration::days(1)] {
        let preview = timetable_lifecycle::preview(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            &pool,
            draft.id,
            PreviewTimetablePublicationRequest {
                row_version: draft.row_version,
                effective_from: date,
            },
        )
        .await
        .unwrap();
        assert!(!preview.can_publish);
        assert!(preview
            .findings
            .iter()
            .any(|finding| finding.code == TimetablePublicationFindingCode::NoContentChange));
        let denied = timetable_lifecycle::publish(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            &pool,
            &mutation,
            draft.id,
            PublishTimetableVersionRequest {
                row_version: preview.row_version,
                effective_from: date,
                preview_hash: preview.preview_hash,
                idempotency_key: Uuid::now_v7(),
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(denied, AppError::ValidationError(_)));
        assert_eq!(publication_state(&pool).await, before);
    }
    let preview = timetable_lifecycle::preview(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        draft.id,
        PreviewTimetablePublicationRequest {
            row_version: draft.row_version,
            effective_from: earlier,
        },
    )
    .await
    .unwrap();
    assert!(preview.can_publish, "{:?}", preview.findings);
    assert_eq!(publication_state(&pool).await, before);
    let correction = timetable_lifecycle::publish(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &pool,
        &mutation,
        draft.id,
        PublishTimetableVersionRequest {
            row_version: preview.row_version,
            effective_from: earlier,
            preview_hash: preview.preview_hash,
            idempotency_key: Uuid::now_v7(),
        },
    )
    .await
    .unwrap();
    assert_ne!(correction.id, original.id);
    assert_eq!(
        correction.publication_no,
        original.publication_no.map(|number| number + 1)
    );
    assert_eq!(correction.delivery_version_id, original.delivery_version_id);
    assert_eq!(
        timetable_content(&pool, correction.id).await,
        timetable_content(&pool, original.id).await
    );
    for date in [earlier, future_date, future_date + Duration::days(30)] {
        assert_eq!(
            timetable_version_service::resolve_version_id_for_date(&pool, context.term_id, date)
                .await
                .unwrap(),
            correction.id
        );
    }
    let superseded = timetable_version_service::get_version(&pool, original.id, today)
        .await
        .unwrap();
    assert_eq!(
        superseded.effective_until,
        Some(earlier - Duration::days(1))
    );
    assert_original_facts(&pool, &accepted).await;
}
