use super::*;
use school_academic_delivery::{
    models::{versions::DeliveryVersion, RemoveDeliveryDraftResourceRequest},
    services::{audiences, versions},
};
use serde_json::Value;

pub(super) async fn version(pool: &PgPool, revision: Uuid) -> DeliveryVersion {
    let revision = change_sets::get_change_set(pool, revision).await.unwrap();
    versions::get_version(pool, revision.target_delivery_version_id)
        .await
        .unwrap()
}
pub(super) async fn removal(pool: &PgPool, revision: Uuid) -> RemoveDeliveryDraftResourceRequest {
    let draft = version(pool, revision).await;
    RemoveDeliveryDraftResourceRequest {
        change_set_row_version: change_sets::get_change_set(pool, revision)
            .await
            .unwrap()
            .row_version,
        target_delivery_version_row_version: draft.row_version,
    }
}
pub(super) async fn extra_room(pool: &PgPool, c: &RuntimeContext, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,capacity,is_active) VALUES($1,$2,$2,$3,$4,$5,40,true)")
        .bind(id).bind(code).bind(c.year_id).bind(c.grade_level_id).bind(c.study_program_id).execute(pool).await.unwrap();
    id
}
pub(super) async fn group(
    pool: &PgPool,
    c: &RuntimeContext,
    offering: Uuid,
    name: &str,
    rooms: Vec<Uuid>,
) -> Uuid {
    let group = groups::create(
        pool,
        c.teacher_id,
        offering,
        CreateLearningGroupRequest {
            name: name.into(),
            description: Some("preserved settings".into()),
            capacity: Some(37),
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    let group = groups::replace_homerooms(
        pool,
        c.teacher_id,
        group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: group.row_version,
            homeroom_ids: rooms,
        },
    )
    .await
    .unwrap();
    groups::replace_teachers(
        pool,
        c.teacher_id,
        group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: c.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap()
    .id
}
pub(super) async fn publish(pool: &PgPool, actor: Uuid, revision: Uuid) {
    let preview = publication_preview(pool, revision).await.unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|f| f.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        pool,
        actor,
        revision,
        PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: preview
                .findings
                .iter()
                .filter(|f| f.severity == AcademicChangeFindingSeverity::Warning)
                .map(|f| f.code)
                .collect(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
}
async fn registry_facts(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('offerings',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_offerings t),'targets',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_offering_targets t),'groups',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_groups t),'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t),'students',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_students t),'plans',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM course_assessment_plans t),'scores',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_scores t),'courseResults',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_course_results t),'activityResults',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_activity_results t),'timetable',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_timetable_versions t))").fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn delivery_room_scope_partial_combined_groups_freeze_audience_and_inherit() {
    let pool = prepare_delivery_runtime_fixture("room_scope_combined").await;
    let c = planning_runtime_context(&pool).await;
    let old_a = extra_room(&pool, &c, "SCOPE-OLD-A").await;
    let old_b = extra_room(&pool, &c, "SCOPE-OLD-B").await;
    let rev = create_runtime_change_set(&pool, c.teacher_id, c.term_id, 1, "scope-combined").await;
    let CreateLearningOfferingRequest::Course(mut request) = course_request(&c) else {
        panic!("course")
    };
    request.targets = vec![OfferingTargetInput {
        target_kind: OfferingTargetKind::GradeProgram,
        homeroom_id: None,
        grade_level_id: c.grade_level_id,
        study_program_id: c.study_program_id,
    }];
    let offering = offerings::create_for_delivery(
        &pool,
        c.teacher_id,
        Some(rev.target_delivery_version_id),
        CreateLearningOfferingRequest::Course(request),
    )
    .await
    .unwrap();
    let combined = group(
        &pool,
        &c,
        offering.id,
        "Keep combined identity",
        vec![old_a, old_b, c.homeroom_id],
    )
    .await;
    let omitted_a = group(&pool, &c, offering.id, "old A only", vec![old_a]).await;
    group(&pool, &c, offering.id, "old B only", vec![old_b]).await;
    publish(&pool, c.teacher_id, rev.id).await;
    seed_retained_grade(&pool, &c, combined).await;
    let rev =
        create_runtime_change_set(&pool, c.teacher_id, c.term_id, 2, "scope-combined-selected")
            .await;
    let sibling =
        create_runtime_change_set(&pool, c.teacher_id, c.term_id, 2, "scope-combined-sibling")
            .await;
    let sibling_before = serde_json::to_value(version(&pool, sibling.id).await).unwrap();
    let teacher:Uuid=sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' AND status='active' AND id<>$1 ORDER BY id LIMIT 1").bind(c.teacher_id).fetch_one(&pool).await.unwrap();
    for group_id in [omitted_a, combined] {
        let current = change_sets::get_change_set(&pool, rev.id).await.unwrap();
        change_sets::upsert_change_item(
            &pool,
            c.teacher_id,
            rev.id,
            UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
                change_set_row_version: current.row_version,
                item_row_version: None,
                learning_group_id: group_id,
                teacher_id: teacher,
                teacher_role: LearningTeacherRole::Secondary,
            },
        )
        .await
        .unwrap();
    }
    // A sibling snapshot pins the group; selected scope must never rewrite registry coverage.
    let facts = registry_facts(&pool).await;
    for room in [old_a, old_b] {
        change_sets::remove_draft_offering_homeroom(
            &pool,
            c.teacher_id,
            rev.id,
            offering.id,
            room,
            removal(&pool, rev.id).await,
        )
        .await
        .unwrap();
    }
    let draft = version(&pool, rev.id).await;
    let selected = draft
        .snapshot
        .offerings
        .iter()
        .find(|o| o.id == offering.id)
        .unwrap();
    assert_eq!(selected.homeroom_ids, vec![c.homeroom_id]);
    assert!(selected
        .targets
        .iter()
        .all(|t| t.target_kind == OfferingTargetKind::Homeroom
            && t.homeroom_id == Some(c.homeroom_id)));
    assert_eq!(selected.groups.len(), 1);
    assert_eq!(selected.groups[0].id, combined);
    assert_eq!(selected.groups[0].homeroom_ids, vec![c.homeroom_id]);
    assert_eq!(selected.groups[0].name, "Keep combined identity");
    assert_eq!(selected.groups[0].capacity, Some(37));
    assert!(selected.groups[0]
        .teachers
        .iter()
        .any(|t| t.teacher_id == c.teacher_id));
    let commands:Vec<Uuid>=sqlx::query_scalar("SELECT learning_group_id FROM academic_term_change_items WHERE change_set_id=$1 AND learning_group_id IS NOT NULL ORDER BY learning_group_id").bind(rev.id).fetch_all(&pool).await.unwrap();
    assert_eq!(
        commands,
        vec![combined],
        "fully omitted group command cleared; retained partial group's command survives"
    );
    assert_eq!(facts, registry_facts(&pool).await);
    assert_eq!(
        sibling_before,
        serde_json::to_value(version(&pool, sibling.id).await).unwrap()
    );
    let late = extra_room(&pool, &c, "SCOPE-LATE-OLD-GRADE").await;
    let projected: Value = sqlx::query_scalar("SELECT academic_delivery_revision_snapshot($1)")
        .bind(rev.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let projected: school_academic_delivery::models::versions::DeliverySnapshot =
        serde_json::from_value(projected).unwrap();
    assert!(!projected
        .offerings
        .iter()
        .find(|o| o.id == offering.id)
        .unwrap()
        .homeroom_ids
        .contains(&late));
    publish(&pool, c.teacher_id, rev.id).await;
    let published = serde_json::to_value(version(&pool, rev.id).await).unwrap();
    let next =
        create_runtime_change_set(&pool, c.teacher_id, c.term_id, 3, "scope-combined-next").await;
    assert_eq!(
        version(&pool, next.id)
            .await
            .snapshot
            .offerings
            .iter()
            .find(|o| o.id == offering.id)
            .unwrap()
            .homeroom_ids,
        vec![c.homeroom_id]
    );
    assert_eq!(
        published,
        serde_json::to_value(version(&pool, rev.id).await).unwrap()
    );
}

#[tokio::test]
async fn delivery_room_scope_shared_group_delta_and_empty_intent_survive_next_draft() {
    let pool = prepare_delivery_runtime_fixture("room_scope_shared_empty").await;
    let c = planning_runtime_context(&pool).await;
    let rev = create_runtime_change_set(&pool, c.teacher_id, c.term_id, 1, "scope-shared").await;
    let offering = offerings::create_for_delivery(
        &pool,
        c.teacher_id,
        Some(rev.target_delivery_version_id),
        course_request(&c),
    )
    .await
    .unwrap();
    let a = group(&pool, &c, offering.id, "split A", vec![c.homeroom_id]).await;
    let b = group(&pool, &c, offering.id, "split B", vec![c.homeroom_id]).await;
    publish(&pool, c.teacher_id, rev.id).await;
    let initial_version = rev.target_delivery_version_id;
    let initial_published: Value =
        sqlx::query_scalar("SELECT to_jsonb(v) FROM academic_delivery_versions v WHERE id=$1")
            .bind(initial_version)
            .fetch_one(&pool)
            .await
            .unwrap();
    let rev =
        create_runtime_change_set(&pool, c.teacher_id, c.term_id, 2, "scope-shared-remove").await;
    change_sets::remove_draft_group(&pool, c.teacher_id, rev.id, a, removal(&pool, rev.id).await)
        .await
        .unwrap();
    let draft = version(&pool, rev.id).await;
    let selected = draft
        .snapshot
        .offerings
        .iter()
        .find(|o| o.id == offering.id)
        .unwrap();
    assert_eq!(selected.homeroom_ids, vec![c.homeroom_id]);
    assert_eq!(
        selected.groups.iter().map(|g| g.id).collect::<Vec<_>>(),
        vec![b]
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM academic_delivery_revision_audiences WHERE change_set_id=$1 AND learning_offering_id=$2)").bind(rev.id).bind(offering.id).fetch_one(&pool).await.unwrap());
    change_sets::remove_draft_group(&pool, c.teacher_id, rev.id, b, removal(&pool, rev.id).await)
        .await
        .unwrap();
    assert!(version(&pool, rev.id)
        .await
        .snapshot
        .offerings
        .iter()
        .all(|o| o.id != offering.id));
    assert!(
        offerings::get(&pool, offering.id).await.is_ok(),
        "empty authoritative intent pins registry identity"
    );
    publish(&pool, c.teacher_id, rev.id).await;
    let mut connection = pool.acquire().await.unwrap();
    let scoped = audiences::pinned_constraint_on_connection(
        &mut connection,
        rev.target_delivery_version_id,
        offering.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        scoped.offering.is_none(),
        "explicit empty never falls back to raw targets"
    );
    drop(connection);
    let next =
        create_runtime_change_set(&pool, c.teacher_id, c.term_id, 3, "scope-shared-next").await;
    assert!(version(&pool, next.id)
        .await
        .snapshot
        .offerings
        .iter()
        .all(|o| o.id != offering.id));
    let rooms:i64=sqlx::query_scalar("SELECT count(*) FROM academic_delivery_revision_audience_rooms WHERE change_set_id=$1 AND learning_offering_id=$2").bind(next.id).bind(offering.id).fetch_one(&pool).await.unwrap();
    let parent:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_delivery_revision_audiences WHERE change_set_id=$1 AND learning_offering_id=$2)").bind(next.id).bind(offering.id).fetch_one(&pool).await.unwrap();
    assert!(parent);
    assert_eq!(rooms, 0);
    assert_eq!(initial_published,sqlx::query_scalar::<_,Value>("SELECT to_jsonb(v) FROM academic_delivery_versions v WHERE id=$1").bind(initial_version).fetch_one(&pool).await.unwrap(),"initial published graph and metadata remain immutable through removal, publication and inheritance");
}

#[tokio::test]
async fn delivery_room_scope_last_central_group_preserves_nonempty_audience() {
    let pool = prepare_delivery_runtime_fixture("room_scope_central").await;
    let c = planning_runtime_context(&pool).await;
    let rev = create_runtime_change_set(&pool, c.teacher_id, c.term_id, 1, "scope-central").await;
    let offering = super::draft_removal_tests::open_activity_with_catalog_target(
        &pool,
        &c,
        rev.target_delivery_version_id,
        ActivitySchedulingMode::Synchronized,
        false,
    )
    .await;
    let group = group(
        &pool,
        &c,
        offering.id,
        "central legacy group",
        vec![c.homeroom_id],
    )
    .await;
    change_sets::remove_draft_group(
        &pool,
        c.teacher_id,
        rev.id,
        group,
        removal(&pool, rev.id).await,
    )
    .await
    .unwrap();
    let draft = version(&pool, rev.id).await;
    let selected = draft
        .snapshot
        .offerings
        .iter()
        .find(|o| o.id == offering.id)
        .unwrap();
    assert_eq!(selected.homeroom_ids, vec![c.homeroom_id]);
    assert!(selected.groups.is_empty());
    assert!(versions::readiness(&draft.snapshot)
        .iter()
        .all(|finding| finding.learning_offering_id != offering.id));
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM academic_delivery_revision_audiences WHERE change_set_id=$1 AND learning_offering_id=$2)").bind(rev.id).bind(offering.id).fetch_one(&pool).await.unwrap());
    change_sets::remove_draft_offering_homeroom(
        &pool,
        c.teacher_id,
        rev.id,
        offering.id,
        c.homeroom_id,
        removal(&pool, rev.id).await,
    )
    .await
    .unwrap();
    assert!(version(&pool, rev.id)
        .await
        .snapshot
        .offerings
        .iter()
        .all(|o| o.id != offering.id));
}

#[tokio::test]
async fn delivery_room_scope_http_guards_and_selected_room_only() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use school_permissions::registry::codes;
    use tower::ServiceExt;
    let pool = prepare_delivery_runtime_fixture("room_scope_http").await;
    let c = planning_runtime_context(&pool).await;
    let extra = extra_room(&pool, &c, "SCOPE-HTTP-OTHER").await;
    let rev = create_runtime_change_set(&pool, c.teacher_id, c.term_id, 1, "scope-http").await;
    let CreateLearningOfferingRequest::Course(mut request) = course_request(&c) else {
        panic!("course")
    };
    request.targets.push(OfferingTargetInput {
        target_kind: OfferingTargetKind::Homeroom,
        homeroom_id: Some(extra),
        grade_level_id: c.grade_level_id,
        study_program_id: c.study_program_id,
    });
    let offering = offerings::create_for_delivery(
        &pool,
        c.teacher_id,
        Some(rev.target_delivery_version_id),
        CreateLearningOfferingRequest::Course(request),
    )
    .await
    .unwrap();
    let group = group(
        &pool,
        &c,
        offering.id,
        "HTTP combined",
        vec![c.homeroom_id, extra],
    )
    .await;
    let route = format!(
        "/api/academic/term-change-sets/{}/offerings/{}/homerooms/{}",
        rev.id, offering.id, extra
    );
    for (permissions, parent_stale, target_stale, expected) in [
        (vec![], false, false, StatusCode::FORBIDDEN),
        (
            vec![codes::LEARNING_OFFERING_MANAGE_SCHOOL],
            true,
            false,
            StatusCode::CONFLICT,
        ),
        (
            vec![codes::LEARNING_OFFERING_MANAGE_SCHOOL],
            false,
            true,
            StatusCode::CONFLICT,
        ),
        (
            vec![codes::LEARNING_OFFERING_MANAGE_SCHOOL],
            false,
            false,
            StatusCode::OK,
        ),
    ] {
        let before = serde_json::to_value(version(&pool, rev.id).await).unwrap();
        let revision = change_sets::get_change_set(&pool, rev.id).await.unwrap();
        let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        let mut input = removal(&pool, rev.id).await;
        input.change_set_row_version += i64::from(parent_stale);
        input.target_delivery_version_row_version += i64::from(target_stale);
        let response =
            super::draft_removal_tests::delivery_router(&pool, c.teacher_id, &permissions)
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri(&route)
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&serde_json::json!({"changeSetRowVersion":input.change_set_row_version,"targetDeliveryVersionRowVersion":input.target_delivery_version_row_version})).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap();
        assert_eq!(status, expected, "{}", String::from_utf8_lossy(&bytes));
        let after = version(&pool, rev.id).await;
        let audits_after: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        if status == StatusCode::OK {
            assert_eq!(
                after.row_version,
                input.target_delivery_version_row_version + 1
            );
            assert_eq!(
                change_sets::get_change_set(&pool, rev.id)
                    .await
                    .unwrap()
                    .row_version,
                revision.row_version + 1
            );
            assert_eq!(audits_after, audits + 1);
            let o = after
                .snapshot
                .offerings
                .iter()
                .find(|o| o.id == offering.id)
                .unwrap();
            assert_eq!(o.homeroom_ids, vec![c.homeroom_id]);
            assert_eq!(o.groups[0].id, group);
        } else {
            assert_eq!(before, serde_json::to_value(after).unwrap());
            assert_eq!(audits, audits_after);
        }
    }
}

// Historical migration fixtures seed the pre-144 rows directly. Current creation
// intentionally requires 144's audience owner and must not branch on schema presence.
pub(super) async fn historical_revision(
    pool: &PgPool,
    c: &RuntimeContext,
    offset: i64,
) -> super::super::models::AcademicTermChangeSet {
    let revision = Uuid::new_v4();
    let target = Uuid::new_v4();
    let base:Option<Uuid>=sqlx::query_scalar("SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1").bind(c.term_id).fetch_optional(pool).await.unwrap();
    let start: NaiveDate = sqlx::query_scalar("SELECT start_date FROM academic_terms WHERE id=$1")
        .bind(c.term_id)
        .fetch_one(pool)
        .await
        .unwrap();
    let date = start + Duration::days(offset);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,source_version_id,reference_date,snapshot,created_by) VALUES($1,$2,$3,$4,$5,'{\"offerings\":[]}'::jsonb,$6)")
        .bind(target).bind(c.term_id).bind(c.year_id).bind(base).bind(date).bind(c.teacher_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_term_change_sets(id,academic_term_id,academic_year_id,reference_date,reason,idempotency_key,creation_request_hash,created_by,base_delivery_version_id,target_delivery_version_id) VALUES($1,$2,$3,$4,'Historical migration fixture',$5,repeat('a',64),$6,$7,$8)")
        .bind(revision).bind(c.term_id).bind(c.year_id).bind(date).bind(revision.to_string()).bind(c.teacher_id).bind(base).bind(target).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE academic_delivery_versions SET snapshot=academic_delivery_revision_snapshot($1) WHERE id=$2").bind(revision).bind(target).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    change_sets::get_change_set(pool, revision).await.unwrap()
}

#[tokio::test]
async fn delivery_room_scope_migration_143_to_144_preserves_all_unscoped_bytes() {
    let pool = prepare_delivery_fixture("room_scope_migration", false).await;
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 143).await.unwrap();
    let c = operational_change_runtime_context(&pool).await;
    let rev = historical_revision(&pool, &c, 12).await;
    let before:Value=sqlx::query_scalar("SELECT jsonb_build_object('versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_delivery_versions v),'revisions',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM academic_term_change_sets c),'journal',(SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM academic_term_change_items i),'evidence',(SELECT jsonb_agg(to_jsonb(e) ORDER BY migration_version) FROM academic_delivery_version_migration_audit e))").fetch_one(&pool).await.unwrap();
    let projection: Value = sqlx::query_scalar("SELECT academic_delivery_revision_snapshot($1)")
        .bind(rev.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let registry = registry_facts(&pool).await;
    apply_migrations_through(&pool, 144).await.unwrap();
    let after:Value=sqlx::query_scalar("SELECT jsonb_build_object('versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_delivery_versions v),'revisions',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM academic_term_change_sets c),'journal',(SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM academic_term_change_items i),'evidence',(SELECT jsonb_agg(to_jsonb(e) ORDER BY migration_version) FROM academic_delivery_version_migration_audit e))").fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    assert_eq!(registry, registry_facts(&pool).await);
    let projected: Value = sqlx::query_scalar("SELECT academic_delivery_revision_snapshot($1)")
        .bind(rev.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(projection, projected);
    let mut conn = pool.acquire().await.unwrap();
    let existing: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM learning_offerings WHERE academic_term_id=$1 ORDER BY id",
    )
    .bind(c.term_id)
    .fetch_all(&mut *conn)
    .await
    .unwrap();
    assert!(
        audiences::dated_constraints_on_connection(&mut conn, &existing, rev.reference_date)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn delivery_room_scope_direct_write_serializes_with_publication_and_cancellation() {
    let pool = prepare_concurrent_delivery_runtime_fixture("room_scope_serialization").await;
    let c = planning_runtime_context(&pool).await;
    let rev =
        create_runtime_change_set(&pool, c.teacher_id, c.term_id, 1, "scope-write-lock").await;
    let offering = offerings::create_for_delivery(
        &pool,
        c.teacher_id,
        Some(rev.target_delivery_version_id),
        course_request(&c),
    )
    .await
    .unwrap();
    group(
        &pool,
        &c,
        offering.id,
        "publishable group",
        vec![c.homeroom_id],
    )
    .await;
    let preview = publication_preview(&pool, rev.id).await.unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|f| f.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    let mut scope_tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO academic_delivery_revision_audiences VALUES($1,$2,$3,$4)")
        .bind(rev.id)
        .bind(offering.id)
        .bind(c.term_id)
        .bind(c.year_id)
        .execute(&mut *scope_tx)
        .await
        .unwrap();
    let publish_pool = pool.clone();
    let actor = c.teacher_id;
    let mut publisher = tokio::spawn(async move {
        change_sets::publish_change_set(
            &publish_pool,
            actor,
            rev.id,
            PublishAcademicTermChangeSetRequest {
                effective_from: preview.effective_from,
                row_version: preview.change_set_row_version,
                target_delivery_version_row_version: preview.target_delivery_version_row_version,
                preview_hash: preview.preview_hash,
                acknowledged_warning_codes: preview
                    .findings
                    .iter()
                    .filter(|f| f.severity == AcademicChangeFindingSeverity::Warning)
                    .map(|f| f.code)
                    .collect(),
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(150), &mut publisher)
            .await
            .is_err(),
        "publication must wait for direct intent transaction's parent/version locks"
    );
    scope_tx.rollback().await.unwrap();
    publisher.await.unwrap().unwrap();
    let error = sqlx::query("INSERT INTO academic_delivery_revision_audiences VALUES($1,$2,$3,$4)")
        .bind(rev.id)
        .bind(offering.id)
        .bind(c.term_id)
        .bind(c.year_id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("ACADEMIC_DELIVERY_AUDIENCE_IMMUTABLE"));
    let next =
        create_runtime_change_set(&pool, c.teacher_id, c.term_id, 2, "scope-cancel-lock").await;
    let mut scope_tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO academic_delivery_revision_audiences VALUES($1,$2,$3,$4)")
        .bind(next.id)
        .bind(offering.id)
        .bind(c.term_id)
        .bind(c.year_id)
        .execute(&mut *scope_tx)
        .await
        .unwrap();
    let cancel_pool = pool.clone();
    let mut cancel = tokio::spawn(async move {
        let mut tx = cancel_pool.begin().await.unwrap();
        sqlx::query("UPDATE academic_term_change_sets SET status='cancelled',cancelled_by=$2,cancelled_at=now() WHERE id=$1").bind(next.id).bind(actor).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE academic_delivery_versions SET status='cancelled' WHERE id=$1")
            .bind(next.target_delivery_version_id)
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(150), &mut cancel)
            .await
            .is_err()
    );
    scope_tx.rollback().await.unwrap();
    cancel.await.unwrap();
    let error = sqlx::query("INSERT INTO academic_delivery_revision_audiences VALUES($1,$2,$3,$4)")
        .bind(next.id)
        .bind(offering.id)
        .bind(c.term_id)
        .bind(c.year_id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("ACADEMIC_DELIVERY_AUDIENCE_IMMUTABLE"));
}

pub(super) async fn seed_retained_grade(pool: &PgPool, c: &RuntimeContext, group: Uuid) {
    let student = Uuid::new_v4();
    let year = Uuid::new_v4();
    let starts: NaiveDate = sqlx::query_scalar("SELECT start_date FROM academic_terms WHERE id=$1")
        .bind(c.term_id)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$1::text,'not-a-login','Synthetic','Retained grade','student','active')").bind(student).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES($1,$2,$3,$4,$5,'active')").bind(year).bind(student).bind(c.year_id).bind(c.grade_level_id).bind(c.study_program_id).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_students(id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,published_at) VALUES($1,$2,$3,$4,$5,$6,'active','manual_add',$7,now())").bind(Uuid::new_v4()).bind(group).bind(c.term_id).bind(c.year_id).bind(year).bind(student).bind(starts).execute(pool).await.unwrap();
    let plan = Uuid::new_v4();
    let phase = Uuid::new_v4();
    let item = Uuid::new_v4();
    sqlx::query("INSERT INTO course_assessment_plans(id,academic_term_id,subject_version_id,learning_offering_id,academic_year_id) SELECT $1,g.academic_term_id,d.subject_version_id,g.learning_offering_id,g.academic_year_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$2").bind(plan).bind(group).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO course_assessment_phases(id,plan_id,phase_code,max_score,exam_arrangement) VALUES($1,$2,'before_midterm',25,'none')").bind(phase).bind(plan).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) SELECT $1,id,learning_offering_id,$2,$3,academic_term_id,academic_year_id,'Retained score',10 FROM learning_groups WHERE id=$4").bind(item).bind(plan).bind(phase).bind(group).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_student_scores(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) SELECT $1,id,learning_offering_id,academic_term_id,academic_year_id,$2,$3,7.25 FROM learning_groups WHERE id=$4").bind(Uuid::new_v4()).bind(item).bind(year).bind(group).execute(pool).await.unwrap();
}
pub(super) async fn retained_grade_facts(pool: &PgPool, group: Uuid) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('members',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_students t WHERE learning_group_id=$1),'plans',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM course_assessment_plans t WHERE learning_offering_id=(SELECT learning_offering_id FROM learning_groups WHERE id=$1)),'items',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_score_items t WHERE learning_group_id=$1),'scores',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_scores t WHERE learning_group_id=$1))").bind(group).fetch_one(pool).await.unwrap()
}
