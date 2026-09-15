use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use school_test_db::create_named_test_pool_with_max_connections;
use uuid::Uuid;

#[tokio::test]
async fn staff_profile_reads_canonical_teaching_and_homeroom_assignments() {
    let pool = create_named_test_pool_with_max_connections("staff_profile_canonical", 5).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();

    let staff_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let homeroom_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id = (
             SELECT id FROM academic_years WHERE year = 2025
         ) ORDER BY code LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO homeroom_advisors (homeroom_id, user_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(homeroom_id)
    .bind(staff_id)
    .execute(&pool)
    .await
    .unwrap();

    let profile = school_staff::services::staff_service::get_staff_profile(&pool, staff_id, false)
        .await
        .unwrap();

    assert!(profile
        .teaching_assignments
        .iter()
        .any(|assignment| assignment.academic_year == 2025));
    assert!(profile.advisor_homerooms.iter().any(|assignment| {
        assignment.homeroom_id == homeroom_id && assignment.academic_year == 2025
    }));
}
