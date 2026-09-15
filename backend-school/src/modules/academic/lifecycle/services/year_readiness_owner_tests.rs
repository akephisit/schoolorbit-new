use super::*;
use crate::modules::academic::{core, cutover_test_support::apply_migrations_through};
use school_permissions::registry::codes;

#[tokio::test]
async fn year_lifecycle_readiness_distinguishes_missing_annual_results_and_optional_terms() {
    let pool = core::services_tests::prepare_core_fixture("year_lifecycle_readiness").await;
    apply_migrations_through(&pool, 70).await.unwrap();
    let (year, term): (Uuid, Uuid) =
        sqlx::query_as("SELECT academic_year_id,id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let reader = ActorContext {
        user_id: id,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    let ws = get_year_workspace(&pool, &reader, year).await.unwrap();
    let manager = ActorContext {
        user_id: id,
        permissions: vec![codes::WILDCARD.into()],
    };
    let privileged = get_year_workspace(&pool, &manager, year).await.unwrap();
    assert_eq!(ws.source_checksum, privileged.source_checksum);
    assert!(privileged
        .findings
        .iter()
        .filter(|row| row.code.starts_with("year.annual"))
        .all(|row| row.resolution_url.is_some()));
    assert!(!ws.can_close && !ws.coverage.ready && ws.available_actions.is_empty());
    assert!(ws
        .findings
        .iter()
        .any(|row| row.code == "year.annual_missing"));
    assert!(ws.findings.iter().any(|row| row.code == "year.terms_open"));
    assert!(ws
        .findings
        .iter()
        .filter(|row| row.code.starts_with("year.annual"))
        .all(|row| row.resolution_url.is_none()));
    let optional: Uuid = sqlx::query_scalar("SELECT id FROM academic_terms WHERE academic_year_id=$1 AND id<>$2 ORDER BY sequence_no LIMIT 1").bind(year).bind(term).fetch_one(&pool).await.unwrap();
    sqlx::query(
        "UPDATE academic_terms SET status='closed',closed_on=start_date WHERE academic_year_id=$1",
    )
    .bind(year)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='planning',closed_on=NULL,included_in_year_result=false,blocks_year_closure=false,term_type='summer' WHERE id=$1").bind(optional).execute(&pool).await.unwrap();
    let ws = get_year_workspace(&pool, &reader, year).await.unwrap();
    assert!(!ws.findings.iter().any(|row| row.code == "year.terms_open"));
    assert!(ws
        .findings
        .iter()
        .any(|row| row.code == "year.optional_terms"));
    sqlx::query("UPDATE academic_terms SET status='active' WHERE id=$1")
        .bind(optional)
        .execute(&pool)
        .await
        .unwrap();
    let ws = get_year_workspace(&pool, &reader, year).await.unwrap();
    assert!(ws.findings.iter().any(|row| row.code == "year.terms_open"));
    assert!(get_year_workspace(&pool, &reader, Uuid::new_v4())
        .await
        .is_err());
    let denied = ActorContext {
        user_id: id,
        permissions: vec![],
    };
    assert!(matches!(
        get_year_workspace(&pool, &denied, year).await,
        Err(AppError::Forbidden(_))
    ));
}
