use chrono::NaiveDate;
use uuid::Uuid;

use super::timetable_block_service;
use crate::error::AppError;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::modules::academic::models::timetable_block::{
    CreateOrdinaryTimetableBlockRequest, CreateStructuralTimetableBlocksRequest,
    CreateSynchronizedTimetableBlockRequest, RemoveTimetableBlockTargetRequest,
    RestoreTimetableBlockGroupRequest, RetryTimetableBlockSyncRequest, TimetableBlockSyncStatus,
    TimetableBlockWorkspaceQuery, TimetableStructuralKind, TimetableStructuralSlotInput,
    TimetableTargetKind, UpdateTimetableBlockRequest,
};
use crate::policies::timetable_access_policy::TimetableAccessFilter;
use crate::test_helpers::create_named_test_pool;

const ACTOR_ID: &str = "50000000-0000-0000-0000-000000000002";

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 59).await.unwrap();
    sqlx::query(
        r#"INSERT INTO bell_schedule_periods (
               id, bell_schedule_id, name,
               start_time, end_time, order_index, applicable_days
           )
           SELECT gen_random_uuid(), schedule.id,
                  'คาบทดสอบ', TIME '09:00', TIME '09:50', 1, 'MON-FRI'
           FROM bell_schedules schedule
           WHERE schedule.is_default
             AND NOT EXISTS (
                 SELECT 1 FROM bell_schedule_periods period
                 WHERE period.bell_schedule_id = schedule.id
                   AND period.order_index = 1
             )"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn draft_version(pool: &sqlx::PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let actor_id = Uuid::parse_str(ACTOR_ID).unwrap();
    let (source_id, term_id, year_id, bell_schedule_id, starts_on): (
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        NaiveDate,
    ) = sqlx::query_as(
        r#"SELECT version.id, version.academic_term_id, version.academic_year_id,
                  version.bell_schedule_id,
                  (SELECT max(live.effective_from) + 1
                   FROM academic_timetable_versions live
                   WHERE live.academic_term_id = version.academic_term_id
                     AND live.status IN ('draft', 'published'))
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.status = 'published'
             AND term.status = 'active'
           ORDER BY version.effective_from, version.id
           LIMIT 1"#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let version_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_timetable_versions (
               id, academic_term_id, academic_year_id, effective_from, status,
               source_version_id, bell_schedule_id, created_by
           ) VALUES ($1, $2, $3, $4, 'draft', $5, $6, $7)"#,
    )
    .bind(version_id)
    .bind(term_id)
    .bind(year_id)
    .bind(starts_on)
    .bind(source_id)
    .bind(bell_schedule_id)
    .bind(actor_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO academic_timetable_version_targets (
               timetable_version_id, learning_offering_id, academic_term_id,
               academic_year_id, weekly_period_target, migration_provenance
           )
           SELECT $1, learning_offering_id, academic_term_id,
                  academic_year_id, weekly_period_target, '{}'::jsonb
           FROM academic_timetable_version_targets
           WHERE timetable_version_id = $2"#,
    )
    .bind(version_id)
    .bind(source_id)
    .execute(pool)
    .await
    .unwrap();
    (version_id, term_id, year_id, bell_schedule_id)
}

#[tokio::test]
async fn ordinary_block_keeps_exact_instructors_and_rejects_cross_block_conflict() {
    let pool = migrated_pool("timetable_block_ordinary").await;
    let actor_id = Uuid::parse_str(ACTOR_ID).unwrap();
    let (version_id, term_id, year_id, bell_schedule_id) = draft_version(&pool).await;
    let (offering_id, offering_year_id, starts_on): (Uuid, Uuid, NaiveDate) = sqlx::query_as(
        r#"SELECT offering.id, offering.academic_year_id, offering.starts_on
           FROM learning_offerings offering
           WHERE offering.academic_term_id = $1
             AND offering.kind = 'course'
           ORDER BY offering.id
           LIMIT 1"#,
    )
    .bind(term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let group_id = Uuid::new_v4();
    let teacher_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name,
               user_type, status
           ) VALUES ($1, $2, $3, 'fixture-not-a-login', 'ครูทดสอบ', 'ตารางสอน',
                     'staff', 'active')"#,
    )
    .bind(teacher_id)
    .bind(format!("{teacher_id}@example.invalid"))
    .bind(format!("timetable-{teacher_id}"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status
           ) VALUES ($1, $2, $3, $4, $5, 'กลุ่มทดสอบตารางสอน', 'draft', 'draft')"#,
    )
    .bind(group_id)
    .bind(offering_id)
    .bind(term_id)
    .bind(offering_year_id)
    .bind(format!("BLOCK-{}", &group_id.to_string()[..8]))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_group_teachers (
               id, learning_group_id, academic_term_id, academic_year_id,
               teacher_id, role, starts_on, created_by, updated_by
           ) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'primary', $5, $6, $6)"#,
    )
    .bind(group_id)
    .bind(term_id)
    .bind(offering_year_id)
    .bind(teacher_id)
    .bind(starts_on)
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    let period_id: Uuid = sqlx::query_scalar(
        r#"SELECT id FROM bell_schedule_periods
           WHERE bell_schedule_id = $1 AND is_active
           ORDER BY order_index, id LIMIT 1"#,
    )
    .bind(bell_schedule_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let request = CreateOrdinaryTimetableBlockRequest {
        timetable_version_id: version_id,
        academic_term_id: term_id,
        learning_group_id: group_id,
        day_of_week: "MON".to_string(),
        bell_schedule_period_id: period_id,
        room_id: None,
        instructor_ids: vec![teacher_id],
        note: None,
    };

    let block = timetable_block_service::create_ordinary_block(&pool, actor_id, request.clone())
        .await
        .expect("ordinary placement must succeed");
    assert_eq!(block.groups.len(), 1);
    assert_eq!(block.groups[0].instructors.len(), 1);
    assert_eq!(block.groups[0].instructors[0].teacher_id, teacher_id);

    let workspace = timetable_block_service::get_workspace(
        &pool,
        TimetableBlockWorkspaceQuery {
            academic_year_id: year_id,
            academic_term_id: term_id,
            timetable_version_id: version_id,
        },
        &TimetableAccessFilter {
            includes_school_owned: true,
            ..TimetableAccessFilter::default()
        },
    )
    .await
    .expect("canonical block workspace must hydrate in bounded queries");
    assert!(workspace
        .blocks
        .iter()
        .any(|candidate| candidate.id == block.id));
    assert!(workspace
        .ordinary_demands
        .iter()
        .any(|demand| demand.learning_group_id == group_id));

    assert!(matches!(
        timetable_block_service::create_ordinary_block(&pool, actor_id, request).await,
        Err(AppError::Conflict(_))
    ));
    let moved = timetable_block_service::update_block(
        &pool,
        actor_id,
        block.id,
        UpdateTimetableBlockRequest {
            timetable_version_id: version_id,
            row_version: block.row_version,
            day_of_week: Some("TUE".to_string()),
            bell_schedule_period_id: None,
            title: None,
            clear_title: false,
            note: Some("ย้ายด้วยการลาก".to_string()),
            clear_note: false,
            room_id: None,
            clear_room: false,
            instructor_ids: Some(vec![teacher_id]),
        },
    )
    .await
    .expect("one-period drag must move the canonical block with its exact instructors");
    assert_eq!(moved.day_of_week, "TUE");
    assert_eq!(moved.groups[0].instructors[0].teacher_id, teacher_id);
}

#[tokio::test]
async fn synchronized_zero_group_and_structural_per_target_removal_are_canonical() {
    let pool = migrated_pool("timetable_block_sync_structural").await;
    let actor_id = Uuid::parse_str(ACTOR_ID).unwrap();
    let (version_id, term_id, year_id, bell_schedule_id) = draft_version(&pool).await;
    let period_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM bell_schedule_periods
           WHERE bell_schedule_id = $1 AND is_active
           ORDER BY order_index, id LIMIT 2"#,
    )
    .bind(bell_schedule_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let homeroom_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM homerooms WHERE academic_year_id = $1 AND is_active",
    )
    .bind(year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    if homeroom_count < 2 {
        let extra_homeroom_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO homerooms (
                   id, code, name, academic_year_id, grade_level_id, room_number,
                   study_program_id, capacity, is_active
               )
               SELECT $1, $2, 'ห้องทดสอบตารางสอน', academic_year_id,
                      grade_level_id, $3, study_program_id, capacity, true
               FROM homerooms
               WHERE academic_year_id = $4 AND is_active
               ORDER BY id LIMIT 1"#,
        )
        .bind(extra_homeroom_id)
        .bind(format!("BLOCK-{}", &extra_homeroom_id.to_string()[..8]))
        .bind(format!("BLOCK-{}", &extra_homeroom_id.to_string()[..8]))
        .bind(year_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let homeroom_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM homerooms
           WHERE academic_year_id = $1 AND is_active
           ORDER BY id LIMIT 2"#,
    )
    .bind(year_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!period_ids.is_empty());
    assert_eq!(homeroom_ids.len(), 2);

    let synchronized_offering_id = Uuid::new_v4();
    sqlx::query_scalar::<_, Uuid>(
        r#"WITH source AS (
               SELECT offering.academic_term_id, offering.academic_year_id,
                      offering.owning_organization_unit_id, offering.starts_on,
                      detail.activity_version_id, detail.activity_id,
                      detail.registration_type, detail.hours
               FROM learning_offerings offering
               JOIN activity_offering_details detail
                 ON detail.learning_offering_id = offering.id
               WHERE offering.academic_term_id = $2
                 AND detail.scheduling_mode = 'synchronized'
               ORDER BY offering.id LIMIT 1
           ), inserted_offering AS (
               INSERT INTO learning_offerings (
                   id, academic_term_id, academic_year_id, kind, code_snapshot,
                   name_snapshot, status, published_at, owning_organization_unit_id,
                   starts_on, migration_provenance
               )
               SELECT $1, academic_term_id, academic_year_id,
                      'activity', 'SYNC-ZERO', 'กิจกรรมยังไม่มีกลุ่ม', 'draft', NULL,
                      owning_organization_unit_id, starts_on, '{}'::jsonb
               FROM source
               RETURNING id
           ), inserted_detail AS (
               INSERT INTO activity_offering_details (
                   learning_offering_id, academic_term_id, academic_year_id,
                   activity_version_id, activity_id, registration_type,
                   scheduling_mode, hours, attendance_requirement, pass_criteria,
                   migration_provenance
               )
               SELECT inserted_offering.id, source.academic_term_id, source.academic_year_id,
                      source.activity_version_id, source.activity_id, source.registration_type,
                      'synchronized', source.hours, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb
               FROM source CROSS JOIN inserted_offering
               RETURNING learning_offering_id
           )
           SELECT learning_offering_id FROM inserted_detail"#,
    )
    .bind(synchronized_offering_id)
    .bind(term_id)
    .fetch_one(&pool)
    .await
    .expect("fixture must create a synchronized offering without groups");
    sqlx::query(
        "UPDATE learning_offerings SET status = 'published', published_at = now() WHERE id = $1",
    )
    .bind(synchronized_offering_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO academic_timetable_version_targets (
               timetable_version_id, learning_offering_id, academic_term_id,
               academic_year_id, weekly_period_target, migration_provenance
           ) VALUES ($1, $2, $3, $4, 1, '{}'::jsonb)
           ON CONFLICT (timetable_version_id, learning_offering_id) DO NOTHING"#,
    )
    .bind(version_id)
    .bind(synchronized_offering_id)
    .bind(term_id)
    .bind(year_id)
    .execute(&pool)
    .await
    .unwrap();

    let sync_block = timetable_block_service::create_synchronized_block(
        &pool,
        actor_id,
        CreateSynchronizedTimetableBlockRequest {
            timetable_version_id: version_id,
            academic_term_id: term_id,
            learning_offering_id: synchronized_offering_id,
            day_of_week: "WED".to_string(),
            bell_schedule_period_id: period_ids[0],
            intended_homeroom_ids: homeroom_ids.clone(),
            room_id: None,
            note: None,
        },
    )
    .await
    .expect("a synchronized block must exist before Delivery groups");
    assert!(sync_block.groups.is_empty());
    assert!(sync_block.sync_states.is_empty());
    assert_eq!(sync_block.homerooms.len(), 2);

    let synchronized_group_id = Uuid::new_v4();
    let teacher_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type = 'staff' AND status = 'active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let offering_starts_on: NaiveDate =
        sqlx::query_scalar("SELECT starts_on FROM learning_offerings WHERE id = $1")
            .bind(synchronized_offering_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status
           ) VALUES ($1, $2, $3, $4, 'SYNC-GROUP', 'กลุ่มกิจกรรมทดสอบ', 'draft', 'draft')"#,
    )
    .bind(synchronized_group_id)
    .bind(synchronized_offering_id)
    .bind(term_id)
    .bind(year_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_group_homerooms (
               id, learning_group_id, academic_term_id, academic_year_id,
               homeroom_id, coverage_source, migration_provenance
           ) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'manual', '{}'::jsonb)"#,
    )
    .bind(synchronized_group_id)
    .bind(term_id)
    .bind(year_id)
    .bind(homeroom_ids[0])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_group_teachers (
               id, learning_group_id, academic_term_id, academic_year_id,
               teacher_id, role, starts_on, created_by, updated_by
           ) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'primary', $5, $6, $6)"#,
    )
    .bind(synchronized_group_id)
    .bind(term_id)
    .bind(year_id)
    .bind(teacher_id)
    .bind(offering_starts_on)
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    let linked = timetable_block_service::retry_sync(
        &pool,
        actor_id,
        sync_block.id,
        RetryTimetableBlockSyncRequest {
            timetable_version_id: version_id,
            block_row_version: sync_block.row_version,
            learning_group_ids: vec![synchronized_group_id],
        },
    )
    .await
    .expect("a later Delivery group must synchronize into the reserved block");
    assert_eq!(
        linked.groups.len(),
        1,
        "sync states: {:?}",
        linked.sync_states
    );
    assert_eq!(linked.groups[0].instructors.len(), 1);
    assert_eq!(
        linked.sync_states[0].status,
        TimetableBlockSyncStatus::Linked
    );

    let excluded = timetable_block_service::remove_target(
        &pool,
        actor_id,
        linked.id,
        RemoveTimetableBlockTargetRequest {
            timetable_version_id: version_id,
            block_row_version: linked.row_version,
            target_kind: TimetableTargetKind::Group,
            target_id: linked.groups[0].id,
            target_row_version: linked.groups[0].row_version,
        },
    )
    .await
    .expect("one synchronized group must be excludable without removing the block");
    assert!(excluded.groups.is_empty());
    assert_eq!(
        excluded.sync_states[0].status,
        TimetableBlockSyncStatus::Excluded
    );
    let restored = timetable_block_service::restore_group(
        &pool,
        actor_id,
        excluded.id,
        RestoreTimetableBlockGroupRequest {
            timetable_version_id: version_id,
            block_row_version: excluded.row_version,
            learning_group_id: synchronized_group_id,
        },
    )
    .await
    .expect("an explicitly excluded group must restore only through the explicit action");
    assert_eq!(restored.groups.len(), 1);
    assert_eq!(
        restored.sync_states[0].status,
        TimetableBlockSyncStatus::Linked
    );

    let structural = timetable_block_service::create_structural_blocks(
        &pool,
        actor_id,
        CreateStructuralTimetableBlocksRequest {
            timetable_version_id: version_id,
            academic_term_id: term_id,
            structural_kind: TimetableStructuralKind::FlagCeremony,
            title: "กิจกรรมหน้าเสาธง".to_string(),
            note: None,
            slots: vec![TimetableStructuralSlotInput {
                day_of_week: "TUE".to_string(),
                bell_schedule_period_id: period_ids[0],
            }],
            homeroom_ids: homeroom_ids.clone(),
            teacher_ids: Vec::new(),
            all_homerooms: false,
            all_teachers: false,
            room_id: None,
        },
    )
    .await
    .expect("structural block must be created once with explicit targets");
    assert_eq!(structural.len(), 1);
    assert_eq!(structural[0].homerooms.len(), 2);

    let removed_target = structural[0].homerooms[0].clone();
    let updated = timetable_block_service::remove_target(
        &pool,
        actor_id,
        structural[0].id,
        RemoveTimetableBlockTargetRequest {
            timetable_version_id: version_id,
            block_row_version: structural[0].row_version,
            target_kind: TimetableTargetKind::Homeroom,
            target_id: removed_target.id,
            target_row_version: removed_target.row_version,
        },
    )
    .await
    .expect("one homeroom target must be removable without deleting the series");
    assert_eq!(updated.homerooms.len(), 1);
    assert_eq!(updated.homerooms[0].homeroom_id, homeroom_ids[1]);
}
