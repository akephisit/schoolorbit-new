use school_authorization::{get_cached_user_permissions, PermissionCache};
use school_test_db::create_named_test_pool;
use uuid::Uuid;

use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture, CutoverFixture,
};

#[tokio::test]
async fn effective_permissions_exclude_removed_legacy_cutover_permissions() {
    let pool = create_named_test_pool("permission_cutover_effective").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44).await.unwrap();
    record_passing_phase_a_reconciliation_marker(&pool)
        .await
        .unwrap();
    school_migrations::run_tenant_migrations(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
           VALUES (
               '50000000-0000-0000-0000-000000000002',
               'a1b2c957-bf35-47f8-bbf4-8a67ce6b777f',
               true,
               '2025-05-01'
           )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let permissions = get_cached_user_permissions(
        "permission-cutover-effective",
        Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap(),
        &pool,
        &PermissionCache::new(),
    )
    .await
    .unwrap();

    assert!(permissions
        .iter()
        .any(|code| code == "academic_context.read.school"));
    assert!(permissions
        .iter()
        .any(|code| code == "academic_year.read.school"));
    assert!(!permissions
        .iter()
        .any(|code| code == "academic_structure.read.all"));
}
