use std::sync::atomic::{AtomicI32, Ordering};

use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarAudienceType, CalendarEventQuery, CalendarEventTargetInput,
    UpsertCalendarCategoryRequest, UpsertCalendarEventRequest, UpsertCalendarTagRequest,
};
use crate::modules::notification::events::TenantNotificationEvent;
use crate::test_helpers::{create_test_pool, run_test_migrations};

use super::services;

static NEXT_YEAR: AtomicI32 = AtomicI32::new(50_000);

struct CalendarFixture {
    staff_user_id: Uuid,
    student_user_id: Uuid,
    second_student_user_id: Uuid,
    parent_user_id: Uuid,
    grade_level_id: Uuid,
    classroom_id: Uuid,
}

async fn migrated_pool() -> PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

async fn insert_user(pool: &PgPool, user_type: &str, first_name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, password_hash, first_name, last_name, user_type, status)
         VALUES ($1, 'test-only', $2, 'Calendar Fixture', $3, 'active')",
    )
    .bind(id)
    .bind(first_name)
    .bind(user_type)
    .execute(pool)
    .await
    .expect("calendar fixture user should insert");
    id
}

async fn insert_fixture(pool: &PgPool) -> CalendarFixture {
    let year = NEXT_YEAR.fetch_add(1, Ordering::Relaxed);
    let academic_year_id = Uuid::new_v4();
    let study_plan_id = Uuid::new_v4();
    let study_plan_version_id = Uuid::new_v4();
    let classroom_id = Uuid::new_v4();
    let staff_user_id = insert_user(pool, "staff", "Calendar Staff").await;
    let student_user_id = insert_user(pool, "student", "Calendar Student").await;
    let second_student_user_id = insert_user(pool, "student", "Second Calendar Student").await;
    let parent_user_id = insert_user(pool, "parent", "Calendar Parent").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY created_at, id LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("baseline grade level should exist");

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date)
         VALUES ($1, $2, $3, '9600-01-01', '9600-12-31')",
    )
    .bind(academic_year_id)
    .bind(year)
    .bind(format!("Calendar {year}"))
    .execute(pool)
    .await
    .expect("academic year should insert");

    sqlx::query(
        "INSERT INTO study_plans (id, code, name_th)
         VALUES ($1, $2, 'Calendar Characterization Plan')",
    )
    .bind(study_plan_id)
    .bind(format!("CAL-{study_plan_id}"))
    .execute(pool)
    .await
    .expect("study plan should insert");

    sqlx::query(
        "INSERT INTO study_plan_versions
            (id, study_plan_id, version_name, start_academic_year_id)
         VALUES ($1, $2, 'Calendar Version', $3)",
    )
    .bind(study_plan_version_id)
    .bind(study_plan_id)
    .bind(academic_year_id)
    .execute(pool)
    .await
    .expect("study-plan version should insert");

    sqlx::query(
        "INSERT INTO class_rooms
            (id, code, name, academic_year_id, grade_level_id, study_plan_version_id)
         VALUES ($1, $2, 'Calendar Classroom', $3, $4, $5)",
    )
    .bind(classroom_id)
    .bind(format!("CAL-{}", &classroom_id.to_string()[..8]))
    .bind(academic_year_id)
    .bind(grade_level_id)
    .bind(study_plan_version_id)
    .execute(pool)
    .await
    .expect("classroom should insert");

    for (student_id, class_number) in [(student_user_id, 1), (second_student_user_id, 2)] {
        sqlx::query(
            "INSERT INTO student_class_enrollments
                (student_id, class_room_id, status, class_number)
             VALUES ($1, $2, 'active', $3)",
        )
        .bind(student_id)
        .bind(classroom_id)
        .bind(class_number)
        .execute(pool)
        .await
        .expect("student enrollment should insert");
    }

    sqlx::query(
        "INSERT INTO student_parents (student_user_id, parent_user_id, relationship)
         VALUES ($1, $2, 'guardian')",
    )
    .bind(student_user_id)
    .bind(parent_user_id)
    .execute(pool)
    .await
    .expect("parent-child link should insert");

    CalendarFixture {
        staff_user_id,
        student_user_id,
        second_student_user_id,
        parent_user_id,
        grade_level_id,
        classroom_id,
    }
}

fn calendar_today() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}

fn query_around(today: NaiveDate) -> CalendarEventQuery {
    CalendarEventQuery {
        from: Some(today - Duration::days(30)),
        to: Some(today + Duration::days(60)),
        category_id: None,
        tag_id: None,
        audience: None,
        visibility: None,
        q: None,
    }
}

fn target(
    audience_type: CalendarAudienceType,
    grade_level_id: Option<Uuid>,
    class_room_id: Option<Uuid>,
) -> CalendarEventTargetInput {
    CalendarEventTargetInput {
        audience_type,
        grade_level_id,
        class_room_id,
    }
}

fn event_request(
    title: &str,
    start_date: NaiveDate,
    is_public: bool,
    tag_ids: Vec<Uuid>,
    targets: Vec<CalendarEventTargetInput>,
    reminder_offsets_days: Vec<i32>,
) -> UpsertCalendarEventRequest {
    UpsertCalendarEventRequest {
        title: title.to_string(),
        description: Some(format!("{title} description")),
        location: Some("Calendar fixture hall".to_string()),
        category_id: None,
        start_date,
        end_date: start_date,
        all_day: true,
        start_time: None,
        end_time: None,
        is_public,
        tag_ids,
        targets,
        reminder_offsets_days,
        notify_audience: false,
    }
}

async fn create_named_event(
    pool: &PgPool,
    fixture: &CalendarFixture,
    title: &str,
    is_public: bool,
    targets: Vec<CalendarEventTargetInput>,
) -> Uuid {
    services::create_event(
        pool,
        fixture.staff_user_id,
        event_request(
            title,
            calendar_today() + Duration::days(5),
            is_public,
            Vec::new(),
            targets,
            Vec::new(),
        ),
    )
    .await
    .expect("calendar fixture event should create")
    .event
    .id
}

#[tokio::test]
async fn event_lifecycle_preserves_targets_tags_reminders_and_soft_delete() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let today = calendar_today();
    let category = services::create_category(
        &pool,
        UpsertCalendarCategoryRequest {
            name: format!("Lifecycle {}", Uuid::new_v4()),
            color: "#2563eb".to_string(),
            order_index: Some(10),
            is_active: Some(true),
        },
    )
    .await
    .expect("category should create");
    let tag = services::create_tag(
        &pool,
        UpsertCalendarTagRequest {
            name: format!("Lifecycle {}", Uuid::new_v4()),
        },
    )
    .await
    .expect("tag should create");

    let mut payload = event_request(
        "Lifecycle event",
        today + Duration::days(8),
        false,
        vec![tag.id, tag.id],
        vec![
            target(
                CalendarAudienceType::Student,
                None,
                Some(fixture.classroom_id),
            ),
            target(
                CalendarAudienceType::Parent,
                Some(fixture.grade_level_id),
                None,
            ),
        ],
        vec![1, 7, 1],
    );
    payload.category_id = Some(category.id);
    let created = services::create_event(&pool, fixture.staff_user_id, payload)
        .await
        .expect("event should create");
    assert!(!created.notify_audience);
    assert_eq!(created.event.tags.len(), 1);
    assert_eq!(created.event.targets.len(), 2);
    assert_eq!(created.event.reminders.len(), 2);
    assert_eq!(
        created
            .event
            .reminders
            .iter()
            .map(|reminder| reminder.days_before)
            .collect::<Vec<_>>(),
        vec![7, 1]
    );

    let updated = services::update_event(
        &pool,
        fixture.staff_user_id,
        created.event.id,
        event_request(
            "Lifecycle event updated",
            today + Duration::days(10),
            true,
            Vec::new(),
            vec![target(CalendarAudienceType::All, None, None)],
            vec![3],
        ),
    )
    .await
    .expect("event should update");
    assert_eq!(updated.event.title, "Lifecycle event updated");
    assert!(updated.event.is_public);
    assert!(updated.event.tags.is_empty());
    assert_eq!(updated.event.targets.len(), 1);
    assert_eq!(updated.event.targets[0].audience_type, "all");
    assert_eq!(updated.event.reminders.len(), 1);
    assert_eq!(updated.event.reminders[0].days_before, 3);

    let listed = services::list_management_events(&pool, query_around(today))
        .await
        .expect("management events should list");
    assert!(listed.iter().any(|event| event.id == created.event.id));

    services::soft_delete_event(&pool, created.event.id, fixture.staff_user_id)
        .await
        .expect("event should soft-delete");
    let listed_after_delete = services::list_management_events(&pool, query_around(today))
        .await
        .expect("management events should list after deletion");
    assert!(!listed_after_delete
        .iter()
        .any(|event| event.id == created.event.id));
    let pending_reminders: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM calendar_event_reminders
         WHERE event_id = $1 AND sent_at IS NULL",
    )
    .bind(created.event.id)
    .fetch_one(&pool)
    .await
    .expect("pending reminder count should query");
    assert_eq!(pending_reminders, 0);
}

#[tokio::test]
async fn duplicate_category_and_tag_names_keep_existing_conflict_messages() {
    let pool = migrated_pool().await;
    insert_fixture(&pool).await;
    let suffix = Uuid::new_v4();
    let category_name = format!("Duplicate Category {suffix}");
    services::create_category(
        &pool,
        UpsertCalendarCategoryRequest {
            name: category_name.clone(),
            color: "#111827".to_string(),
            order_index: None,
            is_active: Some(true),
        },
    )
    .await
    .expect("first category should create");
    let duplicate_category = services::create_category(
        &pool,
        UpsertCalendarCategoryRequest {
            name: category_name.to_lowercase(),
            color: "#111827".to_string(),
            order_index: None,
            is_active: Some(true),
        },
    )
    .await
    .expect_err("case-insensitive duplicate category should fail");
    assert!(matches!(
        duplicate_category,
        AppError::BadRequest(message) if message == "มีหมวดหมู่นี้อยู่แล้ว"
    ));

    let tag_name = format!("Duplicate Tag {suffix}");
    services::create_tag(
        &pool,
        UpsertCalendarTagRequest {
            name: tag_name.clone(),
        },
    )
    .await
    .expect("first tag should create");
    let duplicate_tag = services::create_tag(
        &pool,
        UpsertCalendarTagRequest {
            name: format!("  {}  ", tag_name.to_lowercase()),
        },
    )
    .await
    .expect_err("normalized duplicate tag should fail");
    assert!(matches!(
        duplicate_tag,
        AppError::BadRequest(message) if message == "มีแท็กนี้อยู่แล้ว"
    ));
}

#[tokio::test]
async fn management_self_child_and_public_views_enforce_current_audiences() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let today = calendar_today();
    let all_id = create_named_event(
        &pool,
        &fixture,
        "All audience",
        false,
        vec![target(CalendarAudienceType::All, None, None)],
    )
    .await;
    let staff_id = create_named_event(
        &pool,
        &fixture,
        "Staff audience",
        false,
        vec![target(CalendarAudienceType::Staff, None, None)],
    )
    .await;
    let student_id = create_named_event(
        &pool,
        &fixture,
        "Student audience",
        false,
        vec![target(
            CalendarAudienceType::Student,
            None,
            Some(fixture.classroom_id),
        )],
    )
    .await;
    let parent_public_id = create_named_event(
        &pool,
        &fixture,
        "Parent public audience",
        true,
        vec![target(
            CalendarAudienceType::Parent,
            None,
            Some(fixture.classroom_id),
        )],
    )
    .await;

    let management = services::list_management_events(&pool, query_around(today))
        .await
        .expect("management events should list");
    let staff = services::list_my_events(&pool, fixture.staff_user_id, query_around(today))
        .await
        .expect("staff events should list");
    let student = services::list_my_events(&pool, fixture.student_user_id, query_around(today))
        .await
        .expect("student events should list");
    let child = services::list_child_events(
        &pool,
        fixture.parent_user_id,
        fixture.student_user_id,
        query_around(today),
    )
    .await
    .expect("child events should list");
    let public = services::list_public_events(&pool, query_around(today))
        .await
        .expect("public events should list");

    let ids = |events: &[crate::modules::calendar::models::CalendarEvent]| {
        events.iter().map(|event| event.id).collect::<Vec<_>>()
    };
    let management_ids = ids(&management);
    assert!(management_ids.contains(&all_id));
    assert!(management_ids.contains(&staff_id));
    assert!(management_ids.contains(&student_id));
    assert!(management_ids.contains(&parent_public_id));

    let staff_ids = staff.iter().map(|event| event.id).collect::<Vec<_>>();
    assert!(staff_ids.contains(&all_id));
    assert!(staff_ids.contains(&staff_id));
    assert!(!staff_ids.contains(&student_id));
    assert!(!staff_ids.contains(&parent_public_id));

    let student_ids = student.iter().map(|event| event.id).collect::<Vec<_>>();
    assert!(student_ids.contains(&all_id));
    assert!(student_ids.contains(&student_id));
    assert!(!student_ids.contains(&staff_id));
    assert!(!student_ids.contains(&parent_public_id));

    let child_ids = child.iter().map(|event| event.id).collect::<Vec<_>>();
    assert!(child_ids.contains(&all_id));
    assert!(child_ids.contains(&parent_public_id));
    assert!(!child_ids.contains(&staff_id));
    assert!(!child_ids.contains(&student_id));

    let public_ids = public.iter().map(|event| event.id).collect::<Vec<_>>();
    assert_eq!(
        public_ids
            .iter()
            .filter(|id| **id == parent_public_id)
            .count(),
        1
    );
    assert!(!public_ids.contains(&all_id));
    assert!(!public_ids.contains(&staff_id));
    assert!(!public_ids.contains(&student_id));
}

#[tokio::test]
async fn recipient_resolution_deduplicates_overlapping_all_grade_class_targets() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let event_id = create_named_event(
        &pool,
        &fixture,
        "Recipient overlap",
        false,
        vec![
            target(CalendarAudienceType::All, None, None),
            target(CalendarAudienceType::Staff, None, None),
            target(CalendarAudienceType::Student, None, None),
            target(
                CalendarAudienceType::Student,
                Some(fixture.grade_level_id),
                None,
            ),
            target(
                CalendarAudienceType::Student,
                None,
                Some(fixture.classroom_id),
            ),
            target(CalendarAudienceType::Parent, None, None),
            target(
                CalendarAudienceType::Parent,
                Some(fixture.grade_level_id),
                None,
            ),
            target(
                CalendarAudienceType::Parent,
                None,
                Some(fixture.classroom_id),
            ),
        ],
    )
    .await;

    let recipients = services::resolve_event_recipient_user_ids(&pool, event_id)
        .await
        .expect("recipient resolution should complete");
    for expected in [
        fixture.staff_user_id,
        fixture.student_user_id,
        fixture.second_student_user_id,
        fixture.parent_user_id,
    ] {
        assert_eq!(
            recipients.iter().filter(|id| **id == expected).count(),
            1,
            "each overlapping recipient should appear once"
        );
    }
}

#[tokio::test]
async fn successful_reminder_marks_sent_once_and_second_run_is_idempotent() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let today = calendar_today();
    let created = services::create_event(
        &pool,
        fixture.staff_user_id,
        event_request(
            "Reminder delivery",
            today + Duration::days(1),
            false,
            Vec::new(),
            vec![target(
                CalendarAudienceType::Parent,
                None,
                Some(fixture.classroom_id),
            )],
            vec![1],
        ),
    )
    .await
    .expect("reminder event should create");
    let (notification_tx, mut notification_rx) = broadcast::channel::<TenantNotificationEvent>(8);

    let first = services::process_due_reminders(&pool, &notification_tx, "calendar-test", today)
        .await
        .expect("first reminder run should complete");
    assert_eq!(first, 1);
    let notification =
        tokio::time::timeout(std::time::Duration::from_secs(2), notification_rx.recv())
            .await
            .expect("notification should arrive before timeout")
            .expect("notification channel should remain open");
    assert_eq!(notification.tenant, "calendar-test");
    assert_eq!(notification.user_id, fixture.parent_user_id);
    assert!(notification
        .notification
        .title
        .contains("Reminder delivery"));

    let sent_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT sent_at FROM calendar_event_reminders WHERE event_id = $1")
            .bind(created.event.id)
            .fetch_one(&pool)
            .await
            .expect("sent_at should query");
    assert!(sent_at.is_some());

    let second = services::process_due_reminders(&pool, &notification_tx, "calendar-test", today)
        .await
        .expect("second reminder run should complete");
    assert_eq!(second, 0);
}

#[tokio::test]
async fn reminder_without_active_recipients_is_marked_complete_without_broadcast() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let today = calendar_today();
    let created = services::create_event(
        &pool,
        fixture.staff_user_id,
        event_request(
            "No active recipients",
            today + Duration::days(1),
            false,
            Vec::new(),
            vec![target(
                CalendarAudienceType::Parent,
                None,
                Some(fixture.classroom_id),
            )],
            vec![1],
        ),
    )
    .await
    .expect("reminder event should create");
    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(fixture.parent_user_id)
        .execute(&pool)
        .await
        .expect("fixture recipient should deactivate");
    let (notification_tx, mut notification_rx) = broadcast::channel::<TenantNotificationEvent>(8);

    let processed =
        services::process_due_reminders(&pool, &notification_tx, "calendar-test", today)
            .await
            .expect("recipient-free reminder run should complete");
    assert_eq!(processed, 1);
    assert!(notification_rx.try_recv().is_err());

    let sent_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT sent_at FROM calendar_event_reminders WHERE event_id = $1")
            .bind(created.event.id)
            .fetch_one(&pool)
            .await
            .expect("sent_at should query");
    assert!(sent_at.is_some());
}
