use uuid::Uuid;

use super::exam_schedule_service;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture, CutoverFixture,
};
use crate::modules::academic::models::exam_schedule::{
    CreateExamRoundRequest, ExamSourceChangeKind, ExamSourceSyncItemStatus, SyncExamSourcesRequest,
};
use crate::test_helpers::create_named_test_pool;

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44).await.unwrap();
    record_passing_phase_a_reconciliation_marker(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 56).await.unwrap();
    pool
}

#[tokio::test]
async fn source_preview_and_sync_preserve_snapshots_until_explicit_selection() {
    let pool = migrated_pool("exam_source_preview_sync").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();

    sqlx::query("DELETE FROM academic_exam_sessions WHERE exam_round_id = $1")
        .bind(round_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM academic_exam_schedule_items WHERE exam_round_id = $1")
        .bind(round_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE course_assessment_plans SET assessment_coordinator_id = $1, row_version = row_version + 1",
    )
        .bind(actor_id)
        .execute(&pool)
        .await
        .unwrap();

    let initial = exam_schedule_service::preview_exam_sources(&pool, round_id)
        .await
        .unwrap();
    assert!(initial.new_count > 1);
    assert!(initial
        .changes
        .iter()
        .all(|change| change.change_kind == ExamSourceChangeKind::New));
    let synced = exam_schedule_service::sync_exam_sources(
        &pool,
        round_id,
        actor_id,
        SyncExamSourcesRequest {
            round_row_version: initial.round_row_version,
            preview_token: initial.preview_token,
            source_ids: initial
                .changes
                .iter()
                .map(|change| change.source_id)
                .collect(),
        },
    )
    .await
    .unwrap();
    assert_eq!(synced.inserted_count, initial.new_count);

    let midterm_phase_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM course_assessment_phases WHERE phase_code = 'midterm' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE course_assessment_phases SET exam_duration_minutes = 90 WHERE id = $1")
        .bind(midterm_phase_id)
        .execute(&pool)
        .await
        .unwrap();
    let changed = exam_schedule_service::preview_exam_sources(&pool, round_id)
        .await
        .unwrap();
    assert_eq!(changed.duration_changed_count, initial.new_count);
    let updated = exam_schedule_service::sync_exam_sources(
        &pool,
        round_id,
        actor_id,
        SyncExamSourcesRequest {
            round_row_version: changed.round_row_version,
            preview_token: changed.preview_token,
            source_ids: changed
                .changes
                .iter()
                .map(|change| change.source_id)
                .collect(),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.updated_duration_count, initial.new_count);

    sqlx::query(
        "UPDATE course_assessment_phases SET exam_arrangement = 'outside_timetable' WHERE id = $1",
    )
    .bind(midterm_phase_id)
    .execute(&pool)
    .await
    .unwrap();
    let ineligible = exam_schedule_service::preview_exam_sources(&pool, round_id)
        .await
        .unwrap();
    assert_eq!(ineligible.no_longer_eligible_count, initial.new_count);
    let snapshot_count_before_sync: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM academic_exam_schedule_items WHERE exam_round_id = $1",
    )
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(snapshot_count_before_sync, initial.new_count);

    let removed = exam_schedule_service::sync_exam_sources(
        &pool,
        round_id,
        actor_id,
        SyncExamSourcesRequest {
            round_row_version: ineligible.round_row_version,
            preview_token: ineligible.preview_token,
            source_ids: ineligible
                .changes
                .iter()
                .map(|change| change.source_id)
                .collect(),
        },
    )
    .await
    .unwrap();
    assert_eq!(removed.removed_count, initial.new_count);
}

#[tokio::test]
async fn published_round_source_preview_is_read_only() {
    let pool = migrated_pool("exam_published_source_preview").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    sqlx::query(
        "UPDATE academic_exam_rounds SET status = 'published', published_at = now() WHERE id = $1",
    )
    .bind(round_id)
    .execute(&pool)
    .await
    .unwrap();
    let preview = exam_schedule_service::preview_exam_sources(&pool, round_id)
        .await
        .unwrap();
    assert_eq!(preview.round_status, "published");
    let result = exam_schedule_service::sync_exam_sources(
        &pool,
        round_id,
        actor_id,
        SyncExamSourcesRequest {
            round_row_version: preview.round_row_version,
            preview_token: preview.preview_token,
            source_ids: Vec::new(),
        },
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn placed_duration_sync_revalidates_the_existing_schedule_before_mutation() {
    let pool = migrated_pool("exam_duration_sync_conflict").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    sqlx::query(
        "UPDATE course_assessment_plans SET assessment_coordinator_id = $1, row_version = row_version + 1",
    )
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE course_assessment_phases SET exam_duration_minutes = 600 WHERE phase_code = 'midterm'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let preview = exam_schedule_service::preview_exam_sources(&pool, round_id)
        .await
        .unwrap();
    let scheduled_change = preview
        .changes
        .iter()
        .find(|change| {
            change.change_kind == ExamSourceChangeKind::DurationChanged && change.scheduled
        })
        .expect("fixture must expose one placed duration change");
    let item_id = scheduled_change.exam_schedule_item_id.unwrap();
    let result = exam_schedule_service::sync_exam_sources(
        &pool,
        round_id,
        actor_id,
        SyncExamSourcesRequest {
            round_row_version: preview.round_row_version,
            preview_token: preview.preview_token.clone(),
            source_ids: vec![scheduled_change.source_id],
        },
    )
    .await
    .unwrap();
    assert_eq!(result.updated_duration_count, 0);
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0].source_id, scheduled_change.source_id);
    assert_eq!(result.results[0].status, ExamSourceSyncItemStatus::Conflict);
    assert!(result.results[0].message.is_some());
    assert_eq!(result.round_row_version, preview.round_row_version);

    let snapshot_duration: i32 = sqlx::query_scalar(
        "SELECT duration_minutes FROM academic_exam_schedule_items WHERE id = $1",
    )
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(snapshot_duration, 60);
}

#[tokio::test]
async fn imported_item_keeps_canonical_context() {
    let pool = migrated_pool("exam_canonical_context").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();

    let (round_term_id, item_term_id, offering_id, group_id, plan_offering_id, subject_id): (
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Uuid,
    ) = sqlx::query_as(
        r#"SELECT round.academic_term_id, item.academic_term_id,
                  item.learning_offering_id, item.learning_group_id,
                  plan.learning_offering_id, item.subject_id
           FROM academic_exam_schedule_items item
           JOIN academic_exam_rounds round ON round.id = item.exam_round_id
           JOIN course_assessment_plans plan ON plan.id = item.course_assessment_plan_id
           JOIN subjects subject ON subject.id = item.subject_id
           WHERE item.exam_round_id = $1"#,
    )
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(round_term_id, item_term_id);
    assert_eq!(offering_id, plan_offering_id);

    let group_context: (Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_term_id, learning_offering_id FROM learning_groups WHERE id = $1",
    )
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(group_context, (item_term_id, offering_id));
    assert_ne!(subject_id, Uuid::nil());
}

#[tokio::test]
async fn published_student_view_requires_term_and_uses_group_roster() {
    let pool = migrated_pool("exam_published_group_roster").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let student_id = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
    let term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_exam_rounds WHERE id = $1")
            .bind(round_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    sqlx::query(
        r#"UPDATE academic_exam_rounds
           SET status = 'published', published_at = now(), published_by = $2,
               row_version = row_version + 1, updated_by = $2, updated_at = now()
           WHERE id = $1"#,
    )
    .bind(round_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();

    let schedule =
        exam_schedule_service::list_my_published_exam_schedule(&pool, student_id, term_id)
            .await
            .unwrap();
    assert_eq!(schedule.len(), 1);
    assert_eq!(schedule[0].academic_term_id, term_id);
    assert_eq!(schedule[0].sessions.len(), 1);

    sqlx::query(
        r#"UPDATE learning_group_students
           SET membership_status = 'removed', left_at = joined_at
           WHERE student_id = $1 AND academic_term_id = $2"#,
    )
    .bind(student_id)
    .bind(term_id)
    .execute(&pool)
    .await
    .unwrap();
    let removed =
        exam_schedule_service::list_my_published_exam_schedule(&pool, student_id, term_id)
            .await
            .unwrap();
    assert!(removed.is_empty());
}

#[tokio::test]
async fn round_reads_are_explicitly_term_scoped() {
    let pool = migrated_pool("exam_round_term_scope").await;
    let term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_exam_rounds ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let rounds = exam_schedule_service::list_rounds(&pool, term_id)
        .await
        .unwrap();
    assert!(!rounds.is_empty());
    assert!(rounds.iter().all(|round| round.academic_term_id == term_id));
}

#[test]
fn exam_round_wire_rejects_legacy_semester_identity() {
    let payload = serde_json::json!({
        "academicSemesterId": Uuid::new_v4(),
        "name": "สอบกลางภาค",
        "examKind": "midterm"
    });
    assert!(serde_json::from_value::<CreateExamRoundRequest>(payload).is_err());
}
