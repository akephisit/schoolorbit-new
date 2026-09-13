use chrono::{NaiveDate, NaiveTime};
use uuid::Uuid;

use super::exam_schedule_service;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture, CutoverFixture,
};
use crate::modules::academic::models::exam_schedule::{
    CreateExamRoundRequest, ExamSourceChangeKind, ExamSourceSyncItemStatus,
    PlaceExamSessionRequest, SyncExamSourcesRequest, UpsertDayRoomAssignmentRequest,
    UpsertExamDayRequest,
};
use crate::test_helpers::create_named_test_pool_with_max_connections;

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool_with_max_connections(test_name, 3).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44).await.unwrap();
    record_passing_phase_a_reconciliation_marker(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 59).await.unwrap();
    pool
}

#[tokio::test]
async fn room_assignment_upsert_persists_exam_day_academic_context() {
    let pool = migrated_pool("exam_room_assignment_context").await;
    let exam_day_id = Uuid::parse_str("85000000-0000-0000-0000-000000000001").unwrap();
    let homeroom_id = Uuid::parse_str("40000000-0000-0000-0000-000000000025").unwrap();
    let room_id = Uuid::parse_str("92000000-0000-0000-0000-000000000001").unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();

    let assignment = exam_schedule_service::upsert_day_room_assignment(
        &pool,
        exam_day_id,
        UpsertDayRoomAssignmentRequest {
            homeroom_id,
            room_id,
            capacity_override: None,
            invigilator_staff_ids: None,
        },
        actor_id,
    )
    .await
    .unwrap();

    let persisted_context: (Uuid, Uuid) = sqlx::query_as(
        r#"
        SELECT academic_term_id, academic_year_id
        FROM academic_exam_day_room_assignments
        WHERE id = $1
        "#,
    )
    .bind(assignment.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let day_context: (Uuid, Uuid) = sqlx::query_as(
        r#"
        SELECT academic_term_id, academic_year_id
        FROM academic_exam_days
        WHERE id = $1
        "#,
    )
    .bind(exam_day_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(persisted_context, day_context);
}

#[tokio::test]
async fn placing_exam_session_returns_canonical_academic_context() {
    let pool = migrated_pool("exam_session_response_context").await;
    let exam_schedule_item_id = Uuid::parse_str("86000000-0000-0000-0000-000000000001").unwrap();
    let exam_day_id = Uuid::parse_str("85000000-0000-0000-0000-000000000001").unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let expected_context: (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT academic_term_id, academic_year_id, learning_offering_id
           FROM academic_exam_schedule_items
           WHERE id = $1"#,
    )
    .bind(exam_schedule_item_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let session = exam_schedule_service::place_exam_session(
        &pool,
        PlaceExamSessionRequest {
            exam_schedule_item_id,
            exam_day_id,
            starts_at: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
        },
        actor_id,
    )
    .await
    .unwrap();

    assert_eq!(
        (
            session.academic_term_id,
            session.academic_year_id,
            session.learning_offering_id,
        ),
        expected_context
    );

    let other_round = exam_schedule_service::create_round(
        &pool,
        CreateExamRoundRequest {
            academic_term_id: expected_context.0,
            name: "รอบสอบอื่น".into(),
            description: None,
            exam_kind: Some("final".into()),
        },
        actor_id,
    )
    .await
    .unwrap();
    let other_day = exam_schedule_service::upsert_exam_day(
        &pool,
        other_round.id,
        UpsertExamDayRequest {
            exam_date: NaiveDate::from_ymd_opt(2027, 1, 15).unwrap(),
            label: None,
            start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
            grade_level_ids: vec![],
            blocked_windows: vec![],
        },
    )
    .await
    .unwrap();
    let mut foreign_item_lock = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM academic_exam_schedule_items WHERE id=$1 FOR UPDATE")
        .bind(exam_schedule_item_id)
        .execute(&mut *foreign_item_lock)
        .await
        .unwrap();
    let rejected = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        exam_schedule_service::place_exam_session(
            &pool,
            PlaceExamSessionRequest {
                exam_schedule_item_id,
                exam_day_id: other_day.id,
                starts_at: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            },
            actor_id,
        ),
    )
    .await;
    foreign_item_lock.rollback().await.unwrap();
    assert!(
        matches!(rejected, Ok(Err(crate::error::AppError::BadRequest(_)))),
        "foreign-round placement must reject before waiting on its item: {rejected:?}"
    );
}

#[tokio::test]
async fn invigilator_assignment_mutations_return_the_updated_workspace() {
    let pool = migrated_pool("exam_invigilator_mutation_response").await;
    let assignment_id = Uuid::parse_str("92100000-0000-0000-0000-000000000001").unwrap();
    let staff_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();

    let assigned = exam_schedule_service::assign_invigilator_to_assignment(
        &pool,
        assignment_id,
        staff_id,
        staff_id,
    )
    .await
    .unwrap();

    let assigned_room = assigned
        .assignments
        .iter()
        .find(|assignment| assignment.assignment_id == assignment_id)
        .unwrap();
    assert!(assigned_room
        .invigilators
        .iter()
        .any(|invigilator| invigilator.staff_id == staff_id));
    assert!(assigned
        .staff_workloads
        .iter()
        .any(|workload| workload.staff_id == staff_id && workload.total_minutes == 60));

    let removed = exam_schedule_service::remove_invigilator_from_assignment(
        &pool,
        assignment_id,
        staff_id,
        staff_id,
    )
    .await
    .unwrap();

    let removed_room = removed
        .assignments
        .iter()
        .find(|assignment| assignment.assignment_id == assignment_id)
        .unwrap();
    assert!(removed_room
        .invigilators
        .iter()
        .all(|invigilator| invigilator.staff_id != staff_id));
    assert!(removed
        .staff_workloads
        .iter()
        .all(|workload| workload.staff_id != staff_id));
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

#[tokio::test]
async fn creating_exam_round_accepts_canonical_writable_term_statuses() {
    let pool = migrated_pool("exam_round_create_term_statuses").await;
    let term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_exam_rounds ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();

    for status in ["planning", "ready", "active", "closing"] {
        sqlx::query("UPDATE academic_terms SET status = $2 WHERE id = $1")
            .bind(term_id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();

        let round = exam_schedule_service::create_round(
            &pool,
            CreateExamRoundRequest {
                academic_term_id: term_id,
                name: format!("สอบสถานะ {status}"),
                description: None,
                exam_kind: Some("midterm".to_string()),
            },
            actor_id,
        )
        .await
        .unwrap();

        assert_eq!(round.academic_term_id, term_id);
    }
}

#[tokio::test]
async fn exam_lifecycle_rejects_closed_context_mutations_and_retains_history() {
    use crate::error::AppError;
    use crate::modules::academic::models::exam_schedule::{
        GenerateSeatsRequest, UpdateExamInvigilatorsRequest, UpdateExamRoundRequest,
    };

    fn blocked<T: std::fmt::Debug>(result: Result<T, AppError>) {
        assert!(matches!(result, Err(AppError::Conflict(_))), "{result:?}");
    }

    let pool = migrated_pool("exam_lifecycle_closed_context").await;
    let round = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let day = Uuid::parse_str("85000000-0000-0000-0000-000000000001").unwrap();
    let item = Uuid::parse_str("86000000-0000-0000-0000-000000000001").unwrap();
    let assignment = Uuid::parse_str("92100000-0000-0000-0000-000000000001").unwrap();
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (year, term): (Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_year_id, academic_term_id FROM academic_exam_rounds WHERE id=$1",
    )
    .bind(round)
    .fetch_one(&pool)
    .await
    .unwrap();
    let placement = || PlaceExamSessionRequest {
        exam_schedule_item_id: item,
        exam_day_id: day,
        starts_at: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
    };
    let session = exam_schedule_service::place_exam_session(&pool, placement(), actor)
        .await
        .unwrap();
    let day_request = || UpsertExamDayRequest {
        exam_date: NaiveDate::from_ymd_opt(2027, 1, 15).unwrap(),
        label: Some("วันสอบเพิ่มเติม".into()),
        start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
        end_time: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
        grade_level_ids: vec![],
        blocked_windows: vec![],
    };

    for (year_status, term_status) in [
        ("closed", "active"),
        ("archived", "active"),
        ("active", "closed"),
        ("active", "cancelled"),
    ] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(year)
            .bind(year_status)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
            .bind(term)
            .bind(term_status)
            .execute(&pool)
            .await
            .unwrap();
        let before = serde_json::to_value(
            exam_schedule_service::get_workspace(&pool, round)
                .await
                .unwrap(),
        )
        .unwrap();
        blocked(
            exam_schedule_service::create_round(
                &pool,
                CreateExamRoundRequest {
                    academic_term_id: term,
                    name: "ห้ามสร้างหลังปิด".into(),
                    description: None,
                    exam_kind: Some("final".into()),
                },
                actor,
            )
            .await,
        );
        blocked(
            exam_schedule_service::update_round(
                &pool,
                round,
                UpdateExamRoundRequest {
                    name: Some("ห้ามแก้หลังปิด".into()),
                    description: None,
                    exam_kind: None,
                },
                actor,
            )
            .await,
        );
        blocked(exam_schedule_service::upsert_exam_day(&pool, round, day_request()).await);
        blocked(exam_schedule_service::update_exam_day(&pool, day, day_request()).await);
        blocked(
            exam_schedule_service::upsert_day_room_assignment(
                &pool,
                day,
                UpsertDayRoomAssignmentRequest {
                    homeroom_id: Uuid::parse_str("40000000-0000-0000-0000-000000000025").unwrap(),
                    room_id: Uuid::parse_str("92000000-0000-0000-0000-000000000001").unwrap(),
                    capacity_override: None,
                    invigilator_staff_ids: None,
                },
                actor,
            )
            .await,
        );
        blocked(
            exam_schedule_service::generate_seats_for_assignment(
                &pool,
                assignment,
                GenerateSeatsRequest { regenerate: true },
                actor,
            )
            .await,
        );
        blocked(exam_schedule_service::place_exam_session(&pool, placement(), actor).await);
        blocked(
            exam_schedule_service::update_assignment_invigilators(
                &pool,
                assignment,
                UpdateExamInvigilatorsRequest {
                    invigilator_staff_ids: vec![actor],
                },
                actor,
            )
            .await,
        );
        blocked(
            exam_schedule_service::assign_invigilator_to_assignment(
                &pool, assignment, actor, actor,
            )
            .await,
        );
        blocked(
            exam_schedule_service::remove_invigilator_from_assignment(
                &pool, assignment, actor, actor,
            )
            .await,
        );
        let preview = exam_schedule_service::preview_exam_sources(&pool, round)
            .await
            .unwrap();
        blocked(
            exam_schedule_service::sync_exam_sources(
                &pool,
                round,
                actor,
                SyncExamSourcesRequest {
                    round_row_version: preview.round_row_version,
                    preview_token: preview.preview_token,
                    source_ids: vec![],
                },
            )
            .await,
        );
        blocked(exam_schedule_service::publish_round(&pool, round, actor).await);
        blocked(exam_schedule_service::delete_exam_session(&pool, session.id, actor).await);
        blocked(exam_schedule_service::delete_exam_day(&pool, day).await);
        blocked(exam_schedule_service::delete_round(&pool, round, true).await);
        let after = serde_json::to_value(
            exam_schedule_service::get_workspace(&pool, round)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(before, after);
    }

    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(year)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closing' WHERE id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    let mut boundary = pool.begin().await.unwrap();
    let boundary_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *boundary)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM academic_terms WHERE id=$1 FOR UPDATE")
        .bind(term)
        .execute(&mut *boundary)
        .await
        .unwrap();
    let worker_pool = pool.clone();
    let update = day_request();
    let worker = tokio::spawn(async move {
        exam_schedule_service::update_exam_day(&worker_pool, day, update).await
    });
    let mut waiting = false;
    for _ in 0..200 {
        waiting = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))",
        )
        .bind(boundary_pid)
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let mut probe = pool.begin().await.unwrap();
    let day_free = sqlx::query("SELECT id FROM academic_exam_days WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(day)
        .execute(&mut *probe)
        .await
        .is_ok();
    let round_free = if day_free {
        sqlx::query("SELECT id FROM academic_exam_rounds WHERE id=$1 FOR UPDATE NOWAIT")
            .bind(round)
            .execute(&mut *probe)
            .await
            .is_ok()
    } else {
        false
    };
    probe.rollback().await.unwrap();
    boundary.commit().await.unwrap();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), worker)
        .await
        .unwrap()
        .unwrap();
    assert!(
        waiting && day_free && round_free,
        "term must lock before exam entities"
    );
    assert!(outcome.is_ok(), "closing remains writable: {outcome:?}");
}

#[tokio::test]
async fn creating_exam_round_in_closed_term_returns_conflict() {
    let pool = migrated_pool("exam_round_create_closed_term").await;
    let term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_exam_rounds ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();

    sqlx::query("UPDATE academic_terms SET status = 'closed' WHERE id = $1")
        .bind(term_id)
        .execute(&pool)
        .await
        .unwrap();

    let error = exam_schedule_service::create_round(
        &pool,
        CreateExamRoundRequest {
            academic_term_id: term_id,
            name: "สอบในภาคเรียนที่ปิดแล้ว".to_string(),
            description: None,
            exam_kind: Some("final".to_string()),
        },
        actor_id,
    )
    .await
    .unwrap_err();

    assert!(matches!(error, crate::error::AppError::Conflict(_)));
}

#[tokio::test]
async fn creating_exam_day_copies_the_round_academic_context() {
    let pool = migrated_pool("exam_day_create_context").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let expected_context: (Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_term_id, academic_year_id FROM academic_exam_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let day = exam_schedule_service::upsert_exam_day(
        &pool,
        round_id,
        UpsertExamDayRequest {
            exam_date: NaiveDate::from_ymd_opt(2027, 1, 15).unwrap(),
            label: Some("วันสอบเพิ่มเติม".to_string()),
            start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
            grade_level_ids: Vec::new(),
            blocked_windows: Vec::new(),
        },
    )
    .await
    .unwrap();

    let stored_context: (Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_term_id, academic_year_id FROM academic_exam_days WHERE id = $1",
    )
    .bind(day.id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(stored_context, expected_context);
}

#[tokio::test]
async fn deleting_an_exam_round_cascades_owned_schedule_data() {
    let pool = migrated_pool("exam_round_delete_cascade").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();

    exam_schedule_service::delete_round(&pool, round_id, true)
        .await
        .unwrap();

    let round_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM academic_exam_rounds WHERE id = $1")
            .bind(round_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let day_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM academic_exam_days WHERE exam_round_id = $1")
            .bind(round_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let item_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM academic_exam_schedule_items WHERE exam_round_id = $1",
    )
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(round_count, 0);
    assert_eq!(day_count, 0);
    assert_eq!(item_count, 0);
}

#[tokio::test]
async fn deleting_a_published_exam_round_without_publish_permission_is_denied() {
    let pool = migrated_pool("exam_round_delete_published_denied").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();

    sqlx::query(
        "UPDATE academic_exam_rounds SET status = 'published', published_at = now() WHERE id = $1",
    )
    .bind(round_id)
    .execute(&pool)
    .await
    .unwrap();

    let error = exam_schedule_service::delete_round(&pool, round_id, false)
        .await
        .unwrap_err();

    assert!(matches!(error, crate::error::AppError::Forbidden(_)));
    let round_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM academic_exam_rounds WHERE id = $1")
            .bind(round_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(round_count, 1);
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

#[tokio::test]
async fn paper_receipts_include_outside_exams_without_adding_scheduling_items() {
    let pool = migrated_pool("exam_paper_receipts").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let before = exam_schedule_service::get_workspace(&pool, round_id)
        .await
        .unwrap();
    assert!(!before.paper_receipt_items.is_empty());
    assert!(before
        .paper_receipt_items
        .iter()
        .all(|item| item.exam_arrangement == "in_timetable"));
    let phase_id = before.paper_receipt_items[0].assessment_phase_id;
    sqlx::query("UPDATE course_assessment_phases SET exam_arrangement = 'outside_timetable', exam_duration_minutes = NULL WHERE id = $1")
        .bind(phase_id).execute(&pool).await.unwrap();
    let outside = exam_schedule_service::get_workspace(&pool, round_id)
        .await
        .unwrap();
    assert_eq!(
        outside.paper_receipt_items.len(),
        before.paper_receipt_items.len()
    );
    assert!(outside
        .paper_receipt_items
        .iter()
        .filter(|item| item.assessment_phase_id == phase_id)
        .all(|item| item.exam_arrangement == "outside_timetable"));
    assert_eq!(
        outside.unscheduled_items.len(),
        before.unscheduled_items.len()
    );
    assert_eq!(
        outside.scheduled_sessions.len(),
        before.scheduled_sessions.len()
    );

    sqlx::query("UPDATE course_assessment_phases SET exam_arrangement = 'none' WHERE id = $1")
        .bind(phase_id)
        .execute(&pool)
        .await
        .unwrap();
    let no_exam = exam_schedule_service::get_workspace(&pool, round_id)
        .await
        .unwrap();
    assert!(no_exam
        .paper_receipt_items
        .iter()
        .all(|item| item.assessment_phase_id != phase_id));

    // The round's phase and term are authoritative even if another phase has an exam.
    sqlx::query("UPDATE academic_exam_rounds SET exam_kind = 'final' WHERE id = $1")
        .bind(round_id)
        .execute(&pool)
        .await
        .unwrap();
    let final_round = exam_schedule_service::get_workspace(&pool, round_id)
        .await
        .unwrap();
    assert!(final_round
        .paper_receipt_items
        .iter()
        .all(|item| item.assessment_phase_id != phase_id));
}
