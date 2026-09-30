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

#[tokio::test]
async fn profile_read_reuses_target_without_broadening_profile_or_pii_access() {
    use crate::policies::student_access_policy::authorize_student_profile_read;
    use school_authorization::ActorContext;
    use school_permissions::registry::codes;
    let pool = create_named_test_pool("student_profile_authorization").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    let student_id = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
    let teacher_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    sqlx::query("INSERT INTO homeroom_advisors (homeroom_id, user_id, role) SELECT homeroom_id, $1, 'primary' FROM homeroom_placements WHERE status = 'current' LIMIT 1")
        .bind(teacher_id).execute(&pool).await.unwrap();
    let actor = |id, permissions: &[&str]| ActorContext {
        user_id: id,
        permissions: permissions.iter().map(|code| code.to_string()).collect(),
    };
    for (user_id, read, pii) in [
        (
            student_id,
            codes::STUDENT_READ_OWN,
            codes::STUDENT_PII_READ_OWN,
        ),
        (
            teacher_id,
            codes::STUDENT_READ_ASSIGNED,
            codes::STUDENT_PII_READ_ASSIGNED,
        ),
        (
            Uuid::new_v4(),
            codes::STUDENT_READ_SCHOOL,
            codes::STUDENT_PII_READ_SCHOOL,
        ),
    ] {
        assert!(
            !authorize_student_profile_read(&pool, &actor(user_id, &[read]), student_id)
                .await
                .unwrap()
        );
        assert!(
            authorize_student_profile_read(&pool, &actor(user_id, &[read, pii]), student_id)
                .await
                .unwrap()
        );
    }
    for permissions in [
        vec![codes::STUDENT_PII_READ_SCHOOL],
        vec![codes::STUDENT_READ_OWN],
        vec![codes::STUDENT_READ_ASSIGNED],
    ] {
        assert!(authorize_student_profile_read(
            &pool,
            &actor(Uuid::new_v4(), &permissions),
            student_id
        )
        .await
        .is_err());
    }
    assert!(!authorize_student_profile_read(
        &pool,
        &actor(
            Uuid::new_v4(),
            &[codes::STUDENT_READ_SCHOOL, codes::STUDENT_PII_READ_ASSIGNED]
        ),
        student_id
    )
    .await
    .unwrap());
}
