use chrono::NaiveDate;
use uuid::Uuid;

use super::{daily_teaching_service, timetable_service};
use crate::error::AppError;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::modules::academic::models::timetable::{
    CreateBatchTimetableEntriesRequest, CreateTimetableEntryRequest, SwapTimetableEntriesRequest,
    TimetableQuery,
};
use crate::modules::academic::models::timetable_version::CloneTimetableVersionRequest;
use crate::modules::academic::models::timetable_version::TimetableVersion;
use crate::policies::resource_access_policy::AcademicResourceListFilter;
use crate::test_helpers::create_named_test_pool;

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 53).await.unwrap();
    sqlx::query(
        r#"INSERT INTO bell_schedule_periods (
               id, bell_schedule_id, name,
               start_time, end_time, order_index, applicable_days
           )
           SELECT gen_random_uuid(), schedule.id,
                  'คาบทดสอบ 2', TIME '10:00', TIME '10:50', 2, 'MON-FRI'
           FROM bell_schedules schedule
           WHERE schedule.is_default
             AND NOT EXISTS (
                 SELECT 1 FROM bell_schedule_periods period
                 WHERE period.bell_schedule_id = schedule.id
                   AND period.order_index = 2
             )"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn clone_editable_version(pool: &sqlx::PgPool) -> TimetableVersion {
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (source_id, source_row_version, term_start): (Uuid, i64, NaiveDate) = sqlx::query_as(
        r#"SELECT version.id, version.row_version, term.start_date
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.status = 'published' AND term.status = 'active'
           ORDER BY version.effective_from, version.id LIMIT 1"#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    super::timetable_version_service::clone_draft(
        pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from: term_start.succ_opt().unwrap(),
            source_row_version,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn timetable_mutations_and_reads_are_isolated_by_version() {
    let pool = migrated_pool("timetable_version_isolation").await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let draft = clone_editable_version(&pool).await;
    let source_version_id = draft.source_version_id.unwrap();
    let (term_id, group_id, day, period_id): (Uuid, Uuid, String, Uuid) = sqlx::query_as(
        r#"SELECT entry.academic_term_id, entry.learning_group_id,
                  entry.day_of_week, entry.bell_schedule_period_id
           FROM academic_timetable_entries entry
           WHERE entry.timetable_version_id = $1
             AND entry.learning_group_id IS NOT NULL
             AND entry.is_active
           ORDER BY entry.id LIMIT 1"#,
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let draft_entry_id: Uuid = sqlx::query_scalar(
        r#"SELECT id FROM academic_timetable_entries
           WHERE timetable_version_id = $1
             AND learning_group_id = $2
             AND day_of_week = $3
             AND bell_schedule_period_id = $4
             AND is_active
           ORDER BY id LIMIT 1"#,
    )
    .bind(draft.id)
    .bind(group_id)
    .bind(&day)
    .bind(period_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_timetable_entries SET is_active = false WHERE id = $1")
        .bind(draft_entry_id)
        .execute(&pool)
        .await
        .unwrap();

    let request = CreateTimetableEntryRequest {
        timetable_version_id: draft.id,
        academic_term_id: term_id,
        learning_group_id: Some(group_id),
        homeroom_id: None,
        day_of_week: day.clone(),
        bell_schedule_period_id: period_id,
        room_id: None,
        note: Some("version isolated".to_string()),
        entry_type: "course".to_string(),
        title: None,
        instructor_ids: Vec::new(),
    };
    let created = timetable_service::create_entry(&pool, actor_id, request.clone())
        .await
        .unwrap();
    assert_eq!(created.timetable_version_id, draft.id);

    let conflict = timetable_service::create_entry(&pool, actor_id, request).await;
    assert!(matches!(conflict, Err(AppError::Conflict(_))));

    let published_mutation = timetable_service::create_entry(
        &pool,
        actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: source_version_id,
            academic_term_id: term_id,
            learning_group_id: Some(group_id),
            homeroom_id: None,
            day_of_week: "SUN".to_string(),
            bell_schedule_period_id: period_id,
            room_id: None,
            note: None,
            entry_type: "course".to_string(),
            title: None,
            instructor_ids: Vec::new(),
        },
    )
    .await;
    assert!(matches!(published_mutation, Err(AppError::Conflict(_))));

    let listed = timetable_service::list_entries(
        &pool,
        &TimetableQuery {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_id: Some(group_id),
            homeroom_id: None,
            instructor_id: None,
            room_id: None,
            day_of_week: None,
            entry_type: None,
        },
        &school_access(),
    )
    .await
    .unwrap();
    assert!(listed
        .iter()
        .all(|entry| entry.timetable_version_id == draft.id));
    let occupancy = timetable_service::occupancy(&pool, draft.id, term_id)
        .await
        .unwrap();
    assert!(occupancy.iter().any(|cell| cell.entry_id == created.id));
    assert!(!occupancy.iter().any(|cell| {
        listed.iter().all(|entry| entry.id != cell.entry_id) && cell.entry_id == draft_entry_id
    }));
}

fn school_access() -> AcademicResourceListFilter {
    AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn course_and_activity_groups_share_homeroom_conflict_detection() {
    let pool = migrated_pool("timetable_group_conflict").await;
    let draft = clone_editable_version(&pool).await;
    let (term_id, day, period_id, homeroom_id): (Uuid, String, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT entry.academic_term_id, entry.day_of_week,
                  entry.bell_schedule_period_id,
                  coalesce(entry.homeroom_id, coverage.homeroom_id)
           FROM academic_timetable_entries entry
           LEFT JOIN learning_group_homerooms coverage
             ON coverage.learning_group_id = entry.learning_group_id
           WHERE entry.timetable_version_id = $1
             AND entry.entry_type = 'COURSE' AND entry.is_active
           ORDER BY entry.id, coverage.homeroom_id
           LIMIT 1"#,
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let activity_group_id: Uuid = sqlx::query_scalar(
        r#"SELECT learning_group.id
           FROM learning_groups learning_group
           JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
           JOIN learning_group_homerooms coverage ON coverage.learning_group_id = learning_group.id
           WHERE offering.kind = 'activity'
             AND learning_group.academic_term_id = $1
             AND coverage.homeroom_id = $2
           ORDER BY learning_group.id
           LIMIT 1"#,
    )
    .bind(term_id)
    .bind(homeroom_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let result = timetable_service::create_entry(
        &pool,
        Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap(),
        CreateTimetableEntryRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_id: Some(activity_group_id),
            homeroom_id: None,
            day_of_week: day,
            bell_schedule_period_id: period_id,
            room_id: None,
            note: None,
            entry_type: "activity".to_string(),
            title: None,
            instructor_ids: Vec::new(),
        },
    )
    .await;
    assert!(
        matches!(result, Err(AppError::Conflict(message)) if message.contains("ห้องประจำชั้น") || message.contains("ครู"))
    );
}

#[tokio::test]
async fn student_timetable_uses_the_membership_interval_for_the_requested_date() {
    let pool = migrated_pool("timetable_student_membership_interval").await;
    let (version_id, academic_term_id, learning_group_id, student_id, membership_id, joined_at): (
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        NaiveDate,
    ) = sqlx::query_as(
        r#"SELECT version.id, entry.academic_term_id, entry.learning_group_id,
                  membership.student_id, membership.id, membership.joined_at
           FROM academic_timetable_versions version
           JOIN academic_timetable_entries entry
             ON entry.timetable_version_id = version.id
            AND entry.learning_group_id IS NOT NULL
            AND entry.is_active
           JOIN learning_group_students membership
             ON membership.learning_group_id = entry.learning_group_id
            AND membership.membership_status = 'active'
           JOIN learning_groups learning_group ON learning_group.id = entry.learning_group_id
           WHERE version.status = 'published'
             AND learning_group.roster_status IN ('published', 'closed')
           ORDER BY version.effective_from, entry.id, membership.id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .expect("fixture must contain a scheduled group with a published membership");
    let left_at = joined_at
        .checked_add_signed(chrono::Duration::days(1))
        .unwrap();
    sqlx::query(
        "UPDATE learning_group_students \
         SET membership_status = 'ended', left_at = $1, row_version = row_version + 1 \
         WHERE id = $2",
    )
    .bind(left_at)
    .bind(membership_id)
    .execute(&pool)
    .await
    .unwrap();

    let on_inclusive_end = timetable_service::list_student_entries(
        &pool,
        version_id,
        academic_term_id,
        student_id,
        left_at,
    )
    .await
    .unwrap();
    assert!(on_inclusive_end
        .iter()
        .any(|entry| entry.learning_group_id == Some(learning_group_id)));

    let after_end = timetable_service::list_student_entries(
        &pool,
        version_id,
        academic_term_id,
        student_id,
        left_at
            .checked_add_signed(chrono::Duration::days(1))
            .unwrap(),
    )
    .await
    .unwrap();
    assert!(!after_end
        .iter()
        .any(|entry| entry.learning_group_id == Some(learning_group_id)));
}

#[tokio::test]
async fn listing_occupancy_and_swaps_are_explicitly_term_and_group_scoped() {
    let pool = migrated_pool("timetable_term_group_swap").await;
    let draft = clone_editable_version(&pool).await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (entry_id, term_id, group_id, first_period_id, row_version): (Uuid, Uuid, Uuid, Uuid, i64) =
        sqlx::query_as(
            r#"SELECT id, academic_term_id, learning_group_id,
                  bell_schedule_period_id, row_version
           FROM academic_timetable_entries
           WHERE timetable_version_id = $1
             AND learning_group_id IS NOT NULL AND entry_type = 'COURSE'
           ORDER BY id LIMIT 1"#,
        )
        .bind(draft.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let second_period_id: Uuid = sqlx::query_scalar(
        r#"SELECT period.id
           FROM academic_terms term
           JOIN bell_schedule_periods period ON period.bell_schedule_id = term.bell_schedule_id
           WHERE term.id = $1 AND period.id <> $2 AND period.is_active
           ORDER BY period.order_index, period.id LIMIT 1"#,
    )
    .bind(term_id)
    .bind(first_period_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let created = timetable_service::create_entry(
        &pool,
        actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_id: Some(group_id),
            homeroom_id: None,
            day_of_week: "TUE".to_string(),
            bell_schedule_period_id: second_period_id,
            room_id: None,
            note: Some("canonical".to_string()),
            entry_type: "course".to_string(),
            title: None,
            instructor_ids: Vec::new(),
        },
    )
    .await
    .unwrap();

    let listed = timetable_service::list_entries(
        &pool,
        &TimetableQuery {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_id: Some(group_id),
            homeroom_id: None,
            instructor_id: None,
            room_id: None,
            day_of_week: None,
            entry_type: None,
        },
        &school_access(),
    )
    .await
    .unwrap();
    assert!(listed.iter().all(|entry| {
        entry.academic_term_id == term_id && entry.learning_group_id == Some(group_id)
    }));
    let listed_created = listed
        .iter()
        .find(|entry| entry.id == created.id)
        .expect("batch-hydrated list must retain the created entry");
    assert_eq!(
        listed_created
            .instructors
            .iter()
            .map(|instructor| instructor.user_id)
            .collect::<Vec<_>>(),
        created
            .instructors
            .iter()
            .map(|instructor| instructor.user_id)
            .collect::<Vec<_>>()
    );

    let occupancy = timetable_service::occupancy(&pool, draft.id, term_id)
        .await
        .unwrap();
    let created_cell = occupancy
        .iter()
        .find(|cell| cell.entry_id == created.id)
        .unwrap();
    assert!(!created_cell.homeroom_ids.is_empty());

    let swapped = timetable_service::swap_entries(
        &pool,
        actor_id,
        SwapTimetableEntriesRequest {
            timetable_version_id: draft.id,
            entry_a_id: entry_id,
            entry_a_row_version: row_version,
            entry_b_id: created.id,
            entry_b_row_version: created.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(swapped.entry_a.academic_term_id, term_id);
    assert_eq!(swapped.entry_b.academic_term_id, term_id);
    assert!(swapped.entry_a.row_version > row_version);
    assert!(swapped.entry_b.row_version > created.row_version);
}

#[tokio::test]
async fn batch_and_conflict_reads_preserve_results() {
    let pool = migrated_pool("timetable_batch_conflict_bulk_reads").await;
    let draft = clone_editable_version(&pool).await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (term_id, group_id, period_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT entry.academic_term_id, entry.learning_group_id, period.id
           FROM academic_timetable_entries entry
           JOIN academic_terms term ON term.id = entry.academic_term_id
           JOIN bell_schedule_periods period
             ON period.bell_schedule_id = term.bell_schedule_id
            AND period.order_index = 2
           WHERE entry.timetable_version_id = $1
             AND entry.learning_group_id IS NOT NULL
           ORDER BY entry.id
           LIMIT 1"#,
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let created = timetable_service::create_batch(
        &pool,
        actor_id,
        CreateBatchTimetableEntriesRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_ids: vec![group_id],
            homeroom_ids: Vec::new(),
            days_of_week: vec!["TUE".to_string(), "WED".to_string()],
            bell_schedule_period_ids: vec![period_id],
            entry_type: "course".to_string(),
            title: Some("bulk".to_string()),
            room_id: None,
            note: None,
            instructor_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    assert_eq!(created.entries.len(), 2);
    assert!(created
        .entries
        .iter()
        .all(|entry| entry.learning_group_id == Some(group_id)));

    let moves = timetable_service::validate_moves(&pool, draft.id, term_id, created.entries[0].id)
        .await
        .unwrap();
    assert_eq!(
        moves.iter().filter(|cell| cell.state == "source").count(),
        1
    );
    assert!(moves.len() >= 14);

    let deactivated =
        timetable_service::deactivate_batch(&pool, created.batch_id, draft.id, actor_id)
            .await
            .unwrap();
    let mut expected_deactivated_ids = created
        .entries
        .iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();
    expected_deactivated_ids.sort_unstable();
    assert_eq!(
        deactivated.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        expected_deactivated_ids
    );
    assert!(deactivated.iter().all(|entry| !entry.is_active));
}

#[tokio::test]
async fn daily_teaching_uses_group_and_offering_snapshots_in_requested_term() {
    let pool = migrated_pool("daily_teaching_group_snapshot").await;
    let term_id: Uuid = sqlx::query_scalar(
        "SELECT academic_term_id FROM academic_timetable_entries ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let overview = daily_teaching_service::get_daily_teaching_overview(
        &pool,
        daily_teaching_service::DailyTeachingQuery {
            academic_term_id: term_id,
            date: Some(NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()),
            include_empty_teachers: Some(false),
        },
    )
    .await
    .unwrap();
    assert_eq!(overview.academic_term_id, term_id);
    assert_eq!(overview.day_of_week, "MON");
    assert!(overview
        .teachers
        .iter()
        .flat_map(|teacher| &teacher.periods)
        .flat_map(|period| &period.entries)
        .any(|entry| {
            entry.learning_group_id.is_some()
                && entry.offering_id.is_some()
                && (entry.subject_id.is_some() || entry.activity_id.is_some())
        }));
}

#[test]
fn timetable_request_rejects_legacy_identity_fields() {
    let payload = serde_json::json!({
        "timetableVersionId": Uuid::new_v4(),
        "academicTermId": Uuid::new_v4(),
        "classroomCourseId": Uuid::new_v4(),
        "dayOfWeek": "MON",
        "bellSchedulePeriodId": Uuid::new_v4(),
        "entryType": "course"
    });
    assert!(serde_json::from_value::<CreateTimetableEntryRequest>(payload).is_err());
}
