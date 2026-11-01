use chrono::{Days, NaiveDate};
use uuid::Uuid;

use super::timetable_block_service;
use crate::integration_tests::academic::core::services_tests::prepare_current_core_fixture;
use school_academic_timetable::models::timetable_block::{
    CreateOrdinaryTimetableBlockRequest, CreateStructuralTimetableBlocksRequest,
    CreateSynchronizedTimetableBlockRequest, RemoveTimetableBlockTargetRequest,
    RestoreTimetableBlockGroupRequest, RetryTimetableBlockSyncRequest, TimetableBlockSyncStatus,
    TimetableBlockWorkspaceQuery, TimetableStructuralKind, TimetableStructuralSlotInput,
    TimetableTargetKind, UpdateTimetableBlockRequest,
};
use school_academic_timetable::policy::TimetableAccessFilter;
use school_academic_timetable::ports::{StaffDirectoryPort, StaffSubjectGroupAssignment};
use school_academic_timetable::services::daily_teaching as daily_teaching_service;
use school_errors::AppError;

const ACTOR_ID: &str = "50000000-0000-0000-0000-000000000002";

struct WorkspaceStaffDirectory {
    teacher_id: Uuid,
    subject_group_id: Uuid,
    fail: bool,
}

#[async_trait::async_trait]
impl StaffDirectoryPort for WorkspaceStaffDirectory {
    async fn collaboration_portraits(
        &self,
        _connection: &mut sqlx::PgConnection,
        _viewer: &school_authorization::ActorContext,
        _user_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, Uuid)>, AppError> {
        Ok(Vec::new())
    }

    async fn subject_groups_for_users(
        &self,
        _connection: &mut sqlx::PgConnection,
        user_ids: &[Uuid],
    ) -> Result<Vec<StaffSubjectGroupAssignment>, AppError> {
        assert!(user_ids.contains(&self.teacher_id));
        assert!(user_ids.windows(2).all(|pair| pair[0] < pair[1]));
        if self.fail {
            return Err(AppError::InternalServerError(
                "directory unavailable".into(),
            ));
        }
        Ok(vec![StaffSubjectGroupAssignment {
            user_id: self.teacher_id,
            subject_group_id: self.subject_group_id,
            name: "กลุ่มสาระทดสอบ".into(),
            display_order: Some(3),
        }])
    }
}

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
    let blocks = timetable_block_service::create_structural_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor,
        ),
        request.clone(),
    )
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
        sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
            .bind(term)
            .bind(term_status)
            .execute(&pool)
            .await
            .unwrap();
        let mut create_request = request.clone();
        create_request.slots = vec![TimetableStructuralSlotInput {
            day_of_week: "WED".into(),
            bell_schedule_period_id: period,
        }];
        let outcomes = [
            timetable_block_service::create_structural_blocks(
                &pool,
                &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                    &pool, actor,
                ),
                create_request,
            )
            .await
            .map(|_| ()),
            timetable_block_service::remove_target(
                &pool,
                &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                    &pool, actor,
                ),
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
                &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                    &pool, actor,
                ),
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
                &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                    &pool, actor,
                ),
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
                &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                    &pool, actor,
                ),
                block.id,
                version,
                block.row_version,
            )
            .await
            .map(|_| ()),
            timetable_block_service::deactivate_series(
                &pool,
                &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                    &pool, actor,
                ),
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
    let (block_id, row_version) = (block.id, block.row_version);
    let worker = tokio::spawn(async move {
        timetable_block_service::deactivate_block(
            &worker_pool,
            &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                &worker_pool,
                actor,
            ),
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

pub(super) async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = prepare_current_core_fixture(test_name).await;
    school_test_db::run_test_migrations(&pool).await;
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

pub(super) async fn draft_version(pool: &sqlx::PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let (source_id,term_id,year_id,bell_id,delivery_id):(Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT v.id,v.academic_term_id,v.academic_year_id,v.bell_schedule_id,v.delivery_version_id FROM academic_timetable_versions v JOIN academic_terms t ON t.id=v.academic_term_id WHERE v.status='published' AND t.status='active' ORDER BY v.effective_from,v.id LIMIT 1").fetch_one(pool).await.unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,status,source_version_id,bell_schedule_id,delivery_version_id,created_by) VALUES($1,$2,$3,'draft',$4,$5,$6,$7)").bind(id).bind(term_id).bind(year_id).bind(source_id).bind(bell_id).bind(delivery_id).bind(Uuid::parse_str(ACTOR_ID).unwrap()).execute(pool).await.unwrap();
    (id, term_id, year_id, bell_id)
}

pub(super) async fn publish_updated_opening_for_draft(pool: &sqlx::PgPool, version_id: Uuid) {
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
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
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
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
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
    school_academic_timetable::services::timetable_lifecycle::update_source(pool,&school_academic_timetable::collaboration::TimetableMutationContext::for_tests(pool, actor),version_id,school_academic_timetable::models::timetable_version::UpdateTimetableDeliverySourceRequest {row_version:table.row_version,delivery_version_id:revision.target_delivery_version_id}).await.unwrap();
}

#[tokio::test]
async fn timetable_workspace_classification_uses_pinned_subject_revision() {
    use school_academic_core::{
        models::{CreateSubjectVersionRequest, PublishVersionRequest},
        services::catalog,
    };
    use school_academic_delivery::{models::LearningOfferingSnapshot, services::versions};

    let pool = migrated_pool("timetable_workspace_pinned_classification").await;
    let (version_id, term_id, year_id, _) = draft_version(&pool).await;
    let query = || TimetableBlockWorkspaceQuery {
        academic_year_id: year_id,
        academic_term_id: term_id,
        timetable_version_id: version_id,
    };
    let access = TimetableAccessFilter {
        includes_school_owned: true,
        ..TimetableAccessFilter::default()
    };
    let directory = &crate::adapters::staff_directory::STAFF_DIRECTORY;
    let before = timetable_block_service::get_workspace(&pool, query(), &access, directory)
        .await
        .unwrap();
    let source_id = before.version.delivery_version_id;
    let source = versions::get_version(&pool, source_id).await.unwrap();
    let pinned_offering = source
        .snapshot
        .offerings
        .iter()
        .find(|offering| {
            matches!(&offering.catalog, LearningOfferingSnapshot::Course(_))
                && !offering.groups.is_empty()
        })
        .unwrap();
    let LearningOfferingSnapshot::Course(course) = &pinned_offering.catalog else {
        unreachable!()
    };
    let pinned = catalog::get_subject_version(&pool, course.subject_version_id)
        .await
        .unwrap();
    assert_eq!(pinned.subject_type, "BASIC");
    let pinned_before = serde_json::to_value(&pinned).unwrap();
    let source_before = serde_json::to_value(&source.snapshot).unwrap();
    let revised = catalog::create_subject_version(
        &pool,
        pinned.subject_id,
        CreateSubjectVersionRequest {
            name_th: pinned.name_th.clone(),
            name_en: pinned.name_en.clone(),
            credit: pinned.credit.clone(),
            hours_per_semester: pinned.hours_per_semester,
            subject_type: "ADDITIONAL".into(),
            description: pinned.description.clone(),
            effective_from: catalog::subject_catalog_today(),
            term_code: pinned.term_code.clone(),
            periods_per_week: pinned.periods_per_week,
            grade_level_ids: pinned.grade_level_ids.clone(),
        },
    )
    .await
    .unwrap();
    let revised = catalog::publish_subject_version(
        &pool,
        revised.id,
        PublishVersionRequest {
            row_version: revised.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        catalog::current_subject_version_ids(&pool, &[pinned.subject_id])
            .await
            .unwrap()[&pinned.subject_id],
        revised.id
    );

    let mut connection = pool.acquire().await.unwrap();
    let metadata = catalog::subject_groups_for_versions_on_connection(
        &mut connection,
        &[pinned.id, revised.id],
    )
    .await
    .unwrap();
    assert_eq!(
        metadata
            .iter()
            .find(|row| row.subject_version_id == pinned.id)
            .unwrap()
            .subject_type,
        "BASIC"
    );
    assert_eq!(
        metadata
            .iter()
            .find(|row| row.subject_version_id == revised.id)
            .unwrap()
            .subject_type,
        "ADDITIONAL"
    );
    drop(connection);

    let after = timetable_block_service::get_workspace(&pool, query(), &access, directory)
        .await
        .unwrap();
    let groups = after
        .learning_groups
        .iter()
        .filter(|group| group.learning_offering_id == pinned_offering.id)
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), pinned_offering.groups.len());
    assert!(groups
        .iter()
        .all(|group| group.subject_type.as_deref() == Some("BASIC")));
    assert_eq!(
        serde_json::to_value(&before.learning_groups).unwrap(),
        serde_json::to_value(&after.learning_groups).unwrap()
    );
    assert_eq!(
        pinned_before,
        serde_json::to_value(
            catalog::get_subject_version(&pool, pinned.id)
                .await
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        source_before,
        serde_json::to_value(
            versions::get_version(&pool, source_id)
                .await
                .unwrap()
                .snapshot
        )
        .unwrap()
    );

    let denied = timetable_block_service::get_workspace(
        &pool,
        query(),
        &TimetableAccessFilter::default(),
        directory,
    )
    .await
    .unwrap();
    assert!(denied.learning_groups.is_empty());
    assert!(denied.offering_subject_groups.is_empty());
    assert!(denied.ordinary_demands.is_empty());
    assert!(denied.synchronized_demands.is_empty());
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
               title_en, first_name_en, last_name_en, user_type, status
           ) VALUES ($1, $2, $3, 'fixture-not-a-login', '', '',
                     'Ms.', 'Alice', 'Example', 'staff', 'active')"#,
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
    sqlx::query("UPDATE rooms SET name_th='ห้องคณิตศาสตร์ทดสอบ',code='INTERNAL-MATH-001' WHERE id=$1")
        .bind(preferred_room_id)
        .execute(&pool)
        .await
        .unwrap();
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
        room_id: Some(preferred_room_id),
        instructor_ids: vec![teacher_id],
        note: None,
    };

    let block = timetable_block_service::create_ordinary_block(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
        request.clone(),
    )
    .await
    .expect("ordinary placement must succeed");
    assert_eq!(block.groups.len(), 1);
    assert!(block.groups[0].homeroom_ids.is_empty());
    assert!(block.groups[0].homeroom_names.is_empty());
    assert_eq!(block.groups[0].room_id, Some(preferred_room_id));
    assert_eq!(
        block.groups[0].room_name.as_deref(),
        Some("ห้องคณิตศาสตร์ทดสอบ")
    );
    assert_eq!(
        block.groups[0].room_code.as_deref(),
        Some("INTERNAL-MATH-001")
    );
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
        &crate::adapters::staff_directory::STAFF_DIRECTORY,
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
    assert_eq!(
        workspace_group.eligible_instructors[0].display_name, "Ms. Alice Example",
        "eligible instructor labels must use the canonical complete English name"
    );
    let immutable_versions = || async {
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT jsonb_build_object('delivery',to_jsonb(delivery),'timetable',to_jsonb(timetable)) FROM academic_timetable_versions timetable JOIN academic_delivery_versions delivery ON delivery.id=timetable.delivery_version_id WHERE timetable.id=$1",
        ).bind(version_id).fetch_one(&pool).await.unwrap()
    };
    let before_names = immutable_versions().await;
    let pinned_teacher = before_names["delivery"]["snapshot"]["offerings"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|offering| offering["groups"].as_array().unwrap())
        .flat_map(|group| group["teachers"].as_array().unwrap())
        .find(|teacher| teacher["teacherId"] == teacher_id.to_string())
        .unwrap();
    assert!(
        pinned_teacher["displayName"]
            .as_str()
            .unwrap()
            .trim()
            .is_empty(),
        "English-only fixture must reproduce the historical blank snapshot label"
    );
    let username = format!("timetable-{teacher_id}");
    for (thai, english, expected) in [
        (
            ["", "", ""],
            ["Ms.", "Alice", "Example"],
            "Ms. Alice Example",
        ),
        (
            ["นางสาว", "มาลี", "ตัวอย่าง"],
            ["Ms.", "Alice", "Example"],
            "นางสาวมาลี ตัวอย่าง",
        ),
        (
            ["", "บางส่วน", ""],
            ["Ms.", "Alice", "Example"],
            "Ms. Alice Example",
        ),
        (["", "บางส่วน", ""], ["", "Alice", "Example"], "บางส่วน"),
        (["", "", ""], ["", "", ""], username.as_str()),
    ] {
        sqlx::query("UPDATE users SET title=$2,first_name=$3,last_name=$4,title_en=$5,first_name_en=$6,last_name_en=$7 WHERE id=$1")
            .bind(teacher_id).bind(thai[0]).bind(thai[1]).bind(thai[2])
            .bind(english[0]).bind(english[1]).bind(english[2])
            .execute(&pool).await.unwrap();
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
            &crate::adapters::staff_directory::STAFF_DIRECTORY,
        )
        .await
        .unwrap();
        let eligible = &workspace
            .learning_groups
            .iter()
            .find(|group| group.id == group_id)
            .unwrap()
            .eligible_instructors;
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].teacher_id, teacher_id);
        assert_eq!(eligible[0].role, "primary");
        assert_eq!(eligible[0].order_index, 1);
        assert_eq!(eligible[0].display_name, expected);
        assert_eq!(
            workspace
                .ordinary_demands
                .iter()
                .find(|demand| demand.learning_group_id == group_id)
                .unwrap()
                .eligible_instructors[0]
                .display_name,
            expected
        );
        assert_eq!(
            workspace
                .staff
                .iter()
                .find(|staff| staff.id == teacher_id)
                .unwrap()
                .display_name,
            expected
        );
        assert_eq!(
            workspace
                .blocks
                .iter()
                .find(|candidate| candidate.id == block.id)
                .unwrap()
                .groups[0]
                .instructors[0]
                .display_name,
            expected,
        );
        let placed = timetable_block_service::get_block(&pool, block.id)
            .await
            .unwrap();
        assert_eq!(placed.groups[0].instructors[0].display_name, expected);
        assert_eq!(placed.groups[0].instructors[0].teacher_id, teacher_id);
        assert_eq!(placed.groups[0].instructors[0].role, "primary");
        assert_eq!(placed.groups[0].instructors[0].order_index, 1);
        assert_eq!(immutable_versions().await, before_names);
    }
    let denied = timetable_block_service::get_workspace(
        &pool,
        TimetableBlockWorkspaceQuery {
            academic_year_id: year_id,
            academic_term_id: term_id,
            timetable_version_id: version_id,
        },
        &TimetableAccessFilter::default(),
        &crate::adapters::staff_directory::STAFF_DIRECTORY,
    )
    .await
    .unwrap();
    assert!(denied.learning_groups.is_empty());
    assert!(denied.ordinary_demands.is_empty());
    assert!(denied.staff.is_empty());
    assert!(denied.blocks.is_empty());

    let mut directory = WorkspaceStaffDirectory {
        teacher_id,
        subject_group_id: Uuid::new_v4(),
        fail: false,
    };
    let query = || TimetableBlockWorkspaceQuery {
        academic_year_id: year_id,
        academic_term_id: term_id,
        timetable_version_id: version_id,
    };
    let access = TimetableAccessFilter {
        includes_school_owned: true,
        ..TimetableAccessFilter::default()
    };
    let workspace = timetable_block_service::get_workspace(&pool, query(), &access, &directory)
        .await
        .expect("workspace must use the selected directory provider");
    let teacher = workspace
        .staff
        .iter()
        .find(|staff| staff.id == teacher_id)
        .expect("workspace must retain the selected teacher");
    assert_eq!(teacher.subject_groups.len(), 1);
    assert_eq!(teacher.subject_groups[0].id, directory.subject_group_id);
    assert_eq!(teacher.subject_groups[0].name, "กลุ่มสาระทดสอบ");
    assert_eq!(teacher.subject_groups[0].display_order, Some(3));

    directory.fail = true;
    assert!(
        matches!(
            timetable_block_service::get_workspace(&pool, query(), &access, &directory).await,
            Err(AppError::InternalServerError(message)) if message == "directory unavailable"
        ),
        "a failed authoritative read must not turn into empty affiliations"
    );

    assert!(matches!(
        timetable_block_service::create_ordinary_block(
            &pool,
            &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                &pool, actor_id
            ),
            request
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let moved = timetable_block_service::update_block(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
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

    let mut homeroom_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 AND is_active ORDER BY id LIMIT 2",
    )
    .bind(year_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(homeroom_ids.len(), 2);
    homeroom_ids.reverse();
    for (id, name) in homeroom_ids.iter().zip(["ม.4/1", "ม.3/2"]) {
        sqlx::query("UPDATE homerooms SET name=$2 WHERE id=$1")
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    // Stored coverage, rather than the current delivery group or its display name,
    // remains authoritative for a placed block, including an inactive homeroom.
    sqlx::query("UPDATE homerooms SET is_active=false WHERE id=$1")
        .bind(homeroom_ids[1])
        .execute(&pool)
        .await
        .unwrap();
    let stored_ids = vec![homeroom_ids[0], homeroom_ids[1], homeroom_ids[0]];
    sqlx::query("UPDATE academic_timetable_block_groups SET homeroom_ids=$2 WHERE id=$1")
        .bind(moved.groups[0].id)
        .bind(&stored_ids)
        .execute(&pool)
        .await
        .unwrap();
    let own_blocks =
        timetable_block_service::list_instructor_blocks(&pool, version_id, term_id, teacher_id)
            .await
            .unwrap();
    assert_eq!(own_blocks.len(), 1);
    assert_eq!(own_blocks[0].id, moved.id);
    assert_eq!(own_blocks[0].groups.len(), 1);
    assert_eq!(own_blocks[0].groups[0].name, "กลุ่มทดสอบตารางสอน");
    assert_eq!(own_blocks[0].groups[0].homeroom_ids, stored_ids);
    assert_eq!(
        own_blocks[0].groups[0].homeroom_names,
        vec!["ม.4/1", "ม.3/2"]
    );
    assert!(
        timetable_block_service::list_instructor_blocks(&pool, version_id, term_id, actor_id)
            .await
            .unwrap()
            .is_empty()
    );

    // A return leg must retain the block group's exact persisted coverage and
    // instructors even when the client sends an empty structural candidate.
    use school_academic_timetable::models::timetable_block::{
        TimetableBlockKind, TimetableBlockPlacementCandidate,
        TimetableBlockPlacementPreviewRequest, TimetableBlockPlacementSource,
        TimetableBlockPlacementState,
    };
    let structural = |teacher, homeroom_ids, room_id| CreateStructuralTimetableBlocksRequest {
        timetable_version_id: version_id,
        academic_term_id: term_id,
        structural_kind: TimetableStructuralKind::Other,
        title: "ทดสอบคาบกลุ่มที่สลับกลับ".into(),
        note: None,
        slots: vec![TimetableStructuralSlotInput {
            day_of_week: "MON".into(),
            bell_schedule_period_id: period_id,
        }],
        homeroom_ids,
        teacher_ids: vec![teacher],
        all_homerooms: false,
        all_teachers: false,
        room_id,
    };
    let swap_source = timetable_block_service::create_structural_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
        structural(actor_id, vec![], None),
    )
    .await
    .unwrap()
    .remove(0);
    let third = timetable_block_service::create_structural_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
        structural(teacher_id, vec![homeroom_ids[0]], Some(preferred_room_id)),
    )
    .await
    .unwrap()
    .remove(0);
    let preview_request = TimetableBlockPlacementPreviewRequest {
        timetable_version_id: version_id,
        academic_term_id: term_id,
        source: TimetableBlockPlacementSource::ExistingBlock {
            block_id: swap_source.id,
            row_version: swap_source.row_version,
        },
        candidate: TimetableBlockPlacementCandidate {
            block_kind: TimetableBlockKind::Structural,
            learning_group_id: None,
            learning_offering_id: None,
            room_id: None,
            homeroom_ids: vec![],
            teacher_ids: vec![],
            instructor_ids: vec![],
        },
        target_day_of_week: "TUE".into(),
        target_bell_schedule_period_id: period_id,
        expected_target_block_id: Some(moved.id),
        expected_target_row_version: Some(moved.row_version),
    };
    let preview = timetable_block_service::preview_placement(&pool, preview_request.clone())
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Blocked);
    assert_eq!(preview.conflicts.len(), 3);
    for (code, id) in [
        ("TIMETABLE_SWAP_RETURN_TEACHER_CONFLICT", teacher_id),
        ("TIMETABLE_SWAP_RETURN_ROOM_CONFLICT", preferred_room_id),
        ("TIMETABLE_SWAP_RETURN_HOMEROOM_CONFLICT", homeroom_ids[0]),
    ] {
        assert!(preview
            .conflicts
            .iter()
            .any(|conflict| conflict.code == code
                && conflict.target_id == Some(id)
                && conflict.existing_block_id == Some(third.id)));
    }
    let mut create_request = preview_request.clone();
    create_request.source = TimetableBlockPlacementSource::OrdinaryDemand {
        learning_group_id: group_id,
        learning_offering_id: offering_id,
    };
    create_request.candidate = TimetableBlockPlacementCandidate {
        block_kind: TimetableBlockKind::Course,
        learning_group_id: Some(group_id),
        learning_offering_id: Some(offering_id),
        room_id: Some(preferred_room_id),
        homeroom_ids: vec![],
        teacher_ids: vec![],
        instructor_ids: vec![teacher_id],
    };
    create_request.target_day_of_week = "WED".into();
    create_request.expected_target_block_id = None;
    create_request.expected_target_row_version = None;
    let preview = timetable_block_service::preview_placement(&pool, create_request.clone())
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Source);
    assert!(preview.conflicts.is_empty());
    create_request.target_day_of_week = "TUE".into();
    let preview = timetable_block_service::preview_placement(&pool, create_request)
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Blocked);
    assert!(preview
        .conflicts
        .iter()
        .any(|row| row.code == "TIMETABLE_GROUP_CONFLICT"));

    let mut move_request = preview_request;
    move_request.source = TimetableBlockPlacementSource::ExistingBlock {
        block_id: moved.id,
        row_version: moved.row_version,
    };
    move_request.target_day_of_week = "MON".into();
    move_request.expected_target_block_id = None;
    move_request.expected_target_row_version = None;
    let preview = timetable_block_service::preview_placement(&pool, move_request)
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Blocked);
    assert_eq!(preview.conflicts.len(), 3);
    assert!(preview
        .conflicts
        .iter()
        .any(|row| row.code == "TIMETABLE_HOMEROOM_CONFLICT"));
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
    let activity_version_id:Uuid=sqlx::query_scalar("SELECT v.id FROM activity_versions v JOIN academic_terms t ON t.id=$1 WHERE v.scheduling_mode='synchronized' AND v.status='published' AND v.effective_from<=(now() AT TIME ZONE 'Asia/Bangkok')::date ORDER BY v.id LIMIT 1").bind(term_id).fetch_one(&pool).await.unwrap();
    let (homeroom_id,grade_level_id,study_program_id):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT id,grade_level_id,study_program_id FROM homerooms WHERE academic_year_id=$1 AND is_active ORDER BY id LIMIT 1").bind(year_id).fetch_one(&pool).await.unwrap();
    let changed = change_sets::upsert_change_item(
        &pool,
        actor_id,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AddActivity {
            change_set_row_version: revision.row_version,
            weekly_period_target: 1,
            offering: CreateActivityOfferingRequest {
                starts_on: chrono::Utc::now()
                    .with_timezone(&chrono_tz::Asia::Bangkok)
                    .date_naive(),
                create_homeroom_groups: false,
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
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
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
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
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
    school_academic_timetable::services::timetable_lifecycle::update_source(&pool,&school_academic_timetable::collaboration::TimetableMutationContext::for_tests(&pool, actor_id),version_id,school_academic_timetable::models::timetable_version::UpdateTimetableDeliverySourceRequest {row_version:current.row_version,delivery_version_id:revision.target_delivery_version_id}).await.unwrap();
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
        &crate::adapters::staff_directory::STAFF_DIRECTORY,
    )
    .await
    .unwrap();
    let activities = workspace
        .learning_groups
        .iter()
        .filter(|group| group.offering_kind == "activity")
        .collect::<Vec<_>>();
    assert!(
        !activities.is_empty(),
        "fixture must exercise activity classification"
    );
    assert!(activities.iter().all(|group| group.subject_type.is_none()));
    let json = serde_json::to_value(&workspace).unwrap();
    assert!(json["learningGroups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|group| group["offeringKind"] == "activity")
        .all(|group| group.get("subjectType") == Some(&serde_json::Value::Null)));
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
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
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
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
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
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
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
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
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
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
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
    let physical_room_id: Uuid =
        sqlx::query_scalar("SELECT id FROM rooms WHERE status='ACTIVE' ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE rooms SET name_th='ห้องวิทยาศาสตร์ทดสอบ',code='INTERNAL-SCI-002' WHERE id=$1")
        .bind(physical_room_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_timetable_block_groups SET room_id=$2 WHERE id=$1")
        .bind(group.id)
        .bind(physical_room_id)
        .execute(&pool)
        .await
        .unwrap();
    let structural = timetable_block_service::create_structural_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
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
            room_id: Some(physical_room_id),
        },
    )
    .await
    .unwrap();
    let removed = &structural[0].homerooms[0];
    assert_eq!(removed.room_id, Some(physical_room_id));
    assert_eq!(removed.room_name.as_deref(), Some("ห้องวิทยาศาสตร์ทดสอบ"));
    assert_eq!(removed.room_code.as_deref(), Some("INTERNAL-SCI-002"));
    let changed = timetable_block_service::remove_target(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor_id,
        ),
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
    sqlx::query("UPDATE academic_timetable_versions SET publication_no=(SELECT COALESCE(max(published.publication_no),0)+1 FROM academic_timetable_versions published WHERE published.academic_term_id=academic_timetable_versions.academic_term_id AND published.status='published'),status='published',effective_from=$3,published_by=$2,published_at=now(),publication_idempotency_key=gen_random_uuid(),publication_request_hash=repeat('a',64) WHERE id=$1").bind(version_id).bind(actor_id).bind(date).execute(&pool).await.unwrap();
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
    let projected_entry = daily
        .teachers
        .iter()
        .flat_map(|teacher| &teacher.periods)
        .flat_map(|period| &period.entries)
        .find(|entry| entry.learning_group_id == Some(group.learning_group_id))
        .expect("assigned group must be projected into daily teaching");
    assert_eq!(projected_entry.room_id, Some(physical_room_id));
    assert_eq!(
        projected_entry.room_name.as_deref(),
        Some("ห้องวิทยาศาสตร์ทดสอบ")
    );
    assert_eq!(
        projected_entry.room_code.as_deref(),
        Some("INTERNAL-SCI-002")
    );
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

#[tokio::test]
async fn timetable_placement_preview_checks_both_swap_legs_and_preserves_blocks() {
    use school_academic_timetable::models::timetable_block::{
        SwapTimetableBlocksRequest, TimetableBlockKind, TimetableBlockMutationKind,
        TimetableBlockPlacementCandidate, TimetableBlockPlacementPreviewRequest,
        TimetableBlockPlacementSource, TimetableBlockPlacementState,
    };

    let pool = migrated_pool("timetable_placement_preview_swap").await;
    let actor = Uuid::parse_str(ACTOR_ID).unwrap();
    let (version, term, year, bell) = draft_version(&pool).await;
    let period: Uuid = sqlx::query_scalar("SELECT id FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND is_active ORDER BY order_index,id LIMIT 1")
        .bind(bell).fetch_one(&pool).await.unwrap();
    let teachers: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 2",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let homerooms: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 AND is_active ORDER BY id LIMIT 2",
    )
    .bind(year)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(teachers.len(), 2);
    assert_eq!(homerooms.len(), 2);
    let room: Uuid =
        sqlx::query_scalar("SELECT id FROM rooms WHERE status='ACTIVE' ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let structural = |day: &str, teacher, homeroom_ids: Vec<Uuid>, room_id| {
        CreateStructuralTimetableBlocksRequest {
            timetable_version_id: version,
            academic_term_id: term,
            structural_kind: TimetableStructuralKind::Other,
            title: "ทดสอบสลับคาบ".into(),
            note: None,
            slots: vec![TimetableStructuralSlotInput {
                day_of_week: day.into(),
                bell_schedule_period_id: period,
            }],
            homeroom_ids,
            teacher_ids: vec![teacher],
            all_homerooms: false,
            all_teachers: false,
            room_id,
        }
    };
    let source = timetable_block_service::create_structural_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor,
        ),
        structural("MON", teachers[0], vec![], None),
    )
    .await
    .unwrap()
    .remove(0);
    let target = timetable_block_service::create_structural_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor,
        ),
        structural("TUE", teachers[1], homerooms.clone(), Some(room)),
    )
    .await
    .unwrap()
    .remove(0);
    let third = timetable_block_service::create_structural_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor,
        ),
        structural("MON", teachers[1], homerooms, Some(room)),
    )
    .await
    .unwrap()
    .remove(0);
    // Deliberately omit every candidate reservation: existing block moves/swaps must
    // check the persisted targets, including both homeroom rows using the same room.
    let request = TimetableBlockPlacementPreviewRequest {
        timetable_version_id: version,
        academic_term_id: term,
        source: TimetableBlockPlacementSource::ExistingBlock {
            block_id: source.id,
            row_version: source.row_version,
        },
        candidate: TimetableBlockPlacementCandidate {
            block_kind: TimetableBlockKind::Structural,
            learning_group_id: None,
            learning_offering_id: None,
            room_id: None,
            homeroom_ids: vec![],
            teacher_ids: vec![],
            instructor_ids: vec![],
        },
        target_day_of_week: " tue ".into(),
        target_bell_schedule_period_id: period,
        expected_target_block_id: Some(target.id),
        expected_target_row_version: Some(target.row_version),
    };
    let before = serde_json::to_value([&source, &target, &third]).unwrap();
    let preview = timetable_block_service::preview_placement(&pool, request.clone())
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Blocked);
    assert_eq!(preview.mutation, None);
    assert_eq!(preview.target_day_of_week, "TUE");
    assert_eq!(
        preview.conflicts.len(),
        4,
        "two homerooms, one teacher, one deduplicated room"
    );
    for (code, target_id) in [
        ("TIMETABLE_SWAP_RETURN_TEACHER_CONFLICT", teachers[1]),
        ("TIMETABLE_SWAP_RETURN_ROOM_CONFLICT", room),
    ] {
        let conflicts = preview
            .conflicts
            .iter()
            .filter(|row| row.code == code)
            .collect::<Vec<_>>();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].existing_block_id, Some(third.id));
        assert_eq!(conflicts[0].target_id, Some(target_id));
    }
    assert!(preview
        .conflicts
        .iter()
        .all(|row| row.message.starts_with("คาบที่สลับกลับ: ")));
    let swap = SwapTimetableBlocksRequest {
        timetable_version_id: version,
        block_a_id: source.id,
        block_a_row_version: source.row_version,
        block_b_id: target.id,
        block_b_row_version: target.row_version,
    };
    assert!(matches!(
        timetable_block_service::swap_blocks(
            &pool,
            &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                &pool, actor
            ),
            swap.clone()
        )
        .await,
        Err(AppError::Conflict(_))
    ));

    let mut stale_requests = Vec::new();
    let (other_version, _, _, _) = draft_version(&pool).await;
    let mut stale = request.clone();
    stale.timetable_version_id = other_version;
    stale_requests.push(stale);
    let mut stale = request.clone();
    stale.source = TimetableBlockPlacementSource::ExistingBlock {
        block_id: Uuid::new_v4(),
        row_version: source.row_version,
    };
    stale_requests.push(stale);
    let mut stale = request.clone();
    stale.expected_target_block_id = Some(Uuid::new_v4());
    stale_requests.push(stale);
    let mut stale = request.clone();
    stale.source = TimetableBlockPlacementSource::ExistingBlock {
        block_id: source.id,
        row_version: source.row_version + 1,
    };
    stale_requests.push(stale);
    let mut stale = request.clone();
    stale.expected_target_row_version = Some(target.row_version + 1);
    stale_requests.push(stale);
    let mut stale = request.clone();
    stale.expected_target_row_version = None;
    stale_requests.push(stale);
    let mut stale = request.clone();
    stale.target_day_of_week = "WED".into();
    stale_requests.push(stale);
    let mut stale = request.clone();
    stale.expected_target_block_id = Some(source.id);
    stale.expected_target_row_version = Some(source.row_version);
    stale.target_day_of_week = "MON".into();
    stale_requests.push(stale);
    let mut stale = request.clone();
    stale.source = TimetableBlockPlacementSource::SynchronizedOffering {
        learning_offering_id: Uuid::new_v4(),
    };
    stale_requests.push(stale);
    for stale in stale_requests {
        let preview = timetable_block_service::preview_placement(&pool, stale)
            .await
            .unwrap();
        assert_eq!(preview.state, TimetableBlockPlacementState::Blocked);
        assert_eq!(preview.mutation, None);
        assert_eq!(preview.conflicts.len(), 1);
        assert_eq!(preview.conflicts[0].code, "TIMETABLE_STALE_BLOCK");
        assert_eq!(preview.conflicts[0].existing_block_id, None);
    }
    let after = [
        timetable_block_service::get_block(&pool, source.id)
            .await
            .unwrap(),
        timetable_block_service::get_block(&pool, target.id)
            .await
            .unwrap(),
        timetable_block_service::get_block(&pool, third.id)
            .await
            .unwrap(),
    ];
    assert_eq!(before, serde_json::to_value(after).unwrap());

    // Moving the target without a swap also uses persisted reservations.
    let mut move_request = request.clone();
    move_request.source = TimetableBlockPlacementSource::ExistingBlock {
        block_id: target.id,
        row_version: target.row_version,
    };
    move_request.target_day_of_week = "MON".into();
    move_request.expected_target_block_id = None;
    move_request.expected_target_row_version = None;
    let preview = timetable_block_service::preview_placement(&pool, move_request.clone())
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Blocked);
    assert!(preview
        .conflicts
        .iter()
        .any(|row| row.code == "TIMETABLE_TEACHER_CONFLICT"));
    timetable_block_service::deactivate_block(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor,
        ),
        third.id,
        version,
        third.row_version,
    )
    .await
    .unwrap();
    let preview = timetable_block_service::preview_placement(&pool, move_request)
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Move);
    assert_eq!(preview.mutation, Some(TimetableBlockMutationKind::Move));
    assert!(preview.conflicts.is_empty());
    let preview = timetable_block_service::preview_placement(&pool, request.clone())
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Swap);
    assert_eq!(preview.mutation, Some(TimetableBlockMutationKind::Swap));
    assert!(preview.conflicts.is_empty());
    let swapped = timetable_block_service::swap_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor,
        ),
        swap,
    )
    .await
    .unwrap();
    assert_eq!(swapped.block_a.day_of_week, "TUE");
    assert_eq!(swapped.block_b.day_of_week, "MON");
    let preview = timetable_block_service::preview_placement(&pool, request)
        .await
        .unwrap();
    assert_eq!(preview.state, TimetableBlockPlacementState::Blocked);
    assert_eq!(preview.conflicts[0].code, "TIMETABLE_STALE_BLOCK");
}
