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
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (term_id, term_start, source_id, source_row_version): (Uuid, NaiveDate, Uuid, i64) =
        sqlx::query_as(
            r#"SELECT term.id, term.start_date, version.id, version.row_version
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

    let listed = timetable_version_service::list_versions(&pool, term_id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, source_id);
    assert!(!listed[0].targets.is_empty());
    assert!(listed[0]
        .targets
        .iter()
        .filter_map(|target| {
            target
                .standard_periods_per_week
                .map(|standard| (target.weekly_period_target, standard))
        })
        .all(|(target, standard)| target == standard));

    let resolved = timetable_version_service::resolve_for_date(&pool, term_id, term_start)
        .await
        .unwrap();
    assert_eq!(resolved.id, source_id);
    let before_start =
        timetable_version_service::resolve_for_date(&pool, term_id, term_start.pred_opt().unwrap())
            .await;
    assert!(matches!(before_start, Err(AppError::NotFound(_))));

    let effective_from = term_start.checked_add_days(Days::new(7)).unwrap();
    let cloned = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from,
            source_row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(cloned.source_version_id, Some(source_id));
    assert_eq!(cloned.effective_from, effective_from);
    assert_eq!(
        cloned
            .targets
            .iter()
            .map(|target| (
                target.learning_offering_id,
                target.weekly_period_target,
                target.standard_periods_per_week,
            ))
            .collect::<Vec<_>>(),
        listed[0]
            .targets
            .iter()
            .map(|target| (
                target.learning_offering_id,
                target.weekly_period_target,
                target.standard_periods_per_week,
            ))
            .collect::<Vec<_>>()
    );

    let source_entry_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id = $1 AND is_active",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let cloned_entry_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id = $1 AND is_active",
    )
    .bind(cloned.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cloned_entry_count, source_entry_count);

    let stale = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from: effective_from.checked_add_days(Days::new(7)).unwrap(),
            source_row_version: source_row_version + 1,
        },
    )
    .await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));

    sqlx::query("UPDATE academic_terms SET status = 'closing' WHERE id = $1")
        .bind(term_id)
        .execute(&pool)
        .await
        .unwrap();
    let closing = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from: effective_from.checked_add_days(Days::new(14)).unwrap(),
            source_row_version,
        },
    )
    .await;
    assert!(
        closing.is_ok(),
        "closing must allow unfinished operational preparation: {closing:?}"
    );
    for (year_status, term_status) in [("closed", "active"), ("active", "closed")] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=(SELECT academic_year_id FROM academic_terms WHERE id=$1)")
            .bind(term_id).bind(year_status).execute(&pool).await.unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN start_date ELSE NULL END WHERE id=$1")
            .bind(term_id).bind(term_status).execute(&pool).await.unwrap();
        let closed = timetable_version_service::clone_draft(
            &pool,
            actor_id,
            source_id,
            CloneTimetableVersionRequest {
                effective_from,
                source_row_version,
            },
        )
        .await;
        assert!(matches!(closed, Err(AppError::Conflict(_))));
        assert_eq!(
            timetable_version_service::resolve_for_date(&pool, term_id, term_start)
                .await
                .unwrap()
                .id,
            source_id
        );
    }
}

#[tokio::test]
async fn clone_draft_includes_active_offerings_missing_from_published_source() {
    let pool = migrated_pool("timetable_version_clone_adds_missing_offering").await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (term_id, year_id, term_start, source_id, source_row_version): (
        Uuid,
        Uuid,
        NaiveDate,
        Uuid,
        i64,
    ) = sqlx::query_as(
        r#"SELECT term.id, term.academic_year_id, term.start_date,
                  version.id, version.row_version
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
    let offering_id =
        insert_deferred_synchronized_offering(&pool, term_id, year_id, "CLONE-MISSING").await;

    let source_has_target: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM academic_timetable_version_targets \
         WHERE timetable_version_id = $1 AND learning_offering_id = $2)",
    )
    .bind(source_id)
    .bind(offering_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!source_has_target);

    let cloned = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from: term_start.checked_add_days(Days::new(7)).unwrap(),
            source_row_version,
        },
    )
    .await
    .unwrap();

    let cloned_target: Option<i32> = sqlx::query_scalar(
        "SELECT weekly_period_target FROM academic_timetable_version_targets \
         WHERE timetable_version_id = $1 AND learning_offering_id = $2",
    )
    .bind(cloned.id)
    .bind(offering_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(cloned_target, Some(1));
}

#[tokio::test]
async fn include_offering_target_adds_existing_deferred_activity_only_to_a_draft() {
    let pool = migrated_pool("timetable_version_include_existing_offering").await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (term_id, year_id, term_start, source_id, source_row_version): (
        Uuid,
        Uuid,
        NaiveDate,
        Uuid,
        i64,
    ) = sqlx::query_as(
        r#"SELECT term.id, term.academic_year_id, term.start_date,
                  version.id, version.row_version
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
    let draft = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from: term_start.checked_add_days(Days::new(7)).unwrap(),
            source_row_version,
        },
    )
    .await
    .unwrap();
    let offering_id =
        insert_deferred_synchronized_offering(&pool, term_id, year_id, "INCLUDE-EXISTING").await;

    let included = timetable_version_service::include_offering_target(&pool, draft.id, offering_id)
        .await
        .unwrap();
    assert_eq!(included.timetable_version_id, draft.id);
    assert_eq!(included.learning_offering_id, offering_id);
    assert_eq!(included.weekly_period_target, 1);

    let repeated = timetable_version_service::include_offering_target(&pool, draft.id, offering_id)
        .await
        .unwrap();
    assert_eq!(repeated, included);

    let published =
        timetable_version_service::include_offering_target(&pool, source_id, offering_id).await;
    assert!(matches!(published, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn cloned_timetable_version_preserves_exact_instructor_sets() {
    let pool = migrated_pool("timetable_version_clone_exact_instructors").await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (source_id, source_row_version, term_start): (Uuid, i64, NaiveDate) = sqlx::query_as(
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
            effective_from: term_start.succ_opt().unwrap(),
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
    let handlers = include_str!("../handlers/timetable_versions.rs");

    assert!(routes.contains("/timetable-versions"));
    assert!(routes.contains("/timetable-versions/resolve"));
    assert!(routes.contains("/timetable-versions/{source_id}/clone"));
    assert!(handlers.contains("pub async fn list_versions"));
    assert!(handlers.contains("pub async fn resolve_version"));
    assert!(handlers.contains("pub async fn clone_version"));
}
