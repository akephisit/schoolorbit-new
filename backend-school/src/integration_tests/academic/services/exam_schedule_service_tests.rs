use chrono::{NaiveDate, NaiveTime};
use uuid::Uuid;

use super::exam_schedule_service;
use crate::integration_tests::academic::cutover_test_support::{
    CutoverFixture, apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture,
};
use school_academic_assessment::exam_schedule::models::{
    CreateExamRoundRequest, PlaceExamSessionRequest, UpsertDayRoomAssignmentRequest,
    UpsertExamDayRequest,
};
use school_test_db::create_named_test_pool_with_max_connections;

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
    school_test_db::run_test_migrations(&pool).await;
    sqlx::query("UPDATE course_assessment_plans plan SET assessment_coordinator_id='50000000-0000-0000-0000-000000000002' WHERE EXISTS(SELECT 1 FROM learning_groups g JOIN learning_group_teachers t ON t.learning_group_id=g.id WHERE g.learning_offering_id=plan.learning_offering_id AND t.teacher_id='50000000-0000-0000-0000-000000000002')").execute(&pool).await.unwrap();
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
        matches!(rejected, Ok(Err(school_errors::AppError::BadRequest(_)))),
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
    assert!(
        assigned_room
            .invigilators
            .iter()
            .any(|invigilator| invigilator.staff_id == staff_id)
    );
    assert!(
        assigned
            .staff_workloads
            .iter()
            .any(|workload| workload.staff_id == staff_id && workload.total_minutes == 60)
    );

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
    assert!(
        removed_room
            .invigilators
            .iter()
            .all(|invigilator| invigilator.staff_id != staff_id)
    );
    assert!(
        removed
            .staff_workloads
            .iter()
            .all(|workload| workload.staff_id != staff_id)
    );
}

#[tokio::test]
async fn current_sources_refresh_without_writes_or_an_import_stage() {
    let pool = migrated_pool("exam_current_sources").await;
    let round = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    sqlx::query("DELETE FROM academic_exam_sessions WHERE exam_round_id=$1")
        .bind(round)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM academic_exam_schedule_items WHERE exam_round_id=$1")
        .bind(round)
        .execute(&pool)
        .await
        .unwrap();
    let first = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert!(first.unscheduled_items.len() > 1);
    let repeated = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert_eq!(
        first
            .unscheduled_items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        repeated
            .unscheduled_items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_exam_schedule_items WHERE exam_round_id=$1"
        )
        .bind(round)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let phase = first.unscheduled_items[0].assessment_phase_id;
    sqlx::query("UPDATE course_assessment_phases SET exam_duration_minutes=75 WHERE id=$1")
        .bind(phase)
        .execute(&pool)
        .await
        .unwrap();
    let refreshed = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert!(
        refreshed
            .unscheduled_items
            .iter()
            .filter(|item| item.assessment_phase_id == phase)
            .all(|item| item.duration_minutes == 75)
    );
    sqlx::query(
        "UPDATE course_assessment_phases SET exam_arrangement='outside_timetable' WHERE id=$1",
    )
    .bind(phase)
    .execute(&pool)
    .await
    .unwrap();
    let outside = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert!(
        outside
            .unscheduled_items
            .iter()
            .all(|item| item.assessment_phase_id != phase)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_exam_schedule_items WHERE exam_round_id=$1"
        )
        .bind(round)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn current_round_metadata_is_revision_checked_and_preserves_placements() {
    use school_academic_assessment::exam_schedule::models::UpdateExamRoundRequest;
    let pool = migrated_pool("exam_current_metadata").await;
    let round = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let before = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    let request = || UpdateExamRoundRequest {
        row_version: before.round.row_version,
        name: Some("Current final schedule".into()),
        description: None,
        exam_kind: None,
    };
    let saved = exam_schedule_service::update_round(&pool, round, request(), actor)
        .await
        .unwrap();
    assert_eq!(saved.row_version, before.round.row_version + 1);
    assert!(matches!(
        exam_schedule_service::update_round(&pool, round, request(), actor).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let after = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(before.scheduled_sessions).unwrap(),
        serde_json::to_value(after.scheduled_sessions).unwrap()
    );
    assert!(serde_json::to_value(saved).unwrap().get("status").is_none());
}

#[tokio::test]
async fn current_duration_save_revalidates_the_existing_schedule_before_mutation() {
    let pool = migrated_pool("exam_current_duration_conflict").await;
    let round = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let before = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    let session = &before.scheduled_sessions[0];
    sqlx::query("UPDATE course_assessment_phases SET exam_duration_minutes=600 WHERE id=$1")
        .bind(session.assessment_phase_id)
        .execute(&pool)
        .await
        .unwrap();
    let workspace = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert!(workspace.readiness.findings.iter().any(|finding|finding.code==school_academic_assessment::exam_schedule::models::ExamScheduleReadinessCode::InvalidCurrentSources));
    assert!(
        exam_schedule_service::place_exam_session(
            &pool,
            PlaceExamSessionRequest {
                exam_schedule_item_id: session.exam_schedule_item_id,
                exam_day_id: session.exam_day_id,
                starts_at: session.starts_at
            },
            actor
        )
        .await
        .is_err()
    );
    let after = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(before.scheduled_sessions).unwrap(),
        serde_json::to_value(after.scheduled_sessions).unwrap(),
        "failed current-duration placement rolls back materialized duration and actual sessions"
    );
}

#[tokio::test]
async fn removing_a_placement_retires_its_item_but_keeps_round_room_and_seat_facts() {
    let pool = migrated_pool("exam_current_remove_placement").await;
    let round = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let before = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    let session = &before.scheduled_sessions[0];
    let rooms:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM academic_exam_day_room_assignments a WHERE exam_day_id=$1").bind(session.exam_day_id).fetch_one(&pool).await.unwrap();
    exam_schedule_service::delete_exam_session(&pool, session.id, actor)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_exam_schedule_items WHERE id=$1"
        )
        .bind(session.exam_schedule_item_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let after = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert_eq!(after.round.id, round);
    assert!(
        after
            .unscheduled_items
            .iter()
            .any(
                |item| item.assessment_phase_id == session.assessment_phase_id
                    && item.learning_group_id == session.learning_group_id
            )
    );
    let unchanged:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM academic_exam_day_room_assignments a WHERE exam_day_id=$1").bind(session.exam_day_id).fetch_one(&pool).await.unwrap();
    assert_eq!(rooms, unchanged);
}

#[tokio::test]
async fn current_student_view_requires_term_and_uses_group_roster() {
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

    let schedule = exam_schedule_service::list_my_exam_schedule(&pool, student_id, term_id)
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
    let removed = exam_schedule_service::list_my_exam_schedule(&pool, student_id, term_id)
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
    use school_academic_assessment::exam_schedule::models::{
        GenerateSeatsRequest, UpdateExamInvigilatorsRequest, UpdateExamRoundRequest,
    };
    use school_errors::AppError;

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
                    row_version: before["round"]["rowVersion"].as_i64().unwrap(),
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
        blocked(exam_schedule_service::delete_exam_session(&pool, session.id, actor).await);
        blocked(exam_schedule_service::delete_exam_day(&pool, day).await);
        blocked(exam_schedule_service::delete_round(&pool, round).await);
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

    assert!(matches!(error, school_errors::AppError::Conflict(_)));
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

    exam_schedule_service::delete_round(&pool, round_id)
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
async fn current_round_deletion_does_not_require_a_retired_publish_stage() {
    let pool = migrated_pool("exam_current_delete_round").await;
    let round = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    exam_schedule_service::delete_round(&pool, round)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_exam_rounds WHERE id=$1")
            .bind(round)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
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
    assert!(
        before
            .paper_receipt_items
            .iter()
            .all(|item| item.exam_arrangement == "in_timetable")
    );
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
    assert!(
        outside
            .paper_receipt_items
            .iter()
            .filter(|item| item.assessment_phase_id == phase_id)
            .all(|item| item.exam_arrangement == "outside_timetable")
    );
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
    assert!(
        no_exam
            .paper_receipt_items
            .iter()
            .all(|item| item.assessment_phase_id != phase_id)
    );

    // The round's phase and term are authoritative even if another phase has an exam.
    sqlx::query("UPDATE academic_exam_rounds SET exam_kind = 'final' WHERE id = $1")
        .bind(round_id)
        .execute(&pool)
        .await
        .unwrap();
    let final_round = exam_schedule_service::get_workspace(&pool, round_id)
        .await
        .unwrap();
    assert!(
        final_round
            .paper_receipt_items
            .iter()
            .all(|item| item.assessment_phase_id != phase_id)
    );
}

async fn exam_seat_pool(test_name: &str) -> sqlx::PgPool {
    let pool = migrated_pool(test_name).await;
    // The cutover fixture has two groups with different teachers, so it intentionally
    // leaves the shared assessment coordinator unset. Prepare one eligible exam group.
    sqlx::query(
        "UPDATE course_assessment_plans plan SET assessment_coordinator_id='50000000-0000-0000-0000-000000000002' \
         WHERE EXISTS (SELECT 1 FROM course_assessment_phases phase JOIN academic_exam_schedule_items item ON item.assessment_phase_id=phase.id WHERE phase.plan_id=plan.id)",
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "DELETE FROM learning_group_homerooms WHERE learning_group_id IN(SELECT id FROM learning_groups \
         WHERE academic_term_id=(SELECT academic_term_id FROM academic_exam_rounds WHERE id='84000000-0000-0000-0000-000000000001') \
         AND id NOT IN (SELECT learning_group_id FROM academic_exam_schedule_items))",
    ).execute(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn exam_seats_follow_class_numbers_across_direct_room_saves_and_current_reads() {
    let pool = exam_seat_pool("exam_seat_automatic_numbers").await;
    let round = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let day = Uuid::parse_str("85000000-0000-0000-0000-000000000001").unwrap();
    let homeroom = Uuid::parse_str("40000000-0000-0000-0000-000000000025").unwrap();
    let room = Uuid::parse_str("92000000-0000-0000-0000-000000000001").unwrap();
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let student = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
    sqlx::query(
        "UPDATE homeroom_placements SET class_number=12 WHERE homeroom_id=$1 AND status='current'",
    )
    .bind(homeroom)
    .execute(&pool)
    .await
    .unwrap();
    let request = || UpsertDayRoomAssignmentRequest {
        homeroom_id: homeroom,
        room_id: room,
        capacity_override: None,
        invigilator_staff_ids: None,
    };
    let assignment =
        exam_schedule_service::upsert_day_room_assignment(&pool, day, request(), actor)
            .await
            .unwrap();
    assert!(assignment.seats_generated);
    let seat: (Uuid, String) = sqlx::query_as("SELECT id,seat_number FROM academic_exam_seat_assignments WHERE day_room_assignment_id=$1 AND student_id=$2")
        .bind(assignment.id).bind(student).fetch_one(&pool).await.unwrap();
    assert_eq!(seat.1, "12");
    exam_schedule_service::upsert_day_room_assignment(&pool, day, request(), actor)
        .await
        .unwrap();
    let repeated: (Uuid, String) = sqlx::query_as("SELECT id,seat_number FROM academic_exam_seat_assignments WHERE day_room_assignment_id=$1 AND student_id=$2")
        .bind(assignment.id).bind(student).fetch_one(&pool).await.unwrap();
    assert_eq!(seat, repeated);

    let second_day = exam_schedule_service::upsert_exam_day(
        &pool,
        round,
        UpsertExamDayRequest {
            exam_date: NaiveDate::from_ymd_opt(2025, 9, 16).unwrap(),
            label: None,
            start_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
            grade_level_ids: vec![],
            blocked_windows: vec![],
        },
    )
    .await
    .unwrap();
    exam_schedule_service::upsert_day_room_assignment(&pool, second_day.id, request(), actor)
        .await
        .unwrap();
    sqlx::query("DELETE FROM academic_exam_seat_assignments WHERE student_id=$1")
        .bind(student)
        .execute(&pool)
        .await
        .unwrap();
    let workspace = exam_schedule_service::get_workspace(&pool, round)
        .await
        .unwrap();
    assert!(!workspace.readiness.complete);
    for exam_day in [day, second_day.id] {
        exam_schedule_service::upsert_day_room_assignment(&pool, exam_day, request(), actor)
            .await
            .unwrap();
    }
    let seats: Vec<String> = sqlx::query_scalar("SELECT seat.seat_number FROM academic_exam_seat_assignments seat JOIN academic_exam_day_room_assignments assignment ON assignment.id=seat.day_room_assignment_id JOIN academic_exam_days day ON day.id=assignment.exam_day_id WHERE day.exam_round_id=$1 AND seat.student_id=$2")
        .bind(round).bind(student).fetch_all(&pool).await.unwrap();
    assert_eq!(seats, vec!["12", "12"]);
    let personal = exam_schedule_service::list_my_exam_schedule(
        &pool,
        student,
        workspace.round.academic_term_id,
    )
    .await
    .unwrap();
    assert_eq!(personal[0].sessions[0].seat_number.as_deref(), Some("12"));

    sqlx::query(
        "UPDATE homeroom_placements SET class_number=27 WHERE homeroom_id=$1 AND status='current'",
    )
    .bind(homeroom)
    .execute(&pool)
    .await
    .unwrap();
    for exam_day in [day, second_day.id] {
        exam_schedule_service::upsert_day_room_assignment(&pool, exam_day, request(), actor)
            .await
            .unwrap();
    }
    let seats: Vec<String> = sqlx::query_scalar("SELECT seat.seat_number FROM academic_exam_seat_assignments seat JOIN academic_exam_day_room_assignments assignment ON assignment.id=seat.day_room_assignment_id JOIN academic_exam_days day ON day.id=assignment.exam_day_id WHERE day.exam_round_id=$1 AND seat.student_id=$2")
        .bind(round).bind(student).fetch_all(&pool).await.unwrap();
    assert_eq!(seats, vec!["27", "27"]);
}

async fn add_exam_seat_classmate(pool: &sqlx::PgPool, homeroom: Uuid, number: i32) -> Uuid {
    let student = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) VALUES($1,'synthetic','Test','Classmate','student','active')")
        .bind(student).execute(pool).await.unwrap();
    let year: Uuid = sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,$2,academic_year_id,grade_level_id,study_program_id,'active' FROM homerooms WHERE id=$3 RETURNING id")
        .bind(Uuid::new_v4()).bind(student).bind(homeroom).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type,class_number) SELECT $1,$2,academic_year_id,id,'2025-05-01','current','regular',$4 FROM homerooms WHERE id=$3")
        .bind(Uuid::new_v4()).bind(year).bind(homeroom).bind(number).execute(pool).await.unwrap();
    student
}

#[tokio::test]
async fn exam_seats_reject_invalid_numbers_atomically_and_handle_number_swaps() {
    use school_academic_assessment::exam_schedule::models::ExamScheduleReadinessCode;
    use school_errors::AppError;
    let pool = exam_seat_pool("exam_seat_invalid_numbers").await;
    let round = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let day = Uuid::parse_str("85000000-0000-0000-0000-000000000001").unwrap();
    let homeroom = Uuid::parse_str("40000000-0000-0000-0000-000000000025").unwrap();
    let room = Uuid::parse_str("92000000-0000-0000-0000-000000000001").unwrap();
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let first_student = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
    let second_student = add_exam_seat_classmate(&pool, homeroom, 3).await;
    let request = || UpsertDayRoomAssignmentRequest {
        homeroom_id: homeroom,
        room_id: room,
        capacity_override: None,
        invigilator_staff_ids: None,
    };
    let assignment =
        exam_schedule_service::upsert_day_room_assignment(&pool, day, request(), actor)
            .await
            .unwrap();
    let before: Vec<(Uuid, Uuid, String)> = sqlx::query_as("SELECT id,student_id,seat_number FROM academic_exam_seat_assignments WHERE day_room_assignment_id=$1 ORDER BY student_id")
        .bind(assignment.id).fetch_all(&pool).await.unwrap();
    for invalid in [None, Some(0), Some(-1), Some(1)] {
        sqlx::query("UPDATE homeroom_placements SET class_number=$2 WHERE homeroom_id=$1 AND student_academic_year_id IN (SELECT id FROM student_academic_years WHERE student_id=$3)")
            .bind(homeroom).bind(invalid).bind(second_student).execute(&pool).await.unwrap();
        let workspace = exam_schedule_service::get_workspace(&pool, round)
            .await
            .unwrap();
        assert!(!workspace.readiness.complete);
        assert!(
            workspace
                .readiness
                .findings
                .iter()
                .any(|finding| finding.code == ExamScheduleReadinessCode::MissingSeatAssignments)
        );
        let version = workspace.round.row_version;
        assert!(matches!(
            exam_schedule_service::generate_seats_for_assignment(
                &pool,
                assignment.id,
                school_academic_assessment::exam_schedule::models::GenerateSeatsRequest {
                    regenerate: true
                },
                actor
            )
            .await,
            Err(AppError::BadRequest(_))
        ));
        let mut changed = request();
        changed.capacity_override = Some(50);
        assert!(matches!(
            exam_schedule_service::upsert_day_room_assignment(&pool, day, changed, actor).await,
            Err(AppError::BadRequest(_))
        ));
        let after: Vec<(Uuid, Uuid, String)> = sqlx::query_as("SELECT id,student_id,seat_number FROM academic_exam_seat_assignments WHERE day_room_assignment_id=$1 ORDER BY student_id")
            .bind(assignment.id).fetch_all(&pool).await.unwrap();
        assert_eq!(before, after);
        let workspace = exam_schedule_service::get_workspace(&pool, round)
            .await
            .unwrap();
        assert_eq!(workspace.round.row_version, version);
        assert_eq!(
            workspace
                .days
                .iter()
                .flat_map(|day| &day.room_assignments)
                .find(|room| room.id == assignment.id)
                .unwrap()
                .capacity_override,
            None
        );
    }
    // Swapping two numbers does not collide with the unique seat-number constraint.
    sqlx::query("UPDATE homeroom_placements placement SET class_number=CASE student_year.student_id WHEN $2 THEN 3 ELSE 1 END FROM student_academic_years student_year WHERE student_year.id=placement.student_academic_year_id AND placement.homeroom_id=$1")
        .bind(homeroom).bind(first_student).execute(&pool).await.unwrap();
    exam_schedule_service::upsert_day_room_assignment(&pool, day, request(), actor)
        .await
        .unwrap();
    let seats: Vec<(Uuid, String)> = sqlx::query_as("SELECT student_id,seat_number FROM academic_exam_seat_assignments WHERE day_room_assignment_id=$1 ORDER BY seat_number")
        .bind(assignment.id).fetch_all(&pool).await.unwrap();
    assert_eq!(
        seats,
        vec![(second_student, "1".into()), (first_student, "3".into())]
    );
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(second_student)
        .execute(&pool)
        .await
        .unwrap();
    for exam_day in [day] {
        exam_schedule_service::upsert_day_room_assignment(&pool, exam_day, request(), actor)
            .await
            .unwrap();
    }
    let remaining: Vec<Uuid> = sqlx::query_scalar(
        "SELECT student_id FROM academic_exam_seat_assignments WHERE day_room_assignment_id=$1",
    )
    .bind(assignment.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, vec![first_student]);
}
