use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use school_students::services::get_own_profile;
use school_test_db::create_named_test_pool;
use uuid::Uuid;

#[tokio::test]
async fn own_profile_uses_the_caller_selected_student_academic_year() {
    let pool = create_named_test_pool("student_profile_selected_year").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();

    let student_id = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
    let year_2025_id: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE year = 2025")
        .fetch_one(&pool)
        .await
        .unwrap();
    let year_2026_id: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE year = 2026")
        .fetch_one(&pool)
        .await
        .unwrap();

    let current = get_own_profile(&pool, student_id, year_2025_id, false)
        .await
        .unwrap();
    let planned = get_own_profile(&pool, student_id, year_2026_id, false)
        .await
        .unwrap();

    assert_eq!(current.info.homeroom.as_deref(), Some("ม.1/1 ปี 2025"));
    assert_eq!(planned.info.homeroom.as_deref(), Some("ม.1/1 ปี 2026"));
}
