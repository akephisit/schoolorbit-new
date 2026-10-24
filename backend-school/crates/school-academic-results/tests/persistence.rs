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

#[tokio::test]
async fn result_cutover_audit_reuses_the_callers_single_connection() {
    let pool = school_test_db::create_named_test_pool("results_borrowed_connection").await;
    school_test_db::run_test_migrations(&pool).await;
    let mut connection = pool.acquire().await.unwrap();
    let audit = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        services::read_gradebook_results_cutover_audit_on_connection(&mut connection),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(audit.completed, "cutover checks: {:?}", audit.checks);
    assert_eq!(audit.checks.len(), 6);
}
