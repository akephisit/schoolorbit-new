use super::*;
use school_academic_delivery::{
    models::{
        versions::DeliverySnapshot, AcademicTermChangeItem, AcademicTermChangeSet,
        AcademicTermChangeSetPreview,
    },
    services::versions,
};
use serde_json::Value;

async fn stage_group(
    pool: &PgPool,
    context: &RuntimeContext,
    offering: Uuid,
    teacher: Uuid,
    name: &str,
) -> Uuid {
    let group = groups::create(
        pool,
        context.teacher_id,
        offering,
        CreateLearningGroupRequest {
            name: name.into(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    let group = groups::replace_homerooms(
        pool,
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
        pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: teacher,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap()
    .id
}

async fn course(
    pool: &PgPool,
    context: &RuntimeContext,
    delivery: Option<Uuid>,
    catalog: Uuid,
    starts_on: NaiveDate,
) -> Uuid {
    let CreateLearningOfferingRequest::Course(mut request) = course_request(context) else {
        unreachable!()
    };
    request.subject_version_id = catalog;
    request.starts_on = starts_on;
    offerings::create_for_delivery(
        pool,
        context.teacher_id,
        delivery,
        CreateLearningOfferingRequest::Course(request),
    )
    .await
    .unwrap()
    .id
}

async fn publish_offering(pool: &PgPool, context: &RuntimeContext, id: Uuid) {
    let offering = offerings::get(pool, id).await.unwrap();
    offerings::publish(
        pool,
        context.teacher_id,
        id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::now_v7(),
        },
    )
    .await
    .unwrap();
}

async fn second_teacher(pool: &PgPool, first: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' AND status='active' AND id<>$1 ORDER BY id LIMIT 1")
        .bind(first).fetch_one(pool).await.unwrap()
}

async fn candidate(pool: &PgPool, revision: Uuid, date: NaiveDate) -> DeliverySnapshot {
    // This is the typed owner projection consumed by the actual preview below.
    // Reading it exposes the candidate graph without mutating the draft date.
    sqlx::query_scalar::<_, sqlx::types::Json<DeliverySnapshot>>(
        "SELECT academic_delivery_revision_snapshot($1,$2)",
    )
    .bind(revision)
    .bind(date)
    .fetch_one(pool)
    .await
    .unwrap()
    .0
}

fn teacher(
    graph: &DeliverySnapshot,
    group: Uuid,
) -> &school_academic_delivery::models::versions::DeliveryVersionTeacher {
    let group = graph
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .find(|row| row.id == group)
        .unwrap();
    assert_eq!(group.teachers.len(), 1);
    &group.teachers[0]
}

async fn state(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_delivery_versions v),'revisions',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM academic_term_change_sets c),'items',(SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM academic_term_change_items i),'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t),'offerings',(SELECT jsonb_agg(to_jsonb(o) ORDER BY id) FROM learning_offerings o),'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM academic_audit_events a))")
        .fetch_one(pool).await.unwrap()
}

fn assert_ready(preview: &AcademicTermChangeSetPreview) {
    let blocking = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking)
        .collect::<Vec<_>>();
    assert!(
        blocking.is_empty(),
        "unexpected initial-construction findings: {blocking:#?}"
    );
}

async fn publish_at(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    date: NaiveDate,
) -> AcademicTermChangeSet {
    let preview = change_sets::preview_change_set_at(pool, id, Some(date))
        .await
        .unwrap();
    assert_ready(&preview);
    change_sets::publish_change_set(
        pool,
        actor,
        id,
        PublishAcademicTermChangeSetRequest {
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            effective_from: date,
            preview_hash: preview.preview_hash,
            idempotency_key: Uuid::now_v7(),
            acknowledged_warning_codes: preview
                .findings
                .iter()
                .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
                .map(|finding| finding.code)
                .collect(),
        },
    )
    .await
    .unwrap()
}

async fn remove_only_command(
    pool: &PgPool,
    actor: Uuid,
    revision: &AcademicTermChangeSet,
) -> AcademicTermChangeSet {
    assert_eq!(revision.items.len(), 1);
    let (id, row_version) = match &revision.items[0] {
        AcademicTermChangeItem::StopOffering {
            id, row_version, ..
        }
        | AcademicTermChangeItem::StopGroupTeacher {
            id, row_version, ..
        }
        | AcademicTermChangeItem::AdjustGroupTeacherRole {
            id, row_version, ..
        } => (*id, *row_version),
        other => panic!("unexpected initial construction command: {other:?}"),
    };
    change_sets::delete_change_item(
        pool,
        actor,
        revision.id,
        id,
        DeleteAcademicTermChangeItemRequest {
            change_set_row_version: revision.row_version,
            item_row_version: row_version,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn first_plan_keeps_initial_offerings_and_published_teachers_across_earlier_dates_and_removed_commands(
) {
    let pool = prepare_concurrent_delivery_runtime_fixture("initial_construction_baseline").await;
    let context = planning_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let yesterday = today - Duration::days(1);
    let (other_catalog, _, _) = add_operational_change_catalog_fixture(&pool, &context).await;
    let a = course(&pool, &context, None, context.subject_version_id, today).await;
    let ga = stage_group(&pool, &context, a, context.teacher_id, "Initial A").await;
    publish_offering(&pool, &context, a).await;
    let b = course(&pool, &context, None, other_catalog, yesterday).await;
    let gb = stage_group(&pool, &context, b, context.teacher_id, "Initial B").await;
    publish_offering(&pool, &context, b).await;
    let raw:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t WHERE learning_group_id=ANY($1)")
        .bind(vec![ga,gb]).fetch_one(&pool).await.unwrap();
    let draft = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        today,
        "initial-baseline-create",
    )
    .await;
    assert_eq!(draft.base_delivery_version_id, None);
    assert!(draft.items.is_empty());
    let baseline:(Option<Vec<Uuid>>,Vec<Uuid>,Vec<Uuid>)=sqlx::query_as("SELECT initial_offering_ids,initial_teacher_group_ids,initial_teacher_assignment_ids FROM academic_term_change_sets WHERE id=$1")
        .bind(draft.id).fetch_one(&pool).await.unwrap();
    let mut selected = baseline.0.clone().unwrap();
    selected.sort_unstable();
    let mut expected = vec![a, b];
    expected.sort_unstable();
    assert_eq!(selected, expected);
    assert!(baseline.1.contains(&ga) && baseline.1.contains(&gb));
    let before = state(&pool).await;
    let preview = change_sets::preview_change_set_at(&pool, draft.id, Some(yesterday))
        .await
        .unwrap();
    assert_ready(&preview);
    let graph = candidate(&pool, draft.id, yesterday).await;
    assert_eq!(graph.offerings.len(), 2);
    assert_eq!(
        (teacher(&graph, ga).teacher_id, teacher(&graph, ga).role),
        (context.teacher_id, LearningTeacherRole::Primary)
    );
    assert_eq!(
        (teacher(&graph, gb).teacher_id, teacher(&graph, gb).role),
        (context.teacher_id, LearningTeacherRole::Primary)
    );
    assert_eq!(
        state(&pool).await,
        before,
        "candidate previews must not recapture or persist a different initial selection"
    );
    let draft = change_sets::update_change_set(
        &pool,
        context.teacher_id,
        draft.id,
        UpdateAcademicTermChangeSetRequest {
            row_version: draft.row_version,
            reference_date: yesterday,
            reason: draft.reason,
        },
    )
    .await
    .unwrap();
    let baseline_graph = serde_json::to_value(candidate(&pool, draft.id, yesterday).await).unwrap();
    let stopped = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        draft.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: draft.row_version,
            item_row_version: None,
            learning_offering_id: a,
        },
    )
    .await
    .unwrap();
    assert!(!candidate(&pool, draft.id, yesterday)
        .await
        .offerings
        .iter()
        .any(|offering| offering.id == a));
    let restored = remove_only_command(&pool, context.teacher_id, &stopped).await;
    assert!(
        restored.items.is_empty(),
        "removing StopOffering must not synthesize AddOffering"
    );
    assert_eq!(
        serde_json::to_value(candidate(&pool, draft.id, yesterday).await).unwrap(),
        baseline_graph
    );
    let current = candidate(&pool, draft.id, yesterday).await;
    let prospective = teacher(&current, ga).assignment_id;
    let stopped = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        draft.id,
        UpsertAcademicTermChangeItemRequest::StopGroupTeacher {
            change_set_row_version: restored.row_version,
            item_row_version: None,
            learning_group_id: ga,
            learning_group_teacher_id: prospective,
            teacher_id: context.teacher_id,
        },
    )
    .await
    .unwrap();
    assert!(candidate(&pool, draft.id, yesterday)
        .await
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .find(|group| group.id == ga)
        .unwrap()
        .teachers
        .is_empty());
    let restored = remove_only_command(&pool, context.teacher_id, &stopped).await;
    assert_eq!(
        serde_json::to_value(candidate(&pool, draft.id, yesterday).await).unwrap(),
        baseline_graph
    );
    let adjusted = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        draft.id,
        UpsertAcademicTermChangeItemRequest::AdjustGroupTeacherRole {
            change_set_row_version: restored.row_version,
            item_row_version: None,
            learning_group_id: ga,
            learning_group_teacher_id: prospective,
            teacher_id: context.teacher_id,
            teacher_role: LearningTeacherRole::Assistant,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        teacher(&candidate(&pool, draft.id, yesterday).await, ga).role,
        LearningTeacherRole::Assistant
    );
    let restored = remove_only_command(&pool, context.teacher_id, &adjusted).await;
    assert!(restored.items.is_empty());
    assert_eq!(
        serde_json::to_value(candidate(&pool, draft.id, yesterday).await).unwrap(),
        baseline_graph
    );
    let retained:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t WHERE learning_group_id=ANY($1)")
        .bind(vec![ga,gb]).fetch_one(&pool).await.unwrap();
    assert_eq!(retained, raw);
    let accepted = publish_at(&pool, context.teacher_id, draft.id, yesterday).await;
    assert_eq!(accepted.publication_no, Some(1));
    let after:(Option<Vec<Uuid>>,Vec<Uuid>,Vec<Uuid>)=sqlx::query_as("SELECT initial_offering_ids,initial_teacher_group_ids,initial_teacher_assignment_ids FROM academic_term_change_sets WHERE id=$1")
        .bind(draft.id).fetch_one(&pool).await.unwrap();
    assert_eq!(after, baseline);
}

#[tokio::test]
async fn first_plan_draft_teacher_edits_stay_live_until_standalone_publication_freezes_the_last_selection(
) {
    let pool =
        prepare_concurrent_delivery_runtime_fixture("initial_construction_live_teacher").await;
    let context = planning_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let yesterday = today - Duration::days(1);
    let a = course(&pool, &context, None, context.subject_version_id, today).await;
    let group = stage_group(
        &pool,
        &context,
        a,
        context.teacher_id,
        "Live staged teacher",
    )
    .await;
    let draft = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        today,
        "initial-live-create",
    )
    .await;
    assert_eq!(draft.base_delivery_version_id, None);
    let replacement = second_teacher(&pool, context.teacher_id).await;
    let current = groups::get(&pool, group).await.unwrap();
    let edited = groups::replace_teachers(
        &pool,
        context.teacher_id,
        group,
        ReplaceLearningGroupTeachersRequest {
            row_version: current.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: replacement,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let assignment = edited.teacher_assignments[0].id;
    assert_eq!(
        teacher(&candidate(&pool, draft.id, today).await, group).teacher_id,
        replacement
    );
    publish_offering(&pool, &context, a).await;
    let (marked,assignments):(Vec<Uuid>,Vec<Uuid>)=sqlx::query_as("SELECT initial_teacher_group_ids,initial_teacher_assignment_ids FROM academic_term_change_sets WHERE id=$1")
        .bind(draft.id).fetch_one(&pool).await.unwrap();
    assert!(marked.contains(&group));
    assert!(assignments.contains(&assignment));
    let before = state(&pool).await;
    let preview = change_sets::preview_change_set_at(&pool, draft.id, Some(yesterday))
        .await
        .unwrap();
    assert_ready(&preview);
    let graph = candidate(&pool, draft.id, yesterday).await;
    assert_eq!(
        (
            teacher(&graph, group).teacher_id,
            teacher(&graph, group).role
        ),
        (replacement, LearningTeacherRole::Primary)
    );
    assert_eq!(state(&pool).await, before);
    let accepted = publish_at(&pool, context.teacher_id, draft.id, yesterday).await;
    let graph = versions::get_version(&pool, accepted.target_delivery_version_id)
        .await
        .unwrap()
        .snapshot;
    assert_eq!(teacher(&graph, group).teacher_id, replacement);
}

#[tokio::test]
async fn base_present_added_offering_keeps_the_last_staged_teacher_when_standalone_published_before_an_earlier_candidate(
) {
    let pool =
        prepare_concurrent_delivery_runtime_fixture("initial_construction_added_teacher").await;
    let context = operational_change_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let starts = today + Duration::days(10);
    let earlier = today + Duration::days(5);
    let (catalog, _, _) = add_operational_change_catalog_fixture(&pool, &context).await;
    let draft = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        starts,
        "initial-added-create",
    )
    .await;
    assert!(draft.base_delivery_version_id.is_some());
    let CreateLearningOfferingRequest::Course(mut request) = course_request(&context) else {
        unreachable!()
    };
    request.subject_version_id = catalog;
    request.starts_on = starts;
    let added = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        draft.id,
        UpsertAcademicTermChangeItemRequest::AddCourse {
            change_set_row_version: draft.row_version,
            offering: request,
        },
    )
    .await
    .unwrap();
    let offering = added
        .items
        .iter()
        .find_map(|item| match item {
            AcademicTermChangeItem::AddOffering {
                learning_offering_id,
                ..
            } => Some(*learning_offering_id),
            _ => None,
        })
        .unwrap();
    // AddCourse has already created the group for the one selected homeroom.
    // Stage and replace its teachers instead of leaving that generated group
    // empty while configuring a second group.
    let generated = groups::list(&pool, offering).await.unwrap();
    assert_eq!(generated.len(), 1);
    let group = generated[0].id;
    let staged = groups::replace_teachers(
        &pool,
        context.teacher_id,
        group,
        ReplaceLearningGroupTeachersRequest {
            row_version: generated[0].row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let first_assignment = staged.teacher_assignments[0].id;
    let replacement = second_teacher(&pool, context.teacher_id).await;
    let edited = groups::replace_teachers(
        &pool,
        context.teacher_id,
        group,
        ReplaceLearningGroupTeachersRequest {
            row_version: staged.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: replacement,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let last_assignment = edited.teacher_assignments[0].id;
    publish_offering(&pool, &context, offering).await;
    let (initial,marked,assignments):(Option<Vec<Uuid>>,Vec<Uuid>,Vec<Uuid>)=sqlx::query_as("SELECT initial_offering_ids,initial_teacher_group_ids,initial_teacher_assignment_ids FROM academic_term_change_sets WHERE id=$1")
        .bind(draft.id).fetch_one(&pool).await.unwrap();
    assert_eq!(initial, None);
    assert!(marked.contains(&group));
    assert!(assignments.contains(&last_assignment));
    assert!(!assignments.contains(&first_assignment));
    let before = state(&pool).await;
    let preview = change_sets::preview_change_set_at(&pool, draft.id, Some(earlier))
        .await
        .unwrap();
    assert_ready(&preview);
    let graph = candidate(&pool, draft.id, earlier).await;
    assert!(graph.offerings.iter().any(|row| row.id == offering));
    assert_eq!(
        (
            teacher(&graph, group).teacher_id,
            teacher(&graph, group).role
        ),
        (replacement, LearningTeacherRole::Primary)
    );
    assert_eq!(state(&pool).await, before);
    let accepted = publish_at(&pool, context.teacher_id, draft.id, earlier).await;
    assert_eq!(
        teacher(
            &versions::get_version(&pool, accepted.target_delivery_version_id)
                .await
                .unwrap()
                .snapshot,
            group
        )
        .teacher_id,
        replacement
    );
}
