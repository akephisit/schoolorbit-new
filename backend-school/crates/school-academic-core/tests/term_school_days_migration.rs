use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use sqlx::PgPool;

async fn predecessor(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 154).await.unwrap();
    pool
}

#[tokio::test]
async fn term_school_days155_preserves_historical_rows_receipts_and_empty_year_presets() {
    let pool = predecessor("term_school_days155_preserve").await;
    sqlx::query("INSERT INTO academic_years(year,name,school_days,status,created_at,updated_at) VALUES(2998,'No terms preset','SAT,SUN','planning','2001-01-01','2001-01-02')").execute(&pool).await.unwrap();
    let before: Vec<(String, String)> =
        sqlx::query_as("SELECT id::text,to_jsonb(term)::text FROM academic_terms term ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let receipt_before: Vec<String> = sqlx::query_scalar("SELECT to_jsonb(receipt)::text FROM academic_term_transition_receipts receipt ORDER BY request_id").fetch_all(&pool).await.unwrap();
    let mapping: Vec<(String,String)> = sqlx::query_as("SELECT term.id::text,year.school_days FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id ORDER BY term.id").fetch_all(&pool).await.unwrap();
    apply_migrations_through(&pool, 155).await.unwrap();
    let after: Vec<(String, String)> = sqlx::query_as(
        "SELECT id::text,(to_jsonb(term)-'school_days')::text FROM academic_terms term ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        after, before,
        "all preexisting term identity, revision and timestamp fields remain exact"
    );
    let actual: Vec<(String, String)> =
        sqlx::query_as("SELECT id::text,school_days FROM academic_terms ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(actual, mapping);
    let receipt_after: Vec<String> = sqlx::query_scalar("SELECT to_jsonb(receipt)::text FROM academic_term_transition_receipts receipt ORDER BY request_id").fetch_all(&pool).await.unwrap();
    assert_eq!(receipt_after, receipt_before);
    let empty: (String,String,String) = sqlx::query_as("SELECT default_school_days,created_at::date::text,updated_at::date::text FROM academic_years WHERE year=2998").fetch_one(&pool).await.unwrap();
    assert_eq!(
        empty,
        ("SAT,SUN".into(), "2001-01-01".into(), "2001-01-02".into())
    );
    let before_audit: String = sqlx::query_scalar(
        "SELECT to_jsonb(audit)::text FROM academic_term_school_days_migration_audit audit",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    apply_migrations_through(&pool, 155).await.unwrap();
    let after_audit: String = sqlx::query_scalar(
        "SELECT to_jsonb(audit)::text FROM academic_term_school_days_migration_audit audit",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_audit, before_audit);
    assert!(
        sqlx::query("DELETE FROM academic_term_school_days_migration_audit")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE academic_terms SET school_days='MON,MON'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE academic_years SET default_school_days='MON,MON'")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn term_school_days155_refuses_malformed_presets_without_partial_cutover() {
    let pool = predecessor("term_school_days155_refuse").await;
    for malformed in ["", "{MON,TUE}", "MON,MON", " MON ", "HOLIDAY"] {
        sqlx::query("UPDATE academic_years SET school_days=$1")
            .bind(malformed)
            .execute(&pool)
            .await
            .unwrap();
        let error = apply_migrations_through(&pool, 155).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ACADEMIC_SCHOOL_DAYS155_INVALID_YEAR_PRESET"),
            "{error}"
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND ((table_name='academic_terms' AND column_name='school_days') OR (table_name='academic_years' AND column_name='default_school_days'))").fetch_one(&pool).await.unwrap();
        assert_eq!(count, 0);
        let applied: i64 =
            sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version=155")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(applied, 0);
    }
}
