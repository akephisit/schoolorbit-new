use super::*;

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
        academic_year_id: Uuid::new_v4(),
        academic_term_id: None,
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
