use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use school_test_db::create_named_test_pool;
use uuid::Uuid;

#[tokio::test]
async fn dashboard_counts_homerooms_only_in_the_selected_academic_year() {
    let pool = create_named_test_pool("staff_dashboard_selected_year").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();

    let selected_year_id: Uuid =
        sqlx::query_scalar("SELECT id FROM academic_years WHERE year = 2025")
            .fetch_one(&pool)
            .await
            .unwrap();
    let other_year_id: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE year = 2024")
        .fetch_one(&pool)
        .await
        .unwrap();

    let selected =
        school_staff::services::dashboard_service::get_staff_dashboard(&pool, selected_year_id)
            .await
            .unwrap();
    let other =
        school_staff::services::dashboard_service::get_staff_dashboard(&pool, other_year_id)
            .await
            .unwrap();

    assert_eq!(selected.active_homerooms, 2);
    assert_eq!(other.active_homerooms, 1);
}
