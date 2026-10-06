use super::{curriculum_create_options, curriculum_level_views};
use crate::modules::academic::core::services_tests::prepare_current_core_fixture;
use school_authorization::AcademicResourceListFilter;
#[tokio::test]
async fn curriculum_read_views_resolve_one_parent_edition_and_options_require_school_scope() {
    let pool = prepare_current_core_fixture("academic_curriculum_workspace_options").await;
    let edition_id = sqlx::query_scalar("SELECT id FROM curriculum_editions ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let views = curriculum_level_views(&pool, edition_id).await.unwrap();
    assert!(!views.is_empty());
    assert!(views
        .iter()
        .all(|view| view.level.edition_id == edition_id
            && !view.level.edition_name.trim().is_empty()));
    assert!(
        curriculum_create_options(&pool, &AcademicResourceListFilter::default())
            .await
            .is_err()
    );
    let options = curriculum_create_options(
        &pool,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(!options.grade_levels.is_empty());
}
