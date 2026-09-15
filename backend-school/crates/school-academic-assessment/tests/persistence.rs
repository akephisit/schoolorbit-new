use school_academic_assessment::assessment::services;
use uuid::Uuid;

#[tokio::test]
async fn assessment_phase_controls_read_through_the_assessment_crate_boundary() {
    let pool =
        school_test_db::create_named_test_pool("academic_assessment_crate_persistence").await;
    school_test_db::run_test_migrations(&pool).await;

    let controls = services::list_phase_controls(&pool, Uuid::new_v4())
        .await
        .unwrap();

    assert!(controls.is_empty());
    let relation: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('academic_assessment_phase_controls')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        relation.as_deref(),
        Some("academic_assessment_phase_controls")
    );
}
