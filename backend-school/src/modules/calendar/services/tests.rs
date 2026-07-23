use super::*;

#[test]
fn validate_event_date_time_rejects_end_date_before_start() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 7, 9).unwrap();

    assert!(validate_event_date_time(start, end, true, None, None).is_err());
}

#[test]
fn validate_event_date_time_rejects_same_day_end_time_before_start_time() {
    let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let start_time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();
    let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

    assert!(validate_event_date_time(date, date, false, Some(start_time), Some(end_time)).is_err());
}

#[test]
fn validate_event_date_time_rejects_same_day_equal_start_and_end_time() {
    let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();

    assert!(validate_event_date_time(date, date, false, Some(time), Some(time)).is_err());
}

#[test]
fn validate_event_date_time_accepts_multi_day_timed_event() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 7, 12).unwrap();
    let start_time = NaiveTime::from_hms_opt(15, 0, 0).unwrap();
    let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

    assert!(validate_event_date_time(start, end, false, Some(start_time), Some(end_time)).is_ok());
}

#[test]
fn reminder_dates_are_day_based_and_sorted() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let result = reminder_dates(start, &[1, 7, 3]).unwrap();

    assert_eq!(
        result,
        vec![
            NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
        ]
    );
}

#[test]
fn reminder_dates_dedupes_duplicate_offsets() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let result = reminder_dates(start, &[3, 1, 3, 1]).unwrap();

    assert_eq!(
        result,
        vec![
            NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
        ]
    );
}

#[test]
fn reminder_schedule_pairs_dedupe_offsets_and_sort_by_reminder_date() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let result = reminder_schedule(start, &[1, 7, 1, 3]).unwrap();

    assert_eq!(
        result,
        vec![
            (7, NaiveDate::from_ymd_opt(2026, 7, 3).unwrap()),
            (3, NaiveDate::from_ymd_opt(2026, 7, 7).unwrap()),
            (1, NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()),
        ]
    );
}

#[test]
fn reminder_schedule_pairs_reject_overflowing_offsets() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

    assert!(reminder_schedule(start, &[i32::MAX]).is_err());
}

#[test]
fn reminder_dates_reject_zero_or_negative_offsets() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

    assert!(reminder_dates(start, &[0]).is_err());
    assert!(reminder_dates(start, &[-1]).is_err());
}

#[test]
fn reminder_dates_rejects_offsets_outside_representable_date_range() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

    assert!(reminder_dates(start, &[i32::MAX]).is_err());
}

#[test]
fn validate_targets_rejects_empty_targets() {
    assert!(validate_targets(&[]).is_err());
}

#[test]
fn validate_targets_accepts_student_grade_target() {
    let grade_level_id = Uuid::new_v4();
    let targets = vec![CalendarEventTargetInput {
        audience_type: CalendarAudienceType::Student,
        grade_level_id: Some(grade_level_id),
        class_room_id: None,
    }];

    assert!(validate_targets(&targets).is_ok());
}

#[test]
fn validate_targets_rejects_student_or_parent_with_grade_and_classroom_filter() {
    let targets = vec![CalendarEventTargetInput {
        audience_type: CalendarAudienceType::Parent,
        grade_level_id: Some(Uuid::new_v4()),
        class_room_id: Some(Uuid::new_v4()),
    }];

    assert!(validate_targets(&targets).is_err());
}

#[test]
fn validate_targets_rejects_all_with_grade_or_classroom_filter() {
    let grade_targets = vec![CalendarEventTargetInput {
        audience_type: CalendarAudienceType::All,
        grade_level_id: Some(Uuid::new_v4()),
        class_room_id: None,
    }];
    let classroom_targets = vec![CalendarEventTargetInput {
        audience_type: CalendarAudienceType::All,
        grade_level_id: None,
        class_room_id: Some(Uuid::new_v4()),
    }];

    assert!(validate_targets(&grade_targets).is_err());
    assert!(validate_targets(&classroom_targets).is_err());
}

#[test]
fn target_visible_to_user_type_allows_all_and_matching_user_type_only() {
    assert!(target_visible_to_user_type("all", "staff"));
    assert!(target_visible_to_user_type("all", "student"));
    assert!(!target_visible_to_user_type("all", "parent"));
    assert!(!target_visible_to_user_type("parent", "parent"));
    assert!(!target_visible_to_user_type("student", "parent"));
    assert!(!target_visible_to_user_type("parent", "student"));
}

#[test]
fn self_calendar_user_type_access_rejects_parent_users() {
    assert!(self_calendar_user_type_access("staff").is_ok());
    assert!(self_calendar_user_type_access("student").is_ok());
    assert!(matches!(
        self_calendar_user_type_access("parent"),
        Err(AppError::Forbidden(message))
            if message.contains("/api/parent/students/{student_id}/calendar/events")
    ));
}

#[test]
fn target_visible_to_child_view_allows_all_and_parent_only() {
    assert!(target_visible_to_child_view("all"));
    assert!(target_visible_to_child_view("parent"));
    assert!(!target_visible_to_child_view("student"));
    assert!(!target_visible_to_child_view("staff"));
}

#[test]
fn validate_targets_rejects_duplicate_global_targets() {
    let targets = vec![
        CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Student,
            grade_level_id: None,
            class_room_id: None,
        },
        CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Student,
            grade_level_id: None,
            class_room_id: None,
        },
    ];

    assert!(validate_targets(&targets).is_err());
}

#[test]
fn validate_targets_rejects_duplicate_grade_targets() {
    let grade_level_id = Uuid::new_v4();
    let targets = vec![
        CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Parent,
            grade_level_id: Some(grade_level_id),
            class_room_id: None,
        },
        CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Parent,
            grade_level_id: Some(grade_level_id),
            class_room_id: None,
        },
    ];

    assert!(validate_targets(&targets).is_err());
}

#[test]
fn validate_targets_rejects_duplicate_classroom_targets() {
    let class_room_id = Uuid::new_v4();
    let targets = vec![
        CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Student,
            grade_level_id: None,
            class_room_id: Some(class_room_id),
        },
        CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Student,
            grade_level_id: None,
            class_room_id: Some(class_room_id),
        },
    ];

    assert!(validate_targets(&targets).is_err());
}

#[test]
fn validate_targets_rejects_staff_with_classroom_filter() {
    let targets = vec![CalendarEventTargetInput {
        audience_type: CalendarAudienceType::Staff,
        grade_level_id: None,
        class_room_id: Some(Uuid::new_v4()),
    }];

    assert!(validate_targets(&targets).is_err());
}

#[test]
fn dedupe_user_ids_preserves_first_seen_order() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    assert_eq!(dedupe_user_ids(vec![a, b, a]), vec![a, b]);
}

#[test]
fn dedupe_uuid_ids_keeps_each_tag_once_in_selection_order() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    assert_eq!(
        dedupe_uuid_ids(&[first, second, first]),
        vec![first, second]
    );
}

#[test]
fn normalized_tag_name_trims_and_rejects_blank_values() {
    assert_eq!(
        normalized_tag_name("  กิจกรรมเด่น  ".to_string()).unwrap(),
        "กิจกรรมเด่น"
    );
    assert!(normalized_tag_name("   ".to_string()).is_err());
}

#[test]
fn normalized_event_range_uses_complete_query_range() {
    let from = NaiveDate::from_ymd_opt(2026, 5, 2).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let query = CalendarEventQuery {
        from: Some(from),
        to: Some(to),
        category_id: None,
        tag_id: None,
        audience: None,
        visibility: None,
        q: None,
    };

    assert_eq!(normalized_event_range(&query, today).unwrap(), (from, to));
}

#[test]
fn normalized_event_range_rejects_reversed_complete_query_range() {
    let from = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 5, 2).unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let query = CalendarEventQuery {
        from: Some(from),
        to: Some(to),
        category_id: None,
        tag_id: None,
        audience: None,
        visibility: None,
        q: None,
    };

    assert!(matches!(
        normalized_event_range(&query, today),
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn normalized_event_range_defaults_to_current_month_when_bound_is_missing() {
    let today = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
    let query = CalendarEventQuery {
        from: Some(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
        to: None,
        category_id: None,
        tag_id: None,
        audience: None,
        visibility: None,
        q: None,
    };

    assert_eq!(
        normalized_event_range(&query, today).unwrap(),
        (
            NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(),
        )
    );
}

#[test]
fn normalized_event_range_handles_december_current_month() {
    let today = NaiveDate::from_ymd_opt(2026, 12, 15).unwrap();
    let query = CalendarEventQuery {
        from: None,
        to: None,
        category_id: None,
        tag_id: None,
        audience: None,
        visibility: None,
        q: None,
    };

    assert_eq!(
        normalized_event_range(&query, today).unwrap(),
        (
            NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        )
    );
}

#[test]
fn calendar_search_pattern_escapes_like_wildcards() {
    assert_eq!(
        calendar_search_pattern(" 100%_activity\\plan "),
        "%100\\%\\_activity\\\\plan%"
    );
}

#[test]
fn calendar_notification_text_formats_created_updated_and_reminder_messages() {
    let event = calendar_event_for_notification("สอบกลางภาค");

    assert_eq!(
        calendar_notification_text(&event, &CalendarNotificationKind::Created),
        (
            "เพิ่มกำหนดการ: สอบกลางภาค".to_string(),
            "มีกำหนดการใหม่ในปฏิทินโรงเรียน".to_string(),
        )
    );
    assert_eq!(
        calendar_notification_text(&event, &CalendarNotificationKind::Updated),
        (
            "อัปเดตกำหนดการ: สอบกลางภาค".to_string(),
            "มีการเปลี่ยนแปลงกำหนดการในปฏิทินโรงเรียน".to_string(),
        )
    );
    assert_eq!(
        calendar_notification_text(
            &event,
            &CalendarNotificationKind::Reminder { days_before: 3 },
        ),
        (
            "เตือนล่วงหน้า: สอบกลางภาค".to_string(),
            "กำหนดการนี้จะเริ่มในอีก 3 วัน".to_string(),
        )
    );
}

#[test]
fn calendar_notification_link_matches_supported_user_types() {
    assert_eq!(
        calendar_notification_link_for_user_type("staff"),
        Some("/staff/calendar")
    );
    assert_eq!(
        calendar_notification_link_for_user_type("student"),
        Some("/student/calendar")
    );
    assert_eq!(
        calendar_notification_link_for_user_type("parent"),
        Some("/parent")
    );
    assert_eq!(calendar_notification_link_for_user_type("guest"), None);
}

#[test]
fn reminder_advisory_lock_keys_are_stable_from_uuid_bytes() {
    let reminder_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();

    assert_eq!(
        calendar_reminder_advisory_lock_keys(reminder_id),
        (16_909_060, 84_281_096)
    );
}

#[test]
fn due_reminder_candidate_query_does_not_lock_or_mark_sent() {
    let sql = select_due_calendar_reminder_candidates_sql();

    assert!(sql.contains("LIMIT 200"));
    assert!(sql.contains("sent_at IS NULL"));
    assert!(sql.contains("$2::uuid[]"));
    assert!(!sql.contains("FOR UPDATE"));
    assert!(!sql.contains("SET sent_at"));
}

#[test]
fn due_reminder_candidate_query_excludes_attempted_ids_for_batching() {
    let sql = select_due_calendar_reminder_candidates_sql();

    assert!(sql.contains("NOT (id = ANY($2::uuid[]))"));
    assert!(sql.contains("LIMIT 200"));
}

#[test]
fn due_reminder_mark_query_sets_sent_after_attempt() {
    let sql = mark_calendar_reminder_sent_sql();

    assert!(sql.contains("UPDATE calendar_event_reminders"));
    assert!(sql.contains("SET sent_at = NOW()"));
    assert!(sql.contains("WHERE id = $1 AND sent_at IS NULL"));
}

#[test]
fn notification_outcome_marks_reminders_sent_only_when_none_or_some_success() {
    assert!(CalendarNotificationSendOutcome {
        recipient_count: 0,
        successful_count: 0,
        failed_count: 0,
    }
    .should_mark_reminder_sent());
    assert!(CalendarNotificationSendOutcome {
        recipient_count: 2,
        successful_count: 1,
        failed_count: 1,
    }
    .should_mark_reminder_sent());
    assert!(!CalendarNotificationSendOutcome {
        recipient_count: 2,
        successful_count: 0,
        failed_count: 2,
    }
    .should_mark_reminder_sent());
}

#[test]
fn advisory_lock_queries_use_two_integer_keys() {
    assert!(try_calendar_reminder_advisory_lock_sql().contains("pg_try_advisory_lock($1, $2)"));
    assert!(release_calendar_reminder_advisory_lock_sql().contains("pg_advisory_unlock($1, $2)"));
}

fn calendar_event_for_notification(title: &str) -> CalendarEvent {
    let now = Utc::now();
    let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

    CalendarEvent {
        id: Uuid::new_v4(),
        category_id: None,
        category_name: None,
        category_color: None,
        title: title.to_string(),
        description: None,
        location: None,
        start_date: date,
        end_date: date,
        all_day: true,
        start_time: None,
        end_time: None,
        is_public: false,
        tags: Vec::new(),
        targets: Vec::new(),
        reminders: Vec::new(),
        created_by: None,
        updated_by: None,
        created_at: now,
        updated_at: now,
    }
}
