use chrono::{Days, NaiveDate};
use uuid::Uuid;

use super::timetable_block_service;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use school_academic_timetable::models::timetable_block::{
    CreateOrdinaryTimetableBlockRequest, CreateStructuralTimetableBlocksRequest,
    CreateSynchronizedTimetableBlockRequest, RemoveTimetableBlockTargetRequest,
    RestoreTimetableBlockGroupRequest, RetryTimetableBlockSyncRequest, TimetableBlockSyncStatus,
    TimetableBlockWorkspaceQuery, TimetableStructuralKind, TimetableStructuralSlotInput,
    TimetableTargetKind, UpdateTimetableBlockRequest,
};
use school_academic_timetable::policy::TimetableAccessFilter;
use school_academic_timetable::services::daily_teaching as daily_teaching_service;
use school_errors::AppError;
use school_test_db::create_named_test_pool_with_max_connections;

const ACTOR_ID: &str = "50000000-0000-0000-0000-000000000002";

#[tokio::test]
async fn timetable_lifecycle_preserves_closed_blocks_and_individual_targets() {
    let pool = migrated_pool("timetable_lifecycle_blocks").await;
    let actor = Uuid::parse_str(ACTOR_ID).unwrap();
    let (version, term, year, bell) = draft_version(&pool).await;
    let period: Uuid = sqlx::query_scalar("SELECT id FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND is_active ORDER BY order_index LIMIT 1")
        .bind(bell).fetch_one(&pool).await.unwrap();
    let request = CreateStructuralTimetableBlocksRequest {
        timetable_version_id: version,
        academic_term_id: term,
        structural_kind: TimetableStructuralKind::FlagCeremony,
        title: "Closure boundary".into(),
        note: None,
        slots: ["MON", "TUE"]
            .into_iter()
            .map(|day| TimetableStructuralSlotInput {
                day_of_week: day.into(),
                bell_schedule_period_id: period,
            })
            .collect(),
        homeroom_ids: vec![],
        teacher_ids: vec![],
        all_homerooms: true,
        all_teachers: false,
        room_id: None,
    };
    let blocks = timetable_block_service::create_structural_blocks(&pool, actor, request.clone())
        .await
        .unwrap();
    let block = &blocks[0];
    let target = &block.homerooms[0];
    for (year_status, term_status) in [
        ("closed", "active"),
        ("active", "closed"),
        ("active", "cancelled"),
    ] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(year)
            .bind(year_status)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN start_date ELSE NULL END WHERE id=$1")
            .bind(term).bind(term_status).execute(&pool).await.unwrap();
        let mut create_request = request.clone();
        create_request.slots = vec![TimetableStructuralSlotInput {
            day_of_week: "WED".into(),
            bell_schedule_period_id: period,
        }];
        let outcomes = [
            timetable_block_service::create_structural_blocks(&pool, actor, create_request)
                .await
                .map(|_| ()),
            timetable_block_service::remove_target(
                &pool,
                actor,
                block.id,
                RemoveTimetableBlockTargetRequest {
                    timetable_version_id: version,
                    block_row_version: block.row_version,
                    target_kind: TimetableTargetKind::Homeroom,
                    target_id: target.id,
                    target_row_version: target.row_version,
                },
            )
            .await
            .map(|_| ()),
            timetable_block_service::update_block(
                &pool,
                actor,
                block.id,
                UpdateTimetableBlockRequest {
                    timetable_version_id: version,
                    row_version: block.row_version,
                    day_of_week: Some("THU".into()),
                    bell_schedule_period_id: None,
                    title: None,
                    clear_title: false,
                    note: None,
                    clear_note: false,
                    room_id: None,
                    clear_room: false,
                    instructor_ids: None,
                    teacher_ids: None,
                },
            )
            .await
            .map(|_| ()),
            timetable_block_service::swap_blocks(
                &pool,
                actor,
                school_academic_timetable::models::timetable_block::SwapTimetableBlocksRequest {
                    timetable_version_id: version,
                    block_a_id: block.id,
                    block_a_row_version: block.row_version,
                    block_b_id: blocks[1].id,
                    block_b_row_version: blocks[1].row_version,
                },
            )
            .await
            .map(|_| ()),
            timetable_block_service::deactivate_block(
                &pool,
                actor,
                block.id,
                version,
                block.row_version,
            )
            .await
            .map(|_| ()),
            timetable_block_service::deactivate_series(
                &pool,
                actor,
                block.series_id.unwrap(),
                version,
            )
            .await
            .map(|_| ()),
        ];
        for (operation, outcome) in outcomes.into_iter().enumerate() {
            assert!(
                matches!(outcome, Err(AppError::Conflict(_))),
                "{year_status}/{term_status}, operation {operation}: {outcome:?}"
            );
        }
        assert_eq!(
            serde_json::to_value(
                timetable_block_service::get_block(&pool, block.id)
                    .await
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(block).unwrap()
        );
    }
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(year)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closing',closed_on=NULL WHERE id=$1")
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
    let (block_id, row_version) = (block.id, block.row_version);
    let worker = tokio::spawn(async move {
        timetable_block_service::deactivate_block(
            &worker_pool,
            actor,
            block_id,
            version,
            row_version,
        )
        .await
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
    let version_available =
        sqlx::query("SELECT id FROM academic_timetable_versions WHERE id=$1 FOR UPDATE NOWAIT")
            .bind(version)
            .execute(&mut *probe)
            .await
            .is_ok();
    let block_available = if version_available {
        sqlx::query("SELECT id FROM academic_timetable_blocks WHERE id=$1 FOR UPDATE NOWAIT")
            .bind(block_id)
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
        waiting && version_available && block_available,
        "a waiting writer must not lock the version or block before the term"
    );
    assert!(outcome.is_ok(), "closing must remain writable: {outcome:?}");
}

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool_with_max_connections(test_name, 3).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
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
    let (source_id,term_id,year_id,bell_id,delivery_id):(Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT v.id,v.academic_term_id,v.academic_year_id,v.bell_schedule_id,v.delivery_version_id FROM academic_timetable_versions v JOIN academic_terms t ON t.id=v.academic_term_id WHERE v.status='published' AND t.status='active' ORDER BY v.effective_from,v.id LIMIT 1").fetch_one(pool).await.unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,status,source_version_id,bell_schedule_id,delivery_version_id,created_by) VALUES($1,$2,$3,'draft',$4,$5,$6,$7)").bind(id).bind(term_id).bind(year_id).bind(source_id).bind(bell_id).bind(delivery_id).bind(Uuid::parse_str(ACTOR_ID).unwrap()).execute(pool).await.unwrap();
    (id, term_id, year_id, bell_id)
}

async fn publish_updated_opening_for_draft(pool: &sqlx::PgPool, version_id: Uuid) {
    use school_academic_delivery::{
        models::*,
        services::{change_sets, versions},
    };
    let table = school_academic_timetable::services::timetable_version_service::get_version(
        pool,
        version_id,
        chrono::Utc::now().date_naive(),
    )
    .await
    .unwrap();
    let source = versions::get_version(pool, table.delivery_version_id)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(table.academic_term_id)
        .execute(pool)
        .await
        .unwrap();
    let actor = Uuid::parse_str(ACTOR_ID).unwrap();
    let revision = change_sets::create_change_set(
        pool,
        actor,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: table.academic_term_id,
            reason: "เตรียมกลุ่มและครูสำหรับทดสอบคาบ".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let preview = change_sets::preview_change_set_at(
        pool,
        revision.id,
        Some(source.effective_from.unwrap() + chrono::Duration::days(1)),
    )
    .await
    .unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        pool,
        actor,
        revision.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: Vec::new(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let table = school_academic_timetable::services::timetable_version_service::get_version(
        pool,
        version_id,
        chrono::Utc::now().date_naive(),
    )
    .await
    .unwrap();
    school_academic_timetable::services::timetable_lifecycle::update_source(pool,actor,version_id,school_academic_timetable::models::timetable_version::UpdateTimetableDeliverySourceRequest {row_version:table.row_version,delivery_version_id:revision.target_delivery_version_id}).await.unwrap();
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
    let preferred_room_id: Uuid =
        sqlx::query_scalar("SELECT id FROM rooms WHERE status = 'ACTIVE' ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("fixture must include an active room");
    sqlx::query(
        r#"INSERT INTO learning_group_preferred_rooms (
               learning_group_id, academic_term_id, academic_year_id, room_id, rank
           ) VALUES ($1, $2, $3, $4, 1)"#,
    )
    .bind(group_id)
    .bind(term_id)
    .bind(offering_year_id)
    .bind(preferred_room_id)
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
    publish_updated_opening_for_draft(&pool, version_id).await;
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
    let workspace_group = workspace
        .learning_groups
        .iter()
        .find(|group| group.id == group_id)
        .expect("created group must be available in the timetable workspace");
    assert_eq!(workspace_group.preferred_room_ids, vec![preferred_room_id]);

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
            teacher_ids: None,
        },
    )
    .await
    .expect("one-period drag must move the canonical block with its exact instructors");
    assert_eq!(moved.day_of_week, "TUE");
    assert_eq!(moved.groups[0].instructors[0].teacher_id, teacher_id);
}

#[tokio::test]
async fn synchronized_published_groups_and_structural_per_target_removal_are_canonical() {
    let pool = migrated_pool("timetable_block_sync_structural").await;
    let actor_id = Uuid::parse_str(ACTOR_ID).unwrap();
    let (version_id, term_id, year_id, bell_schedule_id) = draft_version(&pool).await;
    use school_academic_delivery::{
        models::*,
        services::{change_sets, groups, versions},
    };
    let source_id: Uuid = sqlx::query_scalar(
        "SELECT delivery_version_id FROM academic_timetable_versions WHERE id=$1",
    )
    .bind(version_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let source = versions::get_version(&pool, source_id).await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(term_id)
        .execute(&pool)
        .await
        .unwrap();
    let revision = change_sets::create_change_set(
        &pool,
        actor_id,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: term_id,
            reason: "เปิดกิจกรรมก่อนจัดคาบ".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    // Prepare this historical fixture's teachers on the intended publication day.
    let revision = change_sets::update_change_set(
        &pool,
        actor_id,
        revision.id,
        UpdateAcademicTermChangeSetRequest {
            row_version: revision.row_version,
            reference_date: source.effective_from.unwrap() + chrono::Duration::days(1),
            reason: revision.reason.clone(),
        },
    )
    .await
    .unwrap();
    let activity_version_id:Uuid=sqlx::query_scalar("SELECT v.id FROM activity_versions v JOIN academic_terms t ON t.id=$1 WHERE v.scheduling_mode='synchronized' AND v.status='published' AND v.effective_from<=t.start_date AND (v.effective_until IS NULL OR t.start_date<v.effective_until) ORDER BY v.id LIMIT 1").bind(term_id).fetch_one(&pool).await.unwrap();
    let (homeroom_id,grade_level_id,study_program_id):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT id,grade_level_id,study_program_id FROM homerooms WHERE academic_year_id=$1 AND is_active ORDER BY id LIMIT 1").bind(year_id).fetch_one(&pool).await.unwrap();
    let changed = change_sets::upsert_change_item(
        &pool,
        actor_id,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AddActivity {
            change_set_row_version: revision.row_version,
            weekly_period_target: 1,
            offering: CreateActivityOfferingRequest {
                academic_term_id: term_id,
                activity_version_id,
                curriculum_activity_requirement_id: None,
                targets: vec![OfferingTargetInput {
                    target_kind: OfferingTargetKind::Homeroom,
                    homeroom_id: Some(homeroom_id),
                    grade_level_id,
                    study_program_id,
                }],
                registration_type: ActivityRegistrationType::Assigned,
                scheduling_mode: ActivitySchedulingMode::Synchronized,
                capacity: None,
                attendance_requirement: ActivityAttendanceRequirement {
                    minimum_percent: None,
                    required_sessions: None,
                },
                pass_criteria: ActivityPassCriteria {
                    require_attendance: false,
                    require_teacher_confirmation: true,
                    outcomes: vec!["pass".into(), "fail".into()],
                },
            },
        },
    )
    .await
    .unwrap();
    let added_id = changed
        .items
        .iter()
        .find_map(|item| match item {
            AcademicTermChangeItem::AddOffering {
                learning_offering_id,
                ..
            } => Some(*learning_offering_id),
            _ => None,
        })
        .unwrap();
    for group in groups::list(&pool, added_id).await.unwrap() {
        groups::replace_teachers(
            &pool,
            actor_id,
            group.id,
            ReplaceLearningGroupTeachersRequest {
                row_version: group.row_version,
                teachers: vec![TeacherAssignmentInput {
                    teacher_id: actor_id,
                    role: LearningTeacherRole::Primary,
                }],
            },
        )
        .await
        .unwrap();
    }
    let preview = change_sets::preview_change_set_at(
        &pool,
        revision.id,
        Some(source.effective_from.unwrap() + chrono::Duration::days(1)),
    )
    .await
    .unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        &pool,
        actor_id,
        revision.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: Vec::new(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let current = school_academic_timetable::services::timetable_version_service::get_version(
        &pool,
        version_id,
        chrono::Utc::now().date_naive(),
    )
    .await
    .unwrap();
    school_academic_timetable::services::timetable_lifecycle::update_source(&pool,actor_id,version_id,school_academic_timetable::models::timetable_version::UpdateTimetableDeliverySourceRequest {row_version:current.row_version,delivery_version_id:revision.target_delivery_version_id}).await.unwrap();
    let table = school_academic_timetable::services::timetable_version_service::get_version(
        &pool,
        version_id,
        chrono::Utc::now().date_naive(),
    )
    .await
    .unwrap();
    let opening =
        school_academic_delivery::services::versions::get_version(&pool, table.delivery_version_id)
            .await
            .unwrap();
    let offering=opening.snapshot.offerings.iter().find(|offering| matches!(&offering.catalog,school_academic_delivery::models::LearningOfferingSnapshot::Activity(activity) if activity.scheduling_mode==school_academic_delivery::models::ActivitySchedulingMode::Synchronized)).unwrap();
    let period_ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND is_active ORDER BY order_index,id LIMIT 2").bind(bell_schedule_id).fetch_all(&pool).await.unwrap();
    let homeroom_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 AND is_active ORDER BY id LIMIT 2",
    )
    .bind(year_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let reserved_teacher:Uuid=sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' AND status='active' AND NOT(id=ANY($1)) ORDER BY id LIMIT 1")
        .bind(offering.groups.iter().flat_map(|group|group.teachers.iter().map(|teacher|teacher.teacher_id)).collect::<Vec<_>>()).fetch_one(&pool).await.unwrap();
    let sync_block = timetable_block_service::create_synchronized_block(
        &pool,
        actor_id,
        CreateSynchronizedTimetableBlockRequest {
            timetable_version_id: version_id,
            academic_term_id: term_id,
            learning_offering_id: offering.id,
            day_of_week: "WED".into(),
            bell_schedule_period_id: period_ids[0],
            intended_homeroom_ids: offering.homeroom_ids.clone(),
            room_id: None,
            teacher_ids: vec![reserved_teacher],
            note: None,
        },
    )
    .await
    .unwrap();
    assert!(
        !sync_block.groups.is_empty(),
        "published opening groups synchronize immediately"
    );
    let group = sync_block.groups[0].clone();
    assert_eq!(
        sync_block
            .sync_states
            .iter()
            .find(|state| state.learning_group_id == group.learning_group_id)
            .unwrap()
            .status,
        TimetableBlockSyncStatus::Linked
    );
    let reservation = sync_block
        .teachers
        .iter()
        .find(|teacher| teacher.teacher_id == reserved_teacher)
        .unwrap();
    let without_reservation = timetable_block_service::remove_target(
        &pool,
        actor_id,
        sync_block.id,
        RemoveTimetableBlockTargetRequest {
            timetable_version_id: version_id,
            block_row_version: sync_block.row_version,
            target_kind: TimetableTargetKind::Teacher,
            target_id: reservation.id,
            target_row_version: reservation.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(without_reservation.groups.len(), sync_block.groups.len());
    let excluded = timetable_block_service::remove_target(
        &pool,
        actor_id,
        sync_block.id,
        RemoveTimetableBlockTargetRequest {
            timetable_version_id: version_id,
            block_row_version: without_reservation.row_version,
            target_kind: TimetableTargetKind::Group,
            target_id: group.id,
            target_row_version: group.row_version,
        },
    )
    .await
    .unwrap();
    assert!(!excluded
        .groups
        .iter()
        .any(|entry| entry.learning_group_id == group.learning_group_id));
    assert_eq!(
        excluded
            .sync_states
            .iter()
            .find(|state| state.learning_group_id == group.learning_group_id)
            .unwrap()
            .status,
        TimetableBlockSyncStatus::Excluded
    );
    let retried = timetable_block_service::retry_sync(
        &pool,
        actor_id,
        sync_block.id,
        RetryTimetableBlockSyncRequest {
            timetable_version_id: version_id,
            block_row_version: excluded.row_version,
            learning_group_ids: vec![group.learning_group_id],
        },
    )
    .await
    .unwrap();
    assert!(
        !retried
            .groups
            .iter()
            .any(|entry| entry.learning_group_id == group.learning_group_id),
        "retry does not undo a deliberate exclusion"
    );
    let restored = timetable_block_service::restore_group(
        &pool,
        actor_id,
        sync_block.id,
        RestoreTimetableBlockGroupRequest {
            timetable_version_id: version_id,
            block_row_version: retried.row_version,
            learning_group_id: group.learning_group_id,
        },
    )
    .await
    .unwrap();
    assert!(restored
        .groups
        .iter()
        .any(|entry| entry.learning_group_id == group.learning_group_id));
    let structural = timetable_block_service::create_structural_blocks(
        &pool,
        actor_id,
        CreateStructuralTimetableBlocksRequest {
            timetable_version_id: version_id,
            academic_term_id: term_id,
            structural_kind: TimetableStructuralKind::FlagCeremony,
            title: "กิจกรรมหน้าเสาธง".into(),
            note: None,
            slots: vec![TimetableStructuralSlotInput {
                day_of_week: "TUE".into(),
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
    .unwrap();
    let removed = &structural[0].homerooms[0];
    let changed = timetable_block_service::remove_target(
        &pool,
        actor_id,
        structural[0].id,
        RemoveTimetableBlockTargetRequest {
            timetable_version_id: version_id,
            block_row_version: structural[0].row_version,
            target_kind: TimetableTargetKind::Homeroom,
            target_id: removed.id,
            target_row_version: removed.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(changed.homerooms.len(), homeroom_ids.len() - 1);
    assert!(!changed
        .homerooms
        .iter()
        .any(|target| target.id == removed.id));
    let date = opening.effective_from.unwrap() + chrono::Duration::days(1);
    sqlx::query("UPDATE academic_timetable_versions SET status='published',effective_from=$3,published_by=$2,published_at=now(),publication_idempotency_key=gen_random_uuid(),publication_request_hash=repeat('a',64) WHERE id=$1").bind(version_id).bind(actor_id).bind(date).execute(&pool).await.unwrap();
    let mut timetable_date = date;
    while daily_teaching_service::day_code_from_date(timetable_date) != "WED" {
        timetable_date = timetable_date.checked_add_days(Days::new(1)).unwrap();
    }
    let daily = daily_teaching_service::get_daily_teaching_overview(
        &pool,
        daily_teaching_service::DailyTeachingQuery {
            academic_term_id: term_id,
            date: Some(timetable_date),
            include_empty_teachers: Some(false),
        },
    )
    .await
    .unwrap();
    let teacher_id = group.instructors[0].teacher_id;
    assert!(daily
        .teachers
        .iter()
        .find(|teacher| teacher.id == teacher_id)
        .unwrap()
        .periods
        .iter()
        .flat_map(|period| &period.entries)
        .any(|entry| entry.learning_group_id == Some(group.learning_group_id)));
}
