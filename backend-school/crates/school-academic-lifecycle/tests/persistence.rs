use school_academic_lifecycle::services;
use school_authorization::ActorContext;
use school_permissions::registry::codes;
use uuid::Uuid;

#[tokio::test]
async fn opening_policy_reads_through_the_lifecycle_crate_boundary() {
    let pool = school_test_db::create_named_test_pool("academic_lifecycle_crate_persistence").await;
    school_test_db::run_test_migrations(&pool).await;
    let actor = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };

    let policy = services::get_opening_policy(&pool, &actor).await.unwrap();

    assert_eq!(policy.row_version, 1);
    assert!(!policy.require_homeroom_placements);
    assert!(!policy.require_published_offerings);
    assert!(!policy.require_published_timetable);
}
