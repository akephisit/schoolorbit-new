use super::*;
use school_academic_delivery::services::versions as delivery_versions;

#[tokio::test]
async fn earlier_publication_preserves_a_draft_groups_explicit_future_teacher_selection() {
    let pool = prepare_delivery_runtime_fixture("publication_draft_teacher_earlier").await;
    let context = planning_runtime_context(&pool).await;
    let today = runtime_operation_date();
    let staged_date = today + Duration::days(10);
    let chosen_date = today + Duration::days(5);
    let revision = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        staged_date,
        "draft-future-teacher",
    )
    .await;
    let CreateLearningOfferingRequest::Course(mut request) = course_request(&context) else {
        unreachable!()
    };
    request.starts_on = staged_date;
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(revision.target_delivery_version_id),
        CreateLearningOfferingRequest::Course(request),
    )
    .await
    .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "Explicit future draft teacher".into(),
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
    let group = groups::replace_teachers(
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
    let original = &group.teacher_assignments[0];
    assert_eq!(original.starts_on, staged_date);
    let raw: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_teachers t WHERE id=$1")
            .bind(original.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let preview = change_sets::preview_change_set_at(&pool, revision.id, Some(chosen_date))
        .await
        .unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    let accepted = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        revision.id,
        PublishAcademicTermChangeSetRequest {
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            effective_from: chosen_date,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: preview
                .findings
                .iter()
                .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
                .map(|finding| finding.code)
                .collect(),
            idempotency_key: Uuid::now_v7(),
        },
    )
    .await
    .unwrap();
    let version = delivery_versions::get_version(&pool, accepted.target_delivery_version_id)
        .await
        .unwrap();
    let selected = version
        .snapshot
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .find(|selected| selected.id == group.id)
        .unwrap();
    assert_eq!(selected.teachers.len(), 1);
    let assignment = &selected.teachers[0];
    assert_eq!(assignment.teacher_id, context.teacher_id);
    assert_eq!(assignment.role, LearningTeacherRole::Primary);
    assert_ne!(assignment.assignment_id, original.id);
    let materialized: (Uuid, NaiveDate, Option<NaiveDate>) = sqlx::query_as(
        "SELECT teacher_id,starts_on,ends_on FROM learning_group_teachers WHERE id=$1",
    )
    .bind(assignment.assignment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(materialized, (context.teacher_id, chosen_date, None));
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_teachers t WHERE id=$1")
            .bind(original.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        raw, after,
        "staged raw facts retain their exact identity and original date"
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT academic_delivery_offering_available($1,$2)")
            .bind(offering.id)
            .bind(chosen_date)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
}
