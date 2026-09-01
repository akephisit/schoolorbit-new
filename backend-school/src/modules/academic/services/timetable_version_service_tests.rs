use chrono::{Days, NaiveDate};
use uuid::Uuid;

use super::timetable_version_service;
use crate::error::AppError;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::modules::academic::models::timetable_version::CloneTimetableVersionRequest;
use crate::test_helpers::create_named_test_pool;

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 58).await.unwrap();
    pool
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
    let closed = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from: effective_from.checked_add_days(Days::new(14)).unwrap(),
            source_row_version,
        },
    )
    .await;
    assert!(matches!(closed, Err(AppError::Conflict(_))));
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
