use school_academic_results::services;

#[tokio::test]
async fn gradebook_results_cutover_audit_reads_the_canonical_migrated_schema() {
    let pool = school_test_db::create_named_test_pool("academic_results_crate_persistence").await;
    school_test_db::run_test_migrations(&pool).await;

    let audit = services::read_gradebook_results_cutover_audit(&pool)
        .await
        .unwrap();

    assert!(audit.completed, "cutover checks: {:?}", audit.checks);
    assert_eq!(audit.checks.len(), 6);
}
