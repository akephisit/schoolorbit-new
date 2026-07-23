use std::collections::HashMap;

use chrono::NaiveTime;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    BlockedWindow, BlockedWindowInput, ExamInvigilatorView, UpdateExamRoundRequest,
    UpsertDayRoomAssignmentRequest,
};

use super::invigilation::{
    build_invigilator_candidate_session_windows, build_invigilator_staff_workloads,
    invigilator_staff_option_limit, invigilator_staff_option_search_pattern,
    invigilators_for_assignment, InvigilatorSessionWindowRow,
};
use super::room_assignments::{
    build_default_seat_assignments, validate_seat_generation_capacity, SeatStudent,
};
use super::rounds_and_days::{
    ensure_exam_round_is_mutable, normalize_blocked_windows, normalize_exam_kind,
    normalize_update_round_request,
};
use super::sessions_and_conflicts::grade_level_allowed_by_day_scope;
use super::shared::{
    add_minutes, exam_invigilator_staff_lock_keys, exam_session_conflict_lock_keys,
    has_invigilator_time_conflict, has_same_classroom_conflict, invigilator_workload_minutes,
    time_ranges_overlap, validate_session_window, CandidateSession, InvigilatorSessionWindow,
    SessionValidationError,
};
use super::workspace::{build_readiness, WorkspaceCounts, WORKSPACE_COUNTS_SQL};

fn t(value: &str) -> NaiveTime {
    NaiveTime::parse_from_str(value, "%H:%M").unwrap()
}

#[test]
fn room_assignment_payload_without_invigilators_preserves_existing_staff() {
    let request = serde_json::json!({
        "classroomId": Uuid::from_u128(1),
        "roomId": Uuid::from_u128(2),
        "capacityOverride": null
    });

    let parsed: UpsertDayRoomAssignmentRequest = serde_json::from_value(request).unwrap();

    assert_eq!(parsed.invigilator_staff_ids, None);
}

#[test]
fn room_assignment_payload_with_invigilators_remains_backwards_compatible() {
    let staff_id = Uuid::from_u128(3);
    let request = serde_json::json!({
        "classroomId": Uuid::from_u128(1),
        "roomId": Uuid::from_u128(2),
        "capacityOverride": null,
        "invigilatorStaffIds": [staff_id]
    });

    let parsed: UpsertDayRoomAssignmentRequest = serde_json::from_value(request).unwrap();

    assert_eq!(parsed.invigilator_staff_ids, Some(vec![staff_id]));
}

#[test]
fn invigilator_staff_option_limit_uses_bounded_default() {
    assert_eq!(invigilator_staff_option_limit(None), 40);
    assert_eq!(invigilator_staff_option_limit(Some(0)), 1);
    assert_eq!(invigilator_staff_option_limit(Some(250)), 100);
    assert_eq!(invigilator_staff_option_limit(Some(24)), 24);
}

#[test]
fn invigilator_staff_option_search_pattern_trims_empty_values() {
    assert_eq!(invigilator_staff_option_search_pattern(None), None);
    assert_eq!(
        invigilator_staff_option_search_pattern(Some("   ".to_string())),
        None
    );
    assert_eq!(
        invigilator_staff_option_search_pattern(Some("  Kru A  ".to_string())),
        Some("%Kru A%".to_string())
    );
}

#[test]
fn computes_end_time_from_duration() {
    assert_eq!(add_minutes(t("08:30"), 90).unwrap(), t("10:00"));
}

#[test]
fn rejects_zero_duration() {
    assert_eq!(
        add_minutes(t("08:30"), 0),
        Err(SessionValidationError::InvalidDuration)
    );
}

#[test]
fn rejects_negative_duration() {
    assert_eq!(
        add_minutes(t("08:30"), -30),
        Err(SessionValidationError::InvalidDuration)
    );
}

#[test]
fn rejects_end_time_overflow() {
    assert_eq!(
        add_minutes(t("23:30"), 60),
        Err(SessionValidationError::EndTimeOverflow)
    );
}

#[test]
fn detects_half_open_time_overlap() {
    assert!(time_ranges_overlap(
        t("08:30"),
        t("10:00"),
        t("09:59"),
        t("11:00")
    ));
    assert!(!time_ranges_overlap(
        t("08:30"),
        t("10:00"),
        t("10:00"),
        t("11:00")
    ));
}

#[test]
fn detects_classroom_time_conflict() {
    let candidate = CandidateSession {
        session_id: None,
        classroom_id: Uuid::nil(),
        exam_day_id: Uuid::nil(),
        starts_at: t("09:00"),
        ends_at: t("10:00"),
    };
    let existing = vec![CandidateSession {
        session_id: Some(Uuid::max()),
        classroom_id: Uuid::nil(),
        exam_day_id: Uuid::nil(),
        starts_at: t("09:30"),
        ends_at: t("10:30"),
    }];
    assert!(has_same_classroom_conflict(&candidate, &existing));
}

#[test]
fn invigilator_workload_sums_session_minutes_without_gaps() {
    let assignment_id = Uuid::from_u128(1);
    let staff_id = Uuid::from_u128(2);
    let windows = vec![
        InvigilatorSessionWindow {
            assignment_id,
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("08:30"),
            ends_at: t("09:30"),
        },
        InvigilatorSessionWindow {
            assignment_id,
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("10:00"),
            ends_at: t("11:30"),
        },
    ];

    let minutes = invigilator_workload_minutes(&windows);

    assert_eq!(minutes, 150);
}

#[test]
fn invigilator_staff_workloads_group_by_staff_and_day() {
    let staff_id = Uuid::from_u128(7);
    let exam_day_id = Uuid::from_u128(10);
    let rows = vec![
        InvigilatorSessionWindowRow {
            assignment_id: Uuid::from_u128(1),
            exam_day_id,
            staff_id,
            staff_name: "Teacher One".to_string(),
            starts_at: t("08:30"),
            ends_at: t("09:30"),
        },
        InvigilatorSessionWindowRow {
            assignment_id: Uuid::from_u128(2),
            exam_day_id,
            staff_id,
            staff_name: "Teacher One".to_string(),
            starts_at: t("10:00"),
            ends_at: t("11:30"),
        },
    ];

    let workloads = build_invigilator_staff_workloads(rows);

    assert_eq!(workloads.len(), 1);
    assert_eq!(workloads[0].staff_id, staff_id);
    assert_eq!(workloads[0].staff_name, "Teacher One");
    assert_eq!(workloads[0].total_minutes, 150);
    assert_eq!(workloads[0].assigned_day_count, 1);
    assert_eq!(workloads[0].assignment_count, 2);
    assert_eq!(workloads[0].days.len(), 1);
    assert_eq!(workloads[0].days[0].exam_day_id, exam_day_id);
    assert_eq!(workloads[0].days[0].minutes, 150);
    assert_eq!(workloads[0].days[0].assignment_count, 2);
}

#[test]
fn invigilator_conflict_rejects_overlapping_live_session_ranges() {
    let staff_id = Uuid::from_u128(7);
    let candidate = vec![InvigilatorSessionWindow {
        assignment_id: Uuid::from_u128(1),
        exam_day_id: Uuid::from_u128(10),
        staff_id,
        starts_at: t("08:30"),
        ends_at: t("09:30"),
    }];
    let existing = vec![InvigilatorSessionWindow {
        assignment_id: Uuid::from_u128(2),
        exam_day_id: Uuid::from_u128(10),
        staff_id,
        starts_at: t("09:00"),
        ends_at: t("10:00"),
    }];

    assert!(has_invigilator_time_conflict(
        Uuid::from_u128(1),
        &candidate,
        &existing
    ));
}

#[test]
fn invigilator_conflict_allows_non_overlapping_same_day_assignments() {
    let staff_id = Uuid::from_u128(7);
    let candidate = vec![InvigilatorSessionWindow {
        assignment_id: Uuid::from_u128(1),
        exam_day_id: Uuid::from_u128(10),
        staff_id,
        starts_at: t("08:30"),
        ends_at: t("09:30"),
    }];
    let existing = vec![InvigilatorSessionWindow {
        assignment_id: Uuid::from_u128(2),
        exam_day_id: Uuid::from_u128(10),
        staff_id,
        starts_at: t("09:30"),
        ends_at: t("10:30"),
    }];

    assert!(!has_invigilator_time_conflict(
        Uuid::from_u128(1),
        &candidate,
        &existing
    ));
}

#[test]
fn exam_invigilator_staff_lock_keys_are_sorted_deduped_and_stable() {
    let exam_day_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();
    let staff_a = Uuid::parse_str("11121314-1516-1718-191a-1b1c1d1e1f20").unwrap();
    let staff_b = Uuid::parse_str("21222324-2526-2728-292a-2b2c2d2e2f30").unwrap();

    let keys = exam_invigilator_staff_lock_keys(exam_day_id, &[staff_b, staff_a, staff_a]);

    assert_eq!(
        keys,
        exam_invigilator_staff_lock_keys(exam_day_id, &[staff_a, staff_b])
    );
    assert_eq!(keys.len(), 2);
    assert!(keys[0] < keys[1]);
}

#[test]
fn assign_invigilator_to_assignment_uses_day_level_move_semantics() {
    let source = include_str!("invigilation.rs");
    let start = source
        .find("pub async fn assign_invigilator_to_assignment")
        .expect("assign service should exist");
    let body = &source[start
        ..source[start..]
            .find("pub async fn remove_invigilator_from_assignment")
            .map(|index| start + index)
            .unwrap_or(source.len())];

    let lock_position = body
        .find("lock_exam_invigilator_staff_conflict_scope")
        .expect("assign service should lock staff/day scope");
    let validate_position = body
        .find("validate_active_staff_users")
        .expect("assign service should validate active staff");
    let delete_position = body
        .find("delete_staff_invigilator_from_other_day_assignments_in_tx")
        .expect("assign service should remove staff from other rooms on the same day");
    let insert_position = body
        .find("insert_staff_invigilator_if_missing_in_tx")
        .expect("assign service should insert target staff");

    assert!(lock_position < validate_position);
    assert!(validate_position < delete_position);
    assert!(delete_position < insert_position);
    assert!(body.contains("ensure_exam_round_is_mutable"));
    assert!(body.contains("get_invigilator_workspace(pool, round_id)"));
}

#[test]
fn remove_invigilator_from_assignment_only_deletes_target_assignment() {
    let source = include_str!("invigilation.rs");
    let start = source
        .find("pub async fn remove_invigilator_from_assignment")
        .expect("remove service should exist");
    let body = &source[start
        ..source[start..]
            .find("async fn replace_assignment_invigilators_in_tx")
            .map(|index| start + index)
            .unwrap_or(source.len())];

    assert!(body.contains("delete_staff_invigilator_from_assignment_in_tx"));
    assert!(!body.contains("delete_staff_invigilator_from_other_day_assignments_in_tx"));
    assert!(body.contains("ensure_exam_round_is_mutable"));
    assert!(body.contains("get_invigilator_workspace(pool, round_id)"));
}

#[test]
fn exam_round_mutation_guard_rejects_published_rounds() {
    assert!(ensure_exam_round_is_mutable("draft").is_ok());
    assert!(ensure_exam_round_is_mutable("published").is_err());
}

#[test]
fn academic_routes_expose_staff_level_invigilator_actions() {
    let source = include_str!("../../../academic.rs");

    assert!(
        source.contains("/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}")
    );
    assert!(source.contains("assign_assignment_invigilator"));
    assert!(source.contains("remove_assignment_invigilator"));
}

#[test]
fn exam_schedule_handler_uses_staff_level_invigilator_services() {
    let source = include_str!("../../handlers/exam_schedule.rs");

    assert!(source.contains("pub async fn assign_assignment_invigilator"));
    assert!(source.contains("pub async fn remove_assignment_invigilator"));
    assert!(source.contains("exam_schedule_service::assign_invigilator_to_assignment"));
    assert!(source.contains("exam_schedule_service::remove_invigilator_from_assignment"));
    assert!(source.contains("Path((assignment_id, staff_id)): Path<(Uuid, Uuid)>"));
}

#[test]
fn exam_day_update_preserves_day_identity_and_child_assignments() {
    let source = include_str!("rounds_and_days.rs");
    let update_start = source.find("pub async fn update_exam_day").unwrap();
    let update_tail = &source[update_start..];
    let update_end = update_tail.find("pub async fn delete_exam_day").unwrap();
    let update_body = &update_tail[..update_end];

    assert!(update_body.contains("UPDATE academic_exam_days"));
    assert!(update_body.contains("WHERE id = $1"));
    assert!(update_body.contains("replace_exam_day_configuration"));
    assert!(update_body.contains("mark_round_draft_after_mutation"));
    assert!(!update_body.contains("DELETE FROM academic_exam_days"));
    assert!(!update_body.contains("academic_exam_sessions"));
    assert!(!update_body.contains("academic_exam_day_room_assignments"));
    assert!(!update_body.contains("academic_exam_day_invigilators"));
    assert!(!update_body.contains("academic_exam_seat_assignments"));
}

#[test]
fn exam_day_update_maps_occupied_dates_to_actionable_error() {
    let source = include_str!("rounds_and_days.rs");

    assert!(source.contains("map_err(map_exam_day_write_error)"));
    assert!(source.contains("กรุณาย้ายวันนั้นไปวันที่ว่างก่อน"));
}

#[test]
fn update_assignment_invigilators_locks_staff_scope_before_conflict_validation() {
    let source = include_str!("invigilation.rs");
    let update_start = source
        .find("pub async fn update_assignment_invigilators")
        .unwrap();
    let update_body = &source[update_start..];
    let lock_position = update_body
        .find("lock_exam_invigilator_staff_conflict_scope")
        .unwrap();
    let validation_position = update_body
        .find("validate_invigilator_time_conflicts")
        .unwrap();

    assert!(lock_position < validation_position);
}

#[test]
fn upsert_day_room_assignment_locks_optional_invigilator_scope_before_conflict_validation() {
    let source = include_str!("room_assignments.rs");
    let upsert_start = source
        .find("pub async fn upsert_day_room_assignment")
        .unwrap();
    let upsert_tail = &source[upsert_start..];
    let update_start = upsert_tail
        .find("pub async fn generate_seats_for_assignment")
        .unwrap();
    let upsert_body = &upsert_tail[..update_start];
    let lock_position = upsert_body
        .find("lock_exam_invigilator_staff_conflict_scope")
        .unwrap();
    let validation_position = upsert_body
        .find("validate_invigilator_time_conflicts")
        .unwrap();

    assert!(lock_position < validation_position);
}

#[test]
fn place_exam_session_locks_and_validates_invigilators_before_insert() {
    let source = include_str!("sessions_and_conflicts.rs");
    let placement_start = source.find("pub async fn place_exam_session").unwrap();
    let placement_body = &source[placement_start..];
    let lock_position = placement_body
        .find("lock_exam_invigilator_staff_conflict_scope")
        .unwrap();
    let validation_position = placement_body
        .find("validate_invigilator_candidate_session_conflicts")
        .unwrap();
    let insert_position = placement_body
        .find("INSERT INTO academic_exam_sessions")
        .unwrap();

    assert!(lock_position < validation_position);
    assert!(validation_position < insert_position);
}

#[test]
fn builds_invigilator_candidate_session_windows_for_each_staff_member() {
    let assignment_id = Uuid::from_u128(1);
    let exam_day_id = Uuid::from_u128(2);
    let staff_a = Uuid::from_u128(3);
    let staff_b = Uuid::from_u128(4);

    let windows = build_invigilator_candidate_session_windows(
        assignment_id,
        exam_day_id,
        t("08:30"),
        t("09:30"),
        &[staff_a, staff_b],
    );

    assert_eq!(
        windows,
        vec![
            InvigilatorSessionWindow {
                assignment_id,
                exam_day_id,
                staff_id: staff_a,
                starts_at: t("08:30"),
                ends_at: t("09:30"),
            },
            InvigilatorSessionWindow {
                assignment_id,
                exam_day_id,
                staff_id: staff_b,
                starts_at: t("08:30"),
                ends_at: t("09:30"),
            },
        ]
    );
}

#[test]
fn get_invigilator_workspace_checks_round_before_assignment_queries() {
    let source = include_str!("invigilation.rs");
    let workspace_start = source
        .find("pub async fn get_invigilator_workspace")
        .unwrap();
    let workspace_body = &source[workspace_start..];
    let round_position = workspace_body
        .find("fetch_round(pool, round_id).await?")
        .unwrap();
    let assignments_position = workspace_body
        .find("fetch_invigilator_assignment_summaries")
        .unwrap();

    assert!(round_position < assignments_position);
}

#[test]
fn import_exam_items_filters_source_categories_by_round_kind() {
    let source = include_str!("workspace.rs");
    let import_start = source.find("pub async fn import_exam_items").unwrap();
    let import_tail = &source[import_start..];
    let next_function_start = import_tail
        .find("pub async fn clear_mismatched_exam_items")
        .unwrap();
    let import_body = &import_tail[..next_function_start];

    assert!(import_body.contains("exam_kind"));
    assert_eq!(
        import_body.matches("c.code = rc.exam_kind").count(),
        3,
        "existing, missing-duration, and insert source queries must filter by round kind"
    );
}

#[test]
fn clear_mismatched_exam_items_deletes_only_items_outside_round_kind() {
    let source = include_str!("workspace.rs");
    let clear_start = source
        .find("pub async fn clear_mismatched_exam_items")
        .unwrap();
    let clear_tail = &source[clear_start..];
    let next_function_start = clear_tail
        .find("pub(super) async fn fetch_workspace_counts_in_tx")
        .unwrap();
    let clear_body = &clear_tail[..next_function_start];

    assert!(clear_body.contains("SELECT status"));
    assert!(clear_body.contains("FOR UPDATE"));
    assert!(clear_body.contains("DELETE FROM academic_exam_schedule_items"));
    assert!(clear_body.contains("USING academic_assessment_categories c"));
    assert!(clear_body.contains("round_context rc"));
    assert!(clear_body.contains("item.assessment_category_id = c.id"));
    assert!(clear_body.contains("c.code IS DISTINCT FROM rc.exam_kind"));
    assert!(clear_body.contains("mark_round_draft_after_mutation"));
}

#[test]
fn publish_round_locks_round_before_readiness_check() {
    let source = include_str!("publishing.rs");
    let publish_start = source.find("pub async fn publish_round").unwrap();
    let publish_body = &source[publish_start..];
    let tx_position = publish_body
        .find("let mut tx = pool.begin().await?")
        .unwrap();
    let lock_position = publish_body.find("FOR UPDATE").unwrap();
    let readiness_position = publish_body.find("fetch_workspace_counts_in_tx").unwrap();
    let update_position = publish_body.find("UPDATE academic_exam_rounds").unwrap();

    assert!(tx_position < lock_position);
    assert!(lock_position < readiness_position);
    assert!(readiness_position < update_position);
}

#[test]
fn placement_locks_conflict_scope_before_conflict_queries() {
    let source = include_str!("sessions_and_conflicts.rs");
    let placement_start = source.find("pub async fn place_exam_session").unwrap();
    let placement_body = &source[placement_start..];
    let lock_position = placement_body
        .find("lock_exam_session_conflict_scope")
        .unwrap();
    let classroom_conflict_query_position = placement_body
        .find("fetch_candidate_sessions_for_day")
        .unwrap();
    let room_conflict_query_position = placement_body
        .find("fetch_candidate_room_sessions_for_day")
        .unwrap();

    assert!(lock_position < classroom_conflict_query_position);
    assert!(lock_position < room_conflict_query_position);
}

#[test]
fn exam_session_conflict_lock_keys_are_sorted_and_scoped() {
    let exam_day_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();
    let classroom_id = Uuid::parse_str("11121314-1516-1718-191a-1b1c1d1e1f20").unwrap();
    let room_id = Uuid::parse_str("21222324-2526-2728-292a-2b2c2d2e2f30").unwrap();

    let keys = exam_session_conflict_lock_keys(exam_day_id, classroom_id, room_id);

    assert_eq!(
        keys,
        exam_session_conflict_lock_keys(exam_day_id, classroom_id, room_id)
    );
    assert!(keys[0] < keys[1]);
}

#[test]
fn rejects_placement_outside_day_window() {
    let outcome = validate_session_window(
        t("08:00"),
        120,
        t("08:30"),
        t("16:00"),
        &[BlockedWindow {
            id: None,
            label: "Lunch".to_string(),
            start_time: t("12:00"),
            end_time: t("13:00"),
        }],
    );
    assert!(matches!(
        outcome,
        Err(SessionValidationError::BeforeDayStart)
    ));
}

#[test]
fn rejects_placement_after_day_end() {
    let outcome = validate_session_window(t("15:30"), 60, t("08:30"), t("16:00"), &[]);
    assert!(matches!(outcome, Err(SessionValidationError::AfterDayEnd)));
}

#[test]
fn rejects_placement_across_blocked_window() {
    let outcome = validate_session_window(
        t("11:30"),
        90,
        t("08:30"),
        t("16:00"),
        &[BlockedWindow {
            id: None,
            label: "Lunch".to_string(),
            start_time: t("12:00"),
            end_time: t("13:00"),
        }],
    );
    assert!(matches!(
        outcome,
        Err(SessionValidationError::BlockedWindow(_))
    ));
}

#[test]
fn accepts_placement_start_time_on_5_minute_slot() {
    let outcome = validate_session_window(t("08:35"), 60, t("08:30"), t("16:00"), &[]);

    assert!(outcome.is_ok());
}

#[test]
fn rejects_placement_start_time_outside_5_minute_slot() {
    let outcome = validate_session_window(t("08:37"), 60, t("08:30"), t("16:00"), &[]);

    assert!(outcome.is_err());
}

#[test]
fn empty_day_grade_scope_allows_any_grade_level() {
    assert!(grade_level_allowed_by_day_scope(Uuid::nil(), &[]));
}

#[test]
fn explicit_day_grade_scope_rejects_removed_grade_level() {
    assert!(!grade_level_allowed_by_day_scope(
        Uuid::from_u128(1),
        &[Uuid::from_u128(2)]
    ));
}

#[test]
fn readiness_sql_rechecks_sessions_after_day_window_changes() {
    assert!(WORKSPACE_COUNTS_SQL.contains("invalid_session_count"));
    assert!(WORKSPACE_COUNTS_SQL.contains("session.starts_at < day.start_time"));
    assert!(WORKSPACE_COUNTS_SQL.contains("session.ends_at > day.end_time"));
}

#[test]
fn readiness_sql_uses_same_five_minute_slot_as_placement_validation() {
    assert!(WORKSPACE_COUNTS_SQL.contains("% 300"));
    assert!(!WORKSPACE_COUNTS_SQL.contains("% 900"));
}

#[test]
fn readiness_sql_rechecks_sessions_after_blocked_window_changes() {
    assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_blocked_windows blocked"));
    assert!(WORKSPACE_COUNTS_SQL.contains("session.starts_at < blocked.end_time"));
    assert!(WORKSPACE_COUNTS_SQL.contains("blocked.start_time < session.ends_at"));
}

#[test]
fn readiness_sql_rechecks_sessions_after_grade_scope_changes() {
    assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_grade_levels scope"));
    assert!(WORKSPACE_COUNTS_SQL.contains("scope.grade_level_id = item.grade_level_id"));
}

#[test]
fn readiness_sql_requires_seats_for_every_active_student() {
    assert!(WORKSPACE_COUNTS_SQL.contains("missing_seat_student_count"));
    assert!(WORKSPACE_COUNTS_SQL.contains("student_class_enrollments enrollment"));
    assert!(WORKSPACE_COUNTS_SQL.contains("seat.student_id IS NULL"));
}

#[test]
fn readiness_sql_counts_invigilator_live_range_conflicts() {
    assert!(WORKSPACE_COUNTS_SQL.contains("invigilator_conflict_count"));
    assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_invigilators"));
    assert!(WORKSPACE_COUNTS_SQL.contains("left_session.starts_at < right_session.ends_at"));
    assert!(WORKSPACE_COUNTS_SQL.contains("right_session.starts_at < left_session.ends_at"));
}

#[test]
fn day_staff_unique_error_mapping_is_removed_after_live_range_migration() {
    let source = include_str!("room_assignments.rs");
    let mapping_start = source
        .find("fn map_day_room_assignment_write_error")
        .unwrap();
    let mapping_body = &source[mapping_start..];

    assert!(!mapping_body.contains("exam_day_id_staff_id"));
}

#[test]
fn readiness_requires_days_items_rooms_and_sessions() {
    let readiness = build_readiness(WorkspaceCounts {
        day_count: 0,
        item_count: 4,
        unscheduled_count: 4,
        missing_room_assignment_count: 2,
        invalid_session_count: 0,
        missing_seat_student_count: 2,
        invigilator_conflict_count: 0,
    });
    assert!(!readiness.can_publish);
    assert!(readiness
        .blockers
        .iter()
        .any(|value| value.contains("exam day")));
    assert!(readiness
        .blockers
        .iter()
        .any(|value| value.contains("unscheduled")));
}

#[test]
fn readiness_reports_missing_active_student_seats() {
    let readiness = build_readiness(WorkspaceCounts {
        day_count: 1,
        item_count: 4,
        unscheduled_count: 0,
        missing_room_assignment_count: 0,
        invalid_session_count: 0,
        missing_seat_student_count: 3,
        invigilator_conflict_count: 0,
    });

    assert!(!readiness.can_publish);
    assert!(readiness
        .blockers
        .iter()
        .any(|value| value.contains("active student")));
}

#[test]
fn readiness_reports_invalid_scheduled_sessions() {
    let readiness = build_readiness(WorkspaceCounts {
        day_count: 1,
        item_count: 4,
        unscheduled_count: 0,
        missing_room_assignment_count: 0,
        invalid_session_count: 2,
        missing_seat_student_count: 0,
        invigilator_conflict_count: 0,
    });

    assert!(!readiness.can_publish);
    assert!(readiness
        .blockers
        .iter()
        .any(|value| value.contains("no longer fit")));
}

#[test]
fn readiness_reports_invigilator_live_range_conflicts() {
    let readiness = build_readiness(WorkspaceCounts {
        day_count: 1,
        item_count: 4,
        unscheduled_count: 0,
        missing_room_assignment_count: 0,
        invalid_session_count: 0,
        missing_seat_student_count: 0,
        invigilator_conflict_count: 2,
    });

    assert!(!readiness.can_publish);
    assert!(readiness
        .blockers
        .iter()
        .any(|value| value.contains("overlapping invigilator")));
}

#[test]
fn rejects_round_update_without_supplied_fields() {
    let result = normalize_update_round_request(UpdateExamRoundRequest {
        name: None,
        description: None,
        exam_kind: None,
    });

    assert!(matches!(
        result,
        Err(AppError::BadRequest(message)) if message.contains("No fields")
    ));
}

#[test]
fn normalizes_supported_exam_round_kinds() {
    assert_eq!(normalize_exam_kind(None).unwrap(), "midterm");
    assert_eq!(normalize_exam_kind(Some(" final ")).unwrap(), "final");

    assert!(matches!(
        normalize_exam_kind(Some("quiz")),
        Err(AppError::BadRequest(message)) if message.contains("midterm or final")
    ));
}

#[test]
fn rejects_blocked_windows_outside_exam_day_range() {
    let result = normalize_blocked_windows(
        t("08:30"),
        t("16:00"),
        vec![BlockedWindowInput {
            label: "Before school".to_string(),
            start_time: t("08:00"),
            end_time: t("08:45"),
        }],
    );

    assert!(matches!(
        result,
        Err(AppError::BadRequest(message)) if message.contains("within the exam day")
    ));
}

#[test]
fn shared_assignment_invigilators_are_reused_for_each_session() {
    let assignment_id = Uuid::from_u128(1);
    let invigilator = ExamInvigilatorView {
        id: Uuid::from_u128(2),
        exam_day_id: Uuid::from_u128(3),
        day_room_assignment_id: assignment_id,
        staff_id: Uuid::from_u128(4),
        staff_name: Some("Exam Staff".to_string()),
        role_label: Some("Lead".to_string()),
    };
    let invigilators_by_assignment = HashMap::from([(assignment_id, vec![invigilator.clone()])]);

    let first = invigilators_for_assignment(Some(assignment_id), &invigilators_by_assignment);
    let second = invigilators_for_assignment(Some(assignment_id), &invigilators_by_assignment);

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(first[0].id, invigilator.id);
    assert_eq!(second[0].id, invigilator.id);
    assert!(invigilators_by_assignment.contains_key(&assignment_id));
}

#[test]
fn generates_padded_seat_numbers_in_input_order() {
    let students = vec![
        SeatStudent {
            student_id: Uuid::nil(),
        },
        SeatStudent {
            student_id: Uuid::max(),
        },
    ];
    let seats = build_default_seat_assignments(&students);
    assert_eq!(seats[0].seat_number, "01");
    assert_eq!(seats[1].seat_number, "02");
}

#[test]
fn rejects_seat_generation_when_student_count_exceeds_capacity() {
    let result = validate_seat_generation_capacity(41, 40);

    assert!(matches!(
        result,
        Err(AppError::BadRequest(message)) if message.contains("exceeds")
    ));
}

#[test]
fn rejects_seat_generation_when_effective_capacity_is_not_positive() {
    let result = validate_seat_generation_capacity(0, 0);

    assert!(matches!(
        result,
        Err(AppError::BadRequest(message)) if message.contains("greater than zero")
    ));
}
