use super::{curriculum_create_options, curriculum_version_views};
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
};
use school_authorization::AcademicResourceListFilter;
use school_test_db::create_named_test_pool;
use uuid::Uuid;

#[tokio::test]
async fn curriculum_read_views_resolve_years_and_create_options_follow_owner_scope() {
    let pool = create_named_test_pool("academic_curriculum_workspace_options").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 43).await.unwrap();

    let curriculum_id = Uuid::parse_str("30000000-0000-0000-0000-000000000001").unwrap();
    let owner_id = Uuid::parse_str("c5e06a47-ebf6-40f6-bbf9-59c509e842f2").unwrap();
    sqlx::query("UPDATE curricula SET owning_organization_unit_id = $1 WHERE id = $2")
        .bind(owner_id)
        .bind(curriculum_id)
        .execute(&pool)
        .await
        .unwrap();

    let views = curriculum_version_views(&pool, curriculum_id)
        .await
        .unwrap();
    assert!(!views.is_empty());
    assert!(views
        .iter()
        .all(|view| !view.start_academic_year_name.trim().is_empty()));

    let unit_filter = AcademicResourceListFilter {
        organization_unit_ids: vec![owner_id],
        ..AcademicResourceListFilter::default()
    };
    let options = curriculum_create_options(&pool, &unit_filter)
        .await
        .unwrap();
    assert!(!options.academic_years.is_empty());
    assert!(!options.grade_levels.is_empty());
    assert_eq!(options.owner_options.len(), 1);
    assert_eq!(
        options.owner_options[0].organization_unit_id,
        Some(owner_id)
    );

    let school_options = curriculum_create_options(
        &pool,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    assert!(school_options
        .owner_options
        .iter()
        .any(|option| option.organization_unit_id.is_none()));
}
