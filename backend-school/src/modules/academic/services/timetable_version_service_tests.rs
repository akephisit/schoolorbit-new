use chrono::{Days, NaiveDate};
use uuid::Uuid;

use super::timetable_version_service;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use school_academic_timetable::models::timetable_version::CloneTimetableVersionRequest;
use school_errors::AppError;
use school_test_db::create_named_test_pool;

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 59).await.unwrap();
    pool
}

async fn insert_deferred_synchronized_offering(
    pool: &sqlx::PgPool,
    term_id: Uuid,
    year_id: Uuid,
    code: &str,
) -> Uuid {
    let offering_id = Uuid::new_v4();
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query(
        r#"INSERT INTO learning_offerings (
               id, academic_term_id, academic_year_id, kind, code_snapshot,
               name_snapshot, status, owning_organization_unit_id
           )
           SELECT $1, $2, $3, 'activity', $4,
                  'กิจกรรมรอจัดกลุ่ม', 'draft', activity.owning_organization_unit_id
           FROM activities activity
           JOIN activity_versions version ON version.activity_id = activity.id
           WHERE version.scheduling_mode = 'synchronized'
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(offering_id)
    .bind(term_id)
    .bind(year_id)
    .bind(code)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO activity_offering_details (
               learning_offering_id, academic_term_id, academic_year_id,
               activity_version_id, activity_id, curriculum_activity_requirement_id,
               registration_type, scheduling_mode, hours, capacity,
               attendance_requirement, pass_criteria
           )
           SELECT $1, $2, $3, version.id, version.activity_id, NULL,
                  'assigned', 'synchronized', version.hours_per_week, NULL,
                  '{}'::jsonb, '{}'::jsonb
           FROM activity_versions version
           WHERE version.scheduling_mode = 'synchronized'
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(offering_id)
    .bind(term_id)
    .bind(year_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_offering_targets (
               id, learning_offering_id, academic_term_id, academic_year_id,
               target_kind, homeroom_id, grade_level_id, study_program_id
           )
           SELECT gen_random_uuid(), $1, $2, $3, 'homeroom',
                  homeroom.id, homeroom.grade_level_id, homeroom.study_program_id
           FROM homerooms homeroom
           WHERE homeroom.academic_year_id = $3 AND homeroom.is_active
           ORDER BY homeroom.id
           LIMIT 1"#,
    )
    .bind(offering_id)
    .bind(term_id)
    .bind(year_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    offering_id
}

#[tokio::test]
async fn list_resolve_and_clone_preserve_version_isolation_and_targets() {
    let pool = migrated_pool("timetable_version_list_resolve_clone").await;
    apply_migrations_through(&pool, 89).await.unwrap();
    let (term_id,term_start,source_id,actor_id): (Uuid,NaiveDate,Uuid,Uuid)=sqlx::query_as("SELECT version.academic_term_id,version.effective_from,version.id,version.published_by FROM academic_timetable_versions version WHERE status='published' AND academic_term_id IN (SELECT id FROM academic_terms WHERE status='active') ORDER BY effective_from,id LIMIT 1").fetch_one(&pool).await.unwrap();
    let source = timetable_version_service::get_version(&pool, source_id, term_start)
        .await
        .unwrap();
    let listed = timetable_version_service::list_versions(&pool, term_id)
        .await
        .unwrap();
    assert!(listed
        .iter()
        .any(|version| version.id == source_id && !version.targets.is_empty()));
    assert_eq!(
        timetable_version_service::resolve_for_date(&pool, term_id, term_start)
            .await
            .unwrap()
            .id,
        source_id
    );
    assert!(matches!(
        timetable_version_service::resolve_for_date(&pool, term_id, term_start.pred_opt().unwrap())
            .await,
        Err(AppError::NotFound(_))
    ));
    let cloned = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(cloned.source_version_id, Some(source_id));
    assert_eq!(cloned.effective_from, None, "date is chosen at publication");
    assert_eq!(cloned.delivery_version_id, source.delivery_version_id);
    assert_eq!(
        cloned
            .targets
            .iter()
            .map(|target| (target.learning_offering_id, target.weekly_period_target))
            .collect::<Vec<_>>(),
        source
            .targets
            .iter()
            .map(|target| (target.learning_offering_id, target.weekly_period_target))
            .collect::<Vec<_>>()
    );
    let resumed = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resumed.id, cloned.id);
    assert!(matches!(
        timetable_version_service::clone_draft(
            &pool,
            actor_id,
            source_id,
            CloneTimetableVersionRequest {
                source_row_version: source.row_version + 1,
                resume_draft_id: None,
                draft_row_version: None
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    sqlx::query("UPDATE academic_terms SET status='closing' WHERE id=$1")
        .bind(term_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: Some(cloned.id),
            draft_row_version: Some(cloned.row_version)
        }
    )
    .await
    .is_ok());
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=start_date WHERE id=$1")
        .bind(term_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        timetable_version_service::clone_draft(
            &pool,
            actor_id,
            source_id,
            CloneTimetableVersionRequest {
                source_row_version: source.row_version,
                resume_draft_id: None,
                draft_row_version: None
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn clone_draft_uses_only_its_published_opening_not_unpublished_registry_resources() {
    let pool = migrated_pool("timetable_version_no_raw_resources").await;
    apply_migrations_through(&pool, 89).await.unwrap();
    let (source_id,term_id,year_id,actor): (Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT id,academic_term_id,academic_year_id,published_by FROM academic_timetable_versions WHERE status='published' AND academic_term_id IN (SELECT id FROM academic_terms WHERE status='active') ORDER BY effective_from,id LIMIT 1").fetch_one(&pool).await.unwrap();
    let offering =
        insert_deferred_synchronized_offering(&pool, term_id, year_id, "UNPUBLISHED-REGISTRY")
            .await;
    let source =
        timetable_version_service::get_version(&pool, source_id, chrono::Utc::now().date_naive())
            .await
            .unwrap();
    let cloned = timetable_version_service::clone_draft(
        &pool,
        actor,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    assert!(!cloned
        .targets
        .iter()
        .any(|target| target.learning_offering_id == offering));
    assert_eq!(cloned.delivery_version_id, source.delivery_version_id);
    assert_eq!(cloned.targets.len(), source.targets.len());
}

#[tokio::test]
async fn timetable_source_update_rejects_an_unpublished_opening_without_partial_changes() {
    let pool = migrated_pool("timetable_version_source_update_guard").await;
    apply_migrations_through(&pool, 89).await.unwrap();
    let (source_id,actor): (Uuid,Uuid)=sqlx::query_as("SELECT id,published_by FROM academic_timetable_versions WHERE status='published' AND academic_term_id IN (SELECT id FROM academic_terms WHERE status='active') ORDER BY effective_from,id LIMIT 1").fetch_one(&pool).await.unwrap();
    let source =
        timetable_version_service::get_version(&pool, source_id, chrono::Utc::now().date_naive())
            .await
            .unwrap();
    let date = source.effective_from.unwrap().succ_opt().unwrap();
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(source.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
    let opening = school_academic_delivery::services::change_sets::create_change_set(
        &pool,
        actor,
        school_academic_delivery::models::CreateAcademicTermChangeSetRequest {
            academic_term_id: source.academic_term_id,
            reason: "ร่างเปิดสอนยังไม่เผยแพร่".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let cloned = timetable_version_service::clone_draft(
        &pool,
        actor,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    let failure=school_academic_timetable::services::timetable_lifecycle::update_source(&pool,actor,cloned.id,school_academic_timetable::models::timetable_version::UpdateTimetableDeliverySourceRequest {
        row_version:cloned.row_version,delivery_version_id:opening.target_delivery_version_id
    }).await.unwrap_err();
    assert!(matches!(failure, AppError::ValidationError(_)));
    let unchanged = timetable_version_service::get_version(&pool, cloned.id, date)
        .await
        .unwrap();
    assert_eq!(unchanged.delivery_version_id, source.delivery_version_id);
    assert_eq!(unchanged.row_version, cloned.row_version);
}

#[tokio::test]
async fn migration_080_reconciles_eligible_offerings_into_existing_drafts() {
    let pool = migrated_pool("timetable_version_reconcile_existing_draft").await;
    apply_migrations_through(&pool, 79).await.unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (term_id, year_id, term_start, source_id, bell_schedule_id): (
        Uuid,
        Uuid,
        NaiveDate,
        Uuid,
        Uuid,
    ) = sqlx::query_as(
        r#"SELECT term.id, term.academic_year_id, term.start_date,
                  version.id, version.bell_schedule_id
           FROM academic_terms term
           JOIN academic_timetable_versions version
             ON version.academic_term_id = term.id
            AND version.status = 'published'
           WHERE term.status = 'active'
           ORDER BY version.effective_from, version.id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let draft_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_timetable_versions (
               id, academic_term_id, academic_year_id, effective_from, status,
               source_version_id, bell_schedule_id, created_by
           ) VALUES ($1, $2, $3, $4, 'draft', $5, $6, $7)"#,
    )
    .bind(draft_id)
    .bind(term_id)
    .bind(year_id)
    .bind(term_start.checked_add_days(Days::new(7)).unwrap())
    .bind(source_id)
    .bind(bell_schedule_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    let offering_id =
        insert_deferred_synchronized_offering(&pool, term_id, year_id, "MIGRATION-080").await;

    apply_migrations_through(&pool, 80)
        .await
        .expect("migration 080 must reconcile existing draft targets");

    let reconciled: Option<(i32, serde_json::Value)> = sqlx::query_as(
        r#"SELECT weekly_period_target, migration_provenance
           FROM academic_timetable_version_targets
           WHERE timetable_version_id = $1 AND learning_offering_id = $2"#,
    )
    .bind(draft_id)
    .bind(offering_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(
        reconciled,
        Some((1, serde_json::json!({ "reconciledByMigration": 80 })))
    );
    let row_version: i64 =
        sqlx::query_scalar("SELECT row_version FROM academic_timetable_versions WHERE id = $1")
            .bind(draft_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row_version, 2);
}

#[tokio::test]
async fn cloned_timetable_version_preserves_exact_instructor_sets() {
    let pool = migrated_pool("timetable_version_clone_exact_instructors").await;
    apply_migrations_through(&pool, 89).await.unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (source_id, source_row_version, _term_start): (Uuid, i64, NaiveDate) = sqlx::query_as(
        r#"SELECT version.id, version.row_version, term.start_date
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.status = 'published' AND term.status = 'active'
           ORDER BY version.effective_from, version.id LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let cloned = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            resume_draft_id: None,
            draft_row_version: None,
            source_row_version,
        },
    )
    .await
    .unwrap();

    let (source_entry_count, mapped_entry_count, mismatched_instructor_sets): (i64, i64, i64) =
        sqlx::query_as(
            r#"SELECT (
                      SELECT count(*)
                      FROM academic_timetable_blocks
                      WHERE timetable_version_id = $1 AND is_active
                  ),
                  count(*),
                  count(*) FILTER (
                      WHERE ARRAY(
                          SELECT concat(instructor.instructor_id::text, ':', instructor.role::text)
                          FROM academic_timetable_block_groups block_group
                          JOIN academic_timetable_block_group_instructors instructor
                            ON instructor.block_group_id = block_group.id
                          WHERE block_group.block_id = source.id AND block_group.is_active
                          ORDER BY instructor.instructor_id
                      ) <> ARRAY(
                          SELECT concat(instructor.instructor_id::text, ':', instructor.role::text)
                          FROM academic_timetable_block_groups block_group
                          JOIN academic_timetable_block_group_instructors instructor
                            ON instructor.block_group_id = block_group.id
                          WHERE block_group.block_id = target.id AND block_group.is_active
                          ORDER BY instructor.instructor_id
                      )
                  )
           FROM academic_timetable_blocks source
           JOIN academic_timetable_blocks target
             ON target.timetable_version_id = $2
            AND target.migration_provenance ->> 'clonedFromBlockId' = source.id::text
           WHERE source.timetable_version_id = $1 AND source.is_active"#,
        )
        .bind(source_id)
        .bind(cloned.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(source_entry_count > 0);
    assert_eq!(mapped_entry_count, source_entry_count);
    assert_eq!(mismatched_instructor_sets, 0);
}

#[test]
fn academic_routes_expose_timetable_version_workflow() {
    let routes = include_str!("../../academic.rs");
    let handlers = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/school-academic-http/src/handlers/timetable_versions.rs"
    ));

    assert!(routes.contains("/timetable-versions"));
    assert!(routes.contains("/timetable-versions/resolve"));
    assert!(routes.contains("/timetable-versions/{source_id}/clone"));
    assert!(handlers.contains("pub async fn list_versions"));
    assert!(handlers.contains("pub async fn resolve_version"));
    assert!(handlers.contains("pub async fn clone_version"));
}
