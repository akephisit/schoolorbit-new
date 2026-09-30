use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use school_test_db::create_named_test_pool_with_max_connections;
use uuid::Uuid;

#[tokio::test]
async fn public_profile_does_not_hide_failed_role_or_organization_reads() {
    let pool = create_named_test_pool_with_max_connections("public_profile_read_errors", 5).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    let staff_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    assert!(
        school_staff::services::staff_service::get_public_staff_profile(&pool, staff_id)
            .await
            .is_ok()
    );
    for (table, hide, restore) in [
        (
            "roles",
            "ALTER TABLE roles RENAME TO profile_test_hidden_table",
            "ALTER TABLE profile_test_hidden_table RENAME TO roles",
        ),
        (
            "organization_units",
            "ALTER TABLE organization_units RENAME TO profile_test_hidden_table",
            "ALTER TABLE profile_test_hidden_table RENAME TO organization_units",
        ),
    ] {
        sqlx::query(hide).execute(&pool).await.unwrap();
        let result =
            school_staff::services::staff_service::get_public_staff_profile(&pool, staff_id).await;
        assert!(
            result.is_err(),
            "A failed {table} read must not return an empty successful profile"
        );
        sqlx::query(restore).execute(&pool).await.unwrap();
    }
}

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

#[tokio::test]
async fn private_profile_does_not_turn_failed_relations_into_empty_success() {
    let pool = create_named_test_pool_with_max_connections("private_profile_read_errors", 5).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    let staff_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    assert!(
        school_staff::services::staff_service::get_staff_profile(&pool, staff_id, false)
            .await
            .is_ok()
    );
    for (table, hide, restore) in [
        (
            "staff_info",
            "ALTER TABLE staff_info RENAME TO private_profile_hidden_table",
            "ALTER TABLE private_profile_hidden_table RENAME TO staff_info",
        ),
        (
            "roles",
            "ALTER TABLE roles RENAME TO private_profile_hidden_table",
            "ALTER TABLE private_profile_hidden_table RENAME TO roles",
        ),
        (
            "organization_units",
            "ALTER TABLE organization_units RENAME TO private_profile_hidden_table",
            "ALTER TABLE private_profile_hidden_table RENAME TO organization_units",
        ),
        (
            "learning_group_teachers",
            "ALTER TABLE learning_group_teachers RENAME TO private_profile_hidden_table",
            "ALTER TABLE private_profile_hidden_table RENAME TO learning_group_teachers",
        ),
        (
            "homeroom_advisors",
            "ALTER TABLE homeroom_advisors RENAME TO private_profile_hidden_table",
            "ALTER TABLE private_profile_hidden_table RENAME TO homeroom_advisors",
        ),
    ] {
        sqlx::query(hide).execute(&pool).await.unwrap();
        let result =
            school_staff::services::staff_service::get_staff_profile(&pool, staff_id, false).await;
        sqlx::query(restore).execute(&pool).await.unwrap();
        assert!(
            result.is_err(),
            "A failed {table} read must not become an empty successful profile"
        );
    }
}
