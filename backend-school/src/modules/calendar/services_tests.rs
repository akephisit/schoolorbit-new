use std::sync::atomic::{AtomicI32, Ordering};

use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::modules::notification::events::TenantNotificationEvent;
use school_calendar::models::{
    CalendarAudienceType, CalendarEventQuery, CalendarEventTargetInput,
    UpsertCalendarCategoryRequest, UpsertCalendarEventRequest, UpsertCalendarTagRequest,
};
use school_errors::AppError;
use school_test_db::create_named_test_pool_with_max_connections;

use super::services;

static NEXT_YEAR: AtomicI32 = AtomicI32::new(50_000);

struct CalendarFixture {
    staff_user_id: Uuid,
    student_user_id: Uuid,
    second_student_user_id: Uuid,
    parent_user_id: Uuid,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    homeroom_id: Uuid,
}

async fn migrated_pool(name: &str) -> PgPool {
    migrated_pool_through(name, 98).await
}

async fn migrated_pool_through(name: &str, version: i64) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, 5).await;
    apply_migrations_through(&pool, 40)
        .await
        .expect("legacy calendar fixture migrations should run");
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .expect("academic cutover fixture should seed");
    apply_phase_b_runtime_migrations(&pool)
        .await
        .expect("canonical calendar fixture migrations should run");
    apply_migrations_through(&pool, version)
        .await
        .expect("current calendar fixture migrations should run");
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
    let homeroom_id = Uuid::new_v4();
    let staff_user_id = insert_user(pool, "staff", "Calendar Staff").await;
    let student_user_id = insert_user(pool, "student", "Calendar Student").await;
    let second_student_user_id = insert_user(pool, "student", "Second Calendar Student").await;
    let parent_user_id = insert_user(pool, "parent", "Calendar Parent").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY created_at, id LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("baseline grade level should exist");
    let study_program_id: Uuid =
        sqlx::query_scalar("SELECT id FROM study_programs ORDER BY created_at, id LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("cutover fixture study program should exist");
    let today = calendar_today();

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date, status)
         VALUES ($1, $2, $3, $4, $5, 'planning')",
    )
    .bind(academic_year_id)
    .bind(year)
    .bind(format!("Calendar {year}"))
    .bind(today - Duration::days(60))
    .bind(today + Duration::days(365))
    .execute(pool)
    .await
    .expect("academic year should insert");

    sqlx::query(
        "INSERT INTO homerooms (
            id, code, name, academic_year_id, grade_level_id,
            study_program_id
         ) VALUES ($1, $2, 'Calendar Homeroom', $3, $4, $5)",
    )
    .bind(homeroom_id)
    .bind(format!("CAL-{}", &homeroom_id.to_string()[..8]))
    .bind(academic_year_id)
    .bind(grade_level_id)
    .bind(study_program_id)
    .execute(pool)
    .await
    .expect("homeroom should insert");

    for (student_id, class_number) in [(student_user_id, 1), (second_student_user_id, 2)] {
        let student_academic_year_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO student_academic_years (
                id, student_id, academic_year_id, grade_level_id, study_program_id, status
             ) VALUES ($1, $2, $3, $4, $5, 'active')",
        )
        .bind(student_academic_year_id)
        .bind(student_id)
        .bind(academic_year_id)
        .bind(grade_level_id)
        .bind(study_program_id)
        .execute(pool)
        .await
        .expect("student academic-year record should insert");

        sqlx::query(
            "INSERT INTO homeroom_placements (
                id, student_academic_year_id, academic_year_id, homeroom_id,
                start_date, status, enrollment_type, class_number
             ) VALUES ($1, $2, $3, $4, $5, 'current', 'regular', $6)",
        )
        .bind(Uuid::new_v4())
        .bind(student_academic_year_id)
        .bind(academic_year_id)
        .bind(homeroom_id)
        .bind(today - Duration::days(30))
        .bind(class_number)
        .execute(pool)
        .await
        .expect("homeroom placement should insert");
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
        academic_year_id,
        grade_level_id,
        homeroom_id,
    }
}

fn calendar_today() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}

fn query_around(today: NaiveDate, _academic_year_id: Uuid) -> CalendarEventQuery {
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
    homeroom_id: Option<Uuid>,
) -> CalendarEventTargetInput {
    CalendarEventTargetInput {
        audience_type,
        grade_level_id,
        homeroom_id,
    }
}

fn event_request(
    _academic_year_id: Uuid,
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
            fixture.academic_year_id,
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
    let pool = migrated_pool("calendar_event_lifecycle").await;
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
        fixture.academic_year_id,
        "Lifecycle event",
        today + Duration::days(8),
        false,
        vec![tag.id, tag.id],
        vec![
            target(
                CalendarAudienceType::Student,
                None,
                Some(fixture.homeroom_id),
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
            fixture.academic_year_id,
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

    let listed =
        services::list_management_events(&pool, query_around(today, fixture.academic_year_id))
            .await
            .expect("management events should list");
    assert!(listed.iter().any(|event| event.id == created.event.id));

    services::soft_delete_event(&pool, created.event.id, fixture.staff_user_id)
        .await
        .expect("event should soft-delete");
    let listed_after_delete =
        services::list_management_events(&pool, query_around(today, fixture.academic_year_id))
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
async fn calendar_date_migration_preserves_existing_event_and_related_history() {
    let pool = migrated_pool_through("calendar_migration_preservation", 97).await;
    let fixture = insert_fixture(&pool).await;
    let event_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let reminder_id = Uuid::new_v4();
    let today = calendar_today();
    sqlx::query("INSERT INTO calendar_events(id,academic_year_id,title,description,start_date,end_date,created_by,updated_by) VALUES($1,$2,'Preserved event','Existing detail',$3,$3,$4,$4)")
        .bind(event_id).bind(fixture.academic_year_id).bind(today).bind(fixture.staff_user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO calendar_event_targets(id,event_id,academic_year_id,audience_type) VALUES($1,$2,$3,'all')")
        .bind(target_id).bind(event_id).bind(fixture.academic_year_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO calendar_event_reminders(id,event_id,days_before,remind_on,sent_at) VALUES($1,$2,1,$3,now())")
        .bind(reminder_id).bind(event_id).bind(today-Duration::days(1)).execute(&pool).await.unwrap();
    let before:serde_json::Value=sqlx::query_scalar("SELECT to_jsonb(e)-ARRAY['academic_year_id','academic_term_id','migration_provenance'] FROM calendar_events e WHERE id=$1")
        .bind(event_id).fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 98).await.unwrap();
    let after: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(e)-'migration_provenance' FROM calendar_events e WHERE id=$1",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    let original_year:String=sqlx::query_scalar("SELECT migration_provenance->'dateCalendarContext'->>'academicYearId' FROM calendar_events WHERE id=$1")
        .bind(event_id).fetch_one(&pool).await.unwrap();
    assert_eq!(original_year, fixture.academic_year_id.to_string());
    let event = school_calendar::services::get_event_for_response(&pool, event_id)
        .await
        .unwrap();
    assert_eq!(event.targets[0].id, target_id);
    assert_eq!(event.reminders[0].id, reminder_id);
    assert!(event.reminders[0].sent_at.is_some());
}

fn payload(
    date: NaiveDate,
    year: Uuid,
    title: &str,
    targets: Vec<CalendarEventTargetInput>,
) -> UpsertCalendarEventRequest {
    event_request(year, title, date, false, Vec::new(), targets, Vec::new())
}

#[tokio::test]
async fn date_calendar_accepts_dates_outside_and_closed_academic_years() {
    let pool = migrated_pool("date_calendar").await;
    let fixture = insert_fixture(&pool).await;
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(fixture.academic_year_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut request = payload(
        calendar_today(),
        fixture.academic_year_id,
        "Future event",
        vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: None,
            homeroom_id: None,
        }],
    );
    let closed_year_event = services::create_event(&pool, fixture.staff_user_id, request.clone())
        .await
        .unwrap()
        .event;
    services::soft_delete_event(&pool, closed_year_event.id, fixture.staff_user_id)
        .await
        .unwrap();
    request.start_date = NaiveDate::from_ymd_opt(2035, 6, 12).unwrap();
    request.end_date = request.start_date;
    let event = services::create_event(&pool, fixture.staff_user_id, request.clone())
        .await
        .unwrap()
        .event;
    let mut query = query_around(calendar_today(), fixture.academic_year_id);
    query.from = Some(request.start_date);
    query.to = Some(request.end_date);
    assert!(services::list_management_events(&pool, query)
        .await
        .unwrap()
        .iter()
        .any(|item| item.id == event.id));
    request.title = "Updated future event".into();
    assert_eq!(
        services::update_event(&pool, fixture.staff_user_id, event.id, request)
            .await
            .unwrap()
            .event
            .title,
        "Updated future event"
    );
    services::soft_delete_event(&pool, event.id, fixture.staff_user_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn review_queue_is_fifo_across_pages_while_own_history_is_newest_first() {
    use school_authorization::ActorContext;
    use school_calendar::requests::{self, CalendarRequestQuery, CalendarRequestStatus};
    use school_permissions::registry::codes;
    let pool = migrated_pool("calendar_request_fifo").await;
    let fixture = insert_fixture(&pool).await;
    sqlx::query("INSERT INTO calendar_event_requests(requested_by,title,description,start_date,end_date,all_day,created_at)
        SELECT $1,'Queue '||lpad(sequence::text,2,'0'),'Synthetic detail',$2,$2,true,
            '2026-01-01 00:00:00+00'::timestamptz + sequence * interval '1 minute'
        FROM generate_series(0,28) AS sequence")
        .bind(fixture.staff_user_id).bind(calendar_today()).execute(&pool).await.unwrap();
    let manager = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![
            codes::CALENDAR_READ_SCHOOL.into(),
            codes::CALENDAR_MANAGE_SCHOOL.into(),
        ],
    };
    let query = CalendarRequestQuery {
        review: true,
        status: Some(CalendarRequestStatus::Pending),
        offset: None,
    };
    let first = requests::list_requests(&pool, &manager, query.clone())
        .await
        .unwrap();
    assert!(first.has_more);
    assert_eq!(first.records.len(), 25);
    assert_eq!(first.records[0].title, "Queue 00");
    assert_eq!(first.records[24].title, "Queue 24");
    let second = requests::list_requests(
        &pool,
        &manager,
        CalendarRequestQuery {
            offset: Some(25),
            ..query
        },
    )
    .await
    .unwrap();
    assert!(!second.has_more);
    assert_eq!(
        second
            .records
            .iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Queue 25", "Queue 26", "Queue 27", "Queue 28"]
    );
    let own = requests::list_requests(&pool, &manager, CalendarRequestQuery::default())
        .await
        .unwrap();
    assert_eq!(own.records[0].title, "Queue 28");

    requests::approve_request(
        &pool,
        &manager,
        first.records[0].id,
        payload_event_for_pending_test(calendar_today(), fixture.academic_year_id),
    )
    .await
    .unwrap();
    let remaining = requests::list_requests(
        &pool,
        &manager,
        CalendarRequestQuery {
            review: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(remaining.records.len(), 25);
    assert!(remaining.has_more);
    assert_eq!(remaining.records[0].title, "Queue 01");
    assert_eq!(remaining.records[24].title, "Queue 25");
    let final_page = requests::list_requests(
        &pool,
        &manager,
        CalendarRequestQuery {
            review: true,
            offset: Some(25),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        final_page
            .records
            .iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Queue 26", "Queue 27", "Queue 28"]
    );
    assert!(!final_page.has_more);
    let approved_history = requests::list_requests(
        &pool,
        &manager,
        CalendarRequestQuery {
            status: Some(CalendarRequestStatus::Approved),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(approved_history.records.len(), 1);
    assert_eq!(approved_history.records[0].title, "Queue 00");

    sqlx::query("INSERT INTO calendar_event_requests(id,requested_by,title,description,start_date,end_date,all_day,created_at)
        SELECT ('00000000-0000-0000-0000-'||lpad(sequence::text,12,'0'))::uuid,$1,'Tied '||sequence,'Synthetic detail',$2,$2,true,'2025-01-01 00:00:00+00'
        FROM generate_series(1,2) AS sequence")
        .bind(fixture.staff_user_id).bind(calendar_today()).execute(&pool).await.unwrap();
    let tied = requests::list_requests(
        &pool,
        &manager,
        CalendarRequestQuery {
            review: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(tied.records[0].title, "Tied 1");
    assert_eq!(tied.records[1].title, "Tied 2");
}

#[tokio::test]
async fn pending_calendar_is_scoped_date_overlapping_and_excludes_decided_requests() {
    use school_authorization::ActorContext;
    use school_calendar::requests::{self, CreateCalendarRequest, PendingCalendarQuery};
    use school_permissions::registry::codes;
    let pool = migrated_pool("calendar_pending_overlay").await;
    let fixture = insert_fixture(&pool).await;
    let own = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![
            codes::CALENDAR_READ_SCHOOL.into(),
            codes::CALENDAR_REQUEST_OWN.into(),
        ],
    };
    let manager = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![
            codes::CALENDAR_READ_SCHOOL.into(),
            codes::CALENDAR_MANAGE_SCHOOL.into(),
        ],
    };
    let other = ActorContext {
        user_id: insert_user(&pool, "staff", "Other requester").await,
        permissions: own.permissions.clone(),
    };
    let reader = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![codes::CALENDAR_READ_SCHOOL.into()],
    };
    let from = NaiveDate::from_ymd_opt(2027, 6, 1).unwrap();
    let to = from + Duration::days(41);
    let payload = |title: &str, start_date, end_date| CreateCalendarRequest {
        title: title.into(),
        description: "Synthetic request".into(),
        location: None,
        start_date,
        end_date,
        all_day: true,
        start_time: None,
        end_time: None,
    };
    let spanning = requests::create_request(
        &pool,
        &own,
        payload("Spanning", from - Duration::days(2), from),
    )
    .await
    .unwrap();
    let mut timed = payload("Boundary", to, to);
    timed.all_day = false;
    timed.start_time = Some(chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap());
    timed.end_time = Some(chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap());
    let boundary = requests::create_request(&pool, &own, timed).await.unwrap();
    let other_request = requests::create_request(&pool, &other, payload("Other", from, from))
        .await
        .unwrap();
    requests::create_request(
        &pool,
        &own,
        payload("Outside", to + Duration::days(1), to + Duration::days(1)),
    )
    .await
    .unwrap();
    let rejected = requests::create_request(&pool, &own, payload("Rejected", from, from))
        .await
        .unwrap();
    requests::reject_request(&pool, &manager, rejected.id, "Duplicate date")
        .await
        .unwrap();
    let approved = requests::create_request(&pool, &own, payload("Approved", from, from))
        .await
        .unwrap();
    requests::approve_request(
        &pool,
        &manager,
        approved.id,
        payload_event_for_pending_test(from, fixture.academic_year_id),
    )
    .await
    .unwrap();
    let query = PendingCalendarQuery { from, to };
    let detail = requests::get_request_for_review(&pool, &manager, other_request.id)
        .await
        .unwrap();
    assert_eq!(detail.id, other_request.id);
    assert_eq!(detail.description, other_request.description);
    for denied in [&own, &other, &reader] {
        assert!(matches!(
            requests::get_request_for_review(&pool, denied, other_request.id).await,
            Err(AppError::Forbidden(_))
        ));
    }
    assert!(matches!(
        requests::get_request_for_review(&pool, &manager, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
    let queue = requests::list_requests(
        &pool,
        &manager,
        requests::CalendarRequestQuery {
            review: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(queue
        .records
        .iter()
        .all(|row| row.status != requests::CalendarRequestStatus::Approved));
    assert!(queue.records.iter().any(|row| row.id == rejected.id));
    assert!(!queue.records.iter().any(|row| row.id == approved.id));
    let invalid_review_filter = requests::list_requests(
        &pool,
        &manager,
        requests::CalendarRequestQuery {
            review: true,
            status: Some(requests::CalendarRequestStatus::Approved),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(invalid_review_filter.records.is_empty());
    let mine = requests::list_pending_calendar(&pool, &own, query.clone())
        .await
        .unwrap();
    assert!(!mine.has_more);
    assert_eq!(
        mine.records
            .iter()
            .map(|row| row.id)
            .collect::<std::collections::HashSet<_>>(),
        std::collections::HashSet::from([spanning.id, boundary.id])
    );
    assert_eq!(
        mine.records
            .iter()
            .find(|row| row.id == boundary.id)
            .unwrap()
            .start_time,
        Some(chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap())
    );
    let other_view = requests::list_pending_calendar(&pool, &other, query.clone())
        .await
        .unwrap();
    assert_eq!(other_view.records.len(), 1);
    assert_eq!(other_view.records[0].id, other_request.id);
    let all = requests::list_pending_calendar(&pool, &manager, query.clone())
        .await
        .unwrap();
    assert_eq!(all.records.len(), 3);
    assert!(matches!(
        requests::list_pending_calendar(&pool, &reader, query.clone()).await,
        Err(AppError::Forbidden(_))
    ));
    sqlx::query("INSERT INTO calendar_event_requests(requested_by,title,description,start_date,end_date,all_day)
        SELECT $1,'Bulk '||sequence,'Synthetic detail',$2,$2,true FROM generate_series(1,501) AS sequence")
        .bind(fixture.staff_user_id).bind(from).execute(&pool).await.unwrap();
    let bounded = requests::list_pending_calendar(&pool, &manager, query)
        .await
        .unwrap();
    assert!(bounded.has_more);
    assert_eq!(bounded.records.len(), 500);
}

fn payload_event_for_pending_test(date: NaiveDate, year: Uuid) -> UpsertCalendarEventRequest {
    payload(
        date,
        year,
        "Approved event",
        vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: None,
            homeroom_id: None,
        }],
    )
}

#[tokio::test]
async fn pending_requests_are_private_and_decisions_create_one_event() {
    use school_authorization::ActorContext;
    use school_calendar::requests::{
        self, CalendarRequestQuery, CalendarRequestStatus, CreateCalendarRequest,
    };
    use school_permissions::registry::codes;
    let pool = migrated_pool("calendar_requests").await;
    let fixture = insert_fixture(&pool).await;
    let own = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![
            codes::CALENDAR_READ_SCHOOL.into(),
            codes::CALENDAR_REQUEST_OWN.into(),
        ],
    };
    let manager = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![
            codes::CALENDAR_READ_SCHOOL.into(),
            codes::CALENDAR_MANAGE_SCHOOL.into(),
        ],
    };
    let other = ActorContext {
        user_id: insert_user(&pool, "staff", "Other staff").await,
        permissions: own.permissions.clone(),
    };
    let date = calendar_today();
    let original_event_count: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let request = requests::create_request(
        &pool,
        &own,
        CreateCalendarRequest {
            title: "Requested event".into(),
            description: "Request details".into(),
            location: None,
            start_date: date,
            end_date: date,
            all_day: true,
            start_time: None,
            end_time: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(request.status, CalendarRequestStatus::Pending);
    assert!(
        requests::list_requests(&pool, &other, CalendarRequestQuery::default())
            .await
            .unwrap()
            .records
            .is_empty()
    );
    assert!(requests::list_requests(
        &pool,
        &own,
        CalendarRequestQuery {
            review: true,
            ..Default::default()
        }
    )
    .await
    .is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, original_event_count);
    let event_payload = payload(
        date,
        fixture.academic_year_id,
        "Approved event",
        vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: None,
            homeroom_id: None,
        }],
    );
    assert!(
        requests::approve_request(&pool, &own, request.id, event_payload.clone())
            .await
            .is_err()
    );
    let mut invalid = event_payload.clone();
    invalid.tag_ids = vec![Uuid::new_v4()];
    assert!(
        requests::approve_request(&pool, &manager, request.id, invalid)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, original_event_count);
    let (first, second) = tokio::join!(
        requests::approve_request(&pool, &manager, request.id, event_payload.clone()),
        requests::approve_request(&pool, &manager, request.id, event_payload)
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, original_event_count + 1);
    assert!(
        requests::reject_request(&pool, &manager, request.id, "Too late")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejected_request_retains_reason_without_creating_event() {
    use school_authorization::ActorContext;
    use school_calendar::requests::{self, CalendarRequestStatus, CreateCalendarRequest};
    use school_permissions::registry::codes;
    let pool = migrated_pool("calendar_request_rejected").await;
    let fixture = insert_fixture(&pool).await;
    let actor = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![
            codes::CALENDAR_READ_SCHOOL.into(),
            codes::CALENDAR_REQUEST_OWN.into(),
            codes::CALENDAR_MANAGE_SCHOOL.into(),
        ],
    };
    let date = calendar_today();
    let request = requests::create_request(
        &pool,
        &actor,
        CreateCalendarRequest {
            title: "Requested event".into(),
            description: "Request details".into(),
            location: None,
            start_date: date,
            end_date: date,
            all_day: true,
            start_time: None,
            end_time: None,
        },
    )
    .await
    .unwrap();
    assert!(requests::reject_request(&pool, &actor, request.id, " ")
        .await
        .is_err());
    let result = requests::reject_request(&pool, &actor, request.id, "วันที่ซ้ำกับกิจกรรมอื่น")
        .await
        .unwrap();
    assert_eq!(result.status, CalendarRequestStatus::Rejected);
    assert_eq!(result.rejection_reason.as_deref(), Some("วันที่ซ้ำกับกิจกรรมอื่น"));
    assert!(result.event_id.is_none());
    assert!(
        services::list_management_events(&pool, query_around(date, fixture.academic_year_id))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn duplicate_category_and_tag_names_keep_existing_conflict_messages() {
    let pool = migrated_pool("calendar_duplicate_names").await;
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
    let pool = migrated_pool("calendar_audience_views").await;
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
            Some(fixture.homeroom_id),
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
            Some(fixture.homeroom_id),
        )],
    )
    .await;

    let management =
        services::list_management_events(&pool, query_around(today, fixture.academic_year_id))
            .await
            .expect("management events should list");
    let staff = services::list_my_events(
        &pool,
        fixture.staff_user_id,
        query_around(today, fixture.academic_year_id),
    )
    .await
    .expect("staff events should list");
    let student = services::list_my_events(
        &pool,
        fixture.student_user_id,
        query_around(today, fixture.academic_year_id),
    )
    .await
    .expect("student events should list");
    let child = school_calendar::services::list_child_events(
        &pool,
        fixture.parent_user_id,
        fixture.student_user_id,
        query_around(today, fixture.academic_year_id),
    )
    .await
    .expect("child events should list");
    let public = services::list_public_events(&pool, query_around(today, fixture.academic_year_id))
        .await
        .expect("public events should list");

    let ids = |events: &[school_calendar::models::CalendarEvent]| {
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
    let pool = migrated_pool("calendar_recipient_resolution").await;
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
                Some(fixture.homeroom_id),
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
                Some(fixture.homeroom_id),
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
    let pool = migrated_pool("calendar_reminder_idempotency").await;
    let fixture = insert_fixture(&pool).await;
    let today = calendar_today();
    let created = services::create_event(
        &pool,
        fixture.staff_user_id,
        event_request(
            fixture.academic_year_id,
            "Reminder delivery",
            today + Duration::days(1),
            false,
            Vec::new(),
            vec![target(
                CalendarAudienceType::Parent,
                None,
                Some(fixture.homeroom_id),
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
    let pool = migrated_pool("calendar_no_recipients").await;
    let fixture = insert_fixture(&pool).await;
    let today = calendar_today();
    let created = services::create_event(
        &pool,
        fixture.staff_user_id,
        event_request(
            fixture.academic_year_id,
            "No active recipients",
            today + Duration::days(1),
            false,
            Vec::new(),
            vec![target(
                CalendarAudienceType::Parent,
                None,
                Some(fixture.homeroom_id),
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

async fn grant_calendar_role(pool: &PgPool, user: Uuid, permissions: &[&str]) -> Uuid {
    let role = Uuid::new_v4();
    sqlx::query("INSERT INTO roles(id,code,name) VALUES($1,$2,'Synthetic calendar role')")
        .bind(role)
        .bind(format!("cal-{}", role))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=ANY($2)")
        .bind(role).bind(permissions).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(user)
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
    role
}

#[tokio::test]
async fn request_recipient_resolution_matches_actor_grants_and_excludes_ineligible_users() {
    use school_authorization::{
        load_actor_context, staff_users_with_all_permissions, PermissionCache,
    };
    use school_permissions::registry::codes;
    let pool = migrated_pool("calendar_request_recipients").await;
    let required = [codes::CALENDAR_READ_SCHOOL, codes::CALENDAR_MANAGE_SCHOOL];
    let manager = insert_user(&pool, "staff", "Role manager").await;
    grant_calendar_role(&pool, manager, &required).await;
    // Multiple grant sources must still produce a single recipient.
    grant_calendar_role(&pool, manager, &required).await;
    let wildcard = insert_user(&pool, "staff", "Wildcard manager").await;
    grant_calendar_role(&pool, wildcard, &[codes::WILDCARD]).await;
    let read_only = insert_user(&pool, "staff", "Reader").await;
    grant_calendar_role(&pool, read_only, &[required[0]]).await;
    let manage_only = insert_user(&pool, "staff", "No calendar read").await;
    grant_calendar_role(&pool, manage_only, &[required[1]]).await;
    let inactive = insert_user(&pool, "staff", "Inactive manager").await;
    grant_calendar_role(&pool, inactive, &required).await;
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(inactive)
        .execute(&pool)
        .await
        .unwrap();
    let student = insert_user(&pool, "student", "Ineligible student").await;
    grant_calendar_role(&pool, student, &required).await;
    let ended = insert_user(&pool, "staff", "Ended role").await;
    grant_calendar_role(&pool, ended, &required).await;
    sqlx::query("UPDATE user_roles SET ended_at=CURRENT_DATE WHERE user_id=$1")
        .bind(ended)
        .execute(&pool)
        .await
        .unwrap();
    let disabled = insert_user(&pool, "staff", "Disabled role").await;
    let role = grant_calendar_role(&pool, disabled, &required).await;
    sqlx::query("UPDATE roles SET is_active=false WHERE id=$1")
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    let unit = Uuid::new_v4();
    sqlx::query("INSERT INTO organization_units(id,code,name) VALUES($1,'calendar-fixture','Synthetic calendar unit')").bind(unit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code) SELECT $1,id,'head' FROM permissions WHERE code=ANY($2)").bind(unit).bind(&required).execute(&pool).await.unwrap();
    let head = insert_user(&pool, "staff", "Unit head").await;
    let member = insert_user(&pool, "staff", "Unit member").await;
    for (user, position) in [(head, "head"), (member, "member")] {
        sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,$3)").bind(user).bind(unit).bind(position).execute(&pool).await.unwrap();
    }
    let delegated = insert_user(&pool, "staff", "Delegated manager").await;
    let expired = insert_user(&pool, "staff", "Expired delegation").await;
    let revoked = insert_user(&pool, "staff", "Revoked delegation").await;
    for user in [delegated, expired, revoked] {
        sqlx::query("INSERT INTO organization_permission_delegations(from_user_id,to_user_id,permission_id) SELECT $1,$2,id FROM permissions WHERE code=ANY($3)").bind(manager).bind(user).bind(&required).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE organization_permission_delegations SET expires_at=NOW()-INTERVAL '1 hour' WHERE to_user_id=$1").bind(expired).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE organization_permission_delegations SET revoked_at=NOW() WHERE to_user_id=$1",
    )
    .bind(revoked)
    .execute(&pool)
    .await
    .unwrap();
    let recipients = staff_users_with_all_permissions(&pool, &required)
        .await
        .unwrap();
    let cache = PermissionCache::new();
    for user in [
        manager,
        wildcard,
        read_only,
        manage_only,
        inactive,
        ended,
        disabled,
        head,
        member,
        delegated,
        expired,
        revoked,
    ] {
        let actor = load_actor_context(user, "calendar-recipient-test", &pool, &cache)
            .await
            .unwrap();
        assert_eq!(
            recipients.contains(&user),
            actor.has_all_permissions(&required),
            "batch resolution must agree with actor authorization for {user}"
        );
    }
    for allowed in [manager, wildcard, head, delegated] {
        assert!(recipients.contains(&allowed));
    }
    for denied in [
        read_only,
        manage_only,
        inactive,
        student,
        ended,
        disabled,
        member,
        expired,
        revoked,
    ] {
        assert!(!recipients.contains(&denied));
    }
    assert_eq!(recipients.iter().filter(|id| **id == manager).count(), 1);
    sqlx::query("UPDATE organization_units SET is_active=false WHERE id=$1")
        .bind(unit)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!staff_users_with_all_permissions(&pool, &required)
        .await
        .unwrap()
        .contains(&head));
}

#[tokio::test]
async fn request_workflow_notifies_managers_then_only_requester_for_one_successful_decision() {
    use school_authorization::ActorContext;
    use school_calendar::requests::{CalendarRequestStatus, CreateCalendarRequest};
    use school_permissions::registry::codes;
    let pool = migrated_pool("calendar_request_notification_workflow").await;
    let fixture = insert_fixture(&pool).await;
    let manager_id = insert_user(&pool, "staff", "Review manager").await;
    let required = [codes::CALENDAR_READ_SCHOOL, codes::CALENDAR_MANAGE_SCHOOL];
    grant_calendar_role(&pool, manager_id, &required).await;
    grant_calendar_role(&pool, manager_id, &required).await;
    let actor = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![required[0].into(), codes::CALENDAR_REQUEST_OWN.into()],
    };
    let manager = ActorContext {
        user_id: manager_id,
        permissions: required.iter().map(ToString::to_string).collect(),
    };
    let (tx, mut rx) = broadcast::channel::<TenantNotificationEvent>(32);
    let workflow = services::CalendarRequestWorkflow::new(&pool, &tx, "synthetic-tenant");
    let payload = CreateCalendarRequest {
        title: "Synthetic calendar request".into(),
        description: "Synthetic description".into(),
        location: None,
        start_date: calendar_today(),
        end_date: calendar_today(),
        all_day: true,
        start_time: None,
        end_time: None,
    };
    let request = workflow.create(&actor, payload.clone()).await.unwrap();
    let created = rx.try_recv().unwrap();
    assert_eq!(created.tenant, "synthetic-tenant");
    assert_eq!(created.user_id, manager_id);
    assert_eq!(created.notification.type_, "info");
    assert!(created.notification.message.contains(&request.title));
    assert_eq!(
        created.notification.link.as_deref(),
        Some("/staff/calendar/requests?review=true&status=pending")
    );
    assert!(
        rx.try_recv().is_err(),
        "duplicate grant must not duplicate notification"
    );
    let mut event = payload_event_for_pending_test(calendar_today(), fixture.academic_year_id);
    event.notify_audience = false;
    let (one, two) = tokio::join!(
        workflow.approve(&manager, request.id, event.clone()),
        workflow.approve(&manager, request.id, event)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    let decision = rx.try_recv().unwrap();
    assert_eq!(decision.user_id, actor.user_id);
    assert_eq!(decision.tenant, "synthetic-tenant");
    assert_eq!(decision.notification.type_, "success");
    assert_eq!(
        decision.notification.link.as_deref(),
        Some("/staff/calendar/requests?status=approved")
    );
    assert!(rx.try_recv().is_err());
    assert!(workflow
        .reject(&manager, request.id, "already approved")
        .await
        .is_err());
    assert!(rx.try_recv().is_err());
    let rejected = workflow.create(&actor, payload.clone()).await.unwrap();
    assert_eq!(rx.try_recv().unwrap().user_id, manager_id);
    let rejected = workflow
        .reject(&manager, rejected.id, "Synthetic rejection reason")
        .await
        .unwrap();
    assert_eq!(rejected.status, CalendarRequestStatus::Rejected);
    assert_eq!(
        rejected.rejection_reason.as_deref(),
        Some("Synthetic rejection reason")
    );
    let decision = rx.try_recv().unwrap();
    assert_eq!(decision.user_id, actor.user_id);
    assert_eq!(decision.notification.type_, "warning");
    assert_eq!(
        decision.notification.link.as_deref(),
        Some("/staff/calendar/requests?status=rejected")
    );
    assert!(rx.try_recv().is_err());
    let invalid = CreateCalendarRequest {
        title: String::new(),
        ..payload
    };
    assert!(workflow.create(&actor, invalid).await.is_err());
    assert!(rx.try_recv().is_err());
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM notifications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 4);
    let own_links: Vec<String> =
        sqlx::query_scalar("SELECT link FROM notifications WHERE user_id=$1 ORDER BY created_at")
            .bind(actor.user_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        own_links,
        vec![
            "/staff/calendar/requests?status=approved",
            "/staff/calendar/requests?status=rejected"
        ]
    );
}

#[tokio::test]
async fn request_workflow_keeps_committed_mutations_when_notification_storage_fails() {
    use school_authorization::ActorContext;
    use school_calendar::requests::{CalendarRequestStatus, CreateCalendarRequest};
    use school_permissions::registry::codes;
    let pool = migrated_pool("calendar_request_notification_failure").await;
    let fixture = insert_fixture(&pool).await;
    let manager_id = insert_user(&pool, "staff", "Manager").await;
    let required = [codes::CALENDAR_READ_SCHOOL, codes::CALENDAR_MANAGE_SCHOOL];
    grant_calendar_role(&pool, manager_id, &required).await;
    sqlx::query(
        "ALTER TABLE notifications ADD CONSTRAINT synthetic_notification_failure CHECK (false)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let actor = ActorContext {
        user_id: fixture.staff_user_id,
        permissions: vec![required[0].into(), codes::CALENDAR_REQUEST_OWN.into()],
    };
    let manager = ActorContext {
        user_id: manager_id,
        permissions: required.iter().map(ToString::to_string).collect(),
    };
    let (tx, mut rx) = broadcast::channel::<TenantNotificationEvent>(8);
    let workflow = services::CalendarRequestWorkflow::new(&pool, &tx, "failure-test");
    let request = workflow
        .create(
            &actor,
            CreateCalendarRequest {
                title: "Storage failure fixture".into(),
                description: "Synthetic".into(),
                location: None,
                start_date: calendar_today(),
                end_date: calendar_today(),
                all_day: true,
                start_time: None,
                end_time: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(request.status, CalendarRequestStatus::Pending);
    let outcome = workflow
        .approve(
            &manager,
            request.id,
            payload_event_for_pending_test(calendar_today(), fixture.academic_year_id),
        )
        .await
        .unwrap();
    assert_eq!(outcome.request.status, CalendarRequestStatus::Approved);
    assert!(outcome.request.event_id.is_some());
    assert!(rx.try_recv().is_err());
    assert!(workflow
        .approve(
            &manager,
            request.id,
            payload_event_for_pending_test(calendar_today(), fixture.academic_year_id)
        )
        .await
        .is_err());
}
