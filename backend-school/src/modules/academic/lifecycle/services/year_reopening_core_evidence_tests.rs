use super::*;
use crate::modules::academic::{core, cutover_test_support::apply_migrations_through};

#[tokio::test]
async fn year_reopening_core_evidence_keeps_successor_activation_history() {
    let pool = core::services_tests::prepare_core_fixture("year_reopening_core_history").await;
    apply_migrations_through(&pool, 76).await.unwrap();
    let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE status='active'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let target: Uuid = sqlx::query_scalar("INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) SELECT max(year)+1,'E2E-LIFECYCLE-recovery-history',max(end_date)+1,max(end_date)+366,'MON','planning' FROM academic_years RETURNING id").fetch_one(&pool).await.unwrap();
    let actor: Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO academic_year_transition_receipts(request_id,academic_year_id,actor_user_id,action,request_checksum,accepted_readiness,outcome) VALUES($1,$2,$3,'activate',$4,'{}','{}')").bind(Uuid::new_v4()).bind(target).bind(actor).bind("a".repeat(64)).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let state =
        school_academic_core::services::lifecycle_context::read_year_recovery_state(&mut tx, year)
            .await
            .unwrap();
    assert_eq!(state.context.academic_year_id, year);
    assert_eq!(state.successor_years, 1);
    assert!(state.running_terms > 0);
}
