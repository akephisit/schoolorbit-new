use super::*;
use crate::integration_tests::academic::core;
use school_permissions::registry::codes;

async fn fixture(name: &str) -> (PgPool, ActorContext, Uuid, Uuid) {
    let pool = core::services_tests::prepare_core_fixture(name).await;
    let (year, term) =
        sqlx::query_as("SELECT academic_year_id,id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor = ActorContext {
        user_id: sqlx::query_scalar(
            "SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        permissions: vec![codes::WILDCARD.into()],
    };
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE academic_year_id=$1 AND id<>$2")
        .bind(year)
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    (pool, actor, year, term)
}

async fn roster(pool: &PgPool, year: Uuid) -> serde_json::Value {
    sqlx::query_scalar::<_,sqlx::types::Json<serde_json::Value>>("SELECT jsonb_build_object('annual',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM student_academic_years s WHERE academic_year_id=$1),'placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM homeroom_placements p WHERE academic_year_id=$1))").bind(year).fetch_one(pool).await.unwrap().0
}

async fn close_term(pool: &PgPool, actor: &ActorContext, year: Uuid, term: Uuid) {
    // This provider isolates academic closure from Attendance's separately tested
    // finalization barrier while exercising the actual Lifecycle command/receipt.
    let external = super::provider_tests::StubExternalLifecyclePort {
        exam_revision: "period-fixture",
        fail_exams: false,
    };
    let workspace =
        school_academic_lifecycle::services::get_workspace(&external, pool, actor, year, term)
            .await
            .unwrap();
    assert!(workspace
        .findings
        .iter()
        .all(|finding| !finding.code.starts_with("results.")));
    let outcome = school_academic_lifecycle::services::term_transitions::transition_term(
        &external,
        pool,
        actor,
        term,
        TermTransitionRequest {
            academic_year_id: year,
            request_id: Uuid::new_v4(),
            action: TermTransitionAction::Close,
            expected_year_version: workspace.context.year_row_version,
            expected_term_version: workspace.context.term_row_version,
            readiness_checksum: workspace.source_checksum,
            acknowledged_warning_codes: workspace
                .findings
                .iter()
                .filter(|f| f.severity == LifecycleSeverity::Warning)
                .map(|f| f.code.clone())
                .collect(),
            reason: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        outcome.context.term_status,
        school_academic_core::models::AcademicTermStatus::Closed
    );
}

#[tokio::test]
async fn manual_period_closure_directly_closes_active_term_and_year_without_aggregate_results_or_roster_changes(
) {
    let (pool, actor, year, term) = fixture("manual_period_no_results").await;
    let original = roster(&pool, year).await;
    let results: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_annual_result_revisions WHERE academic_year_id=$1",
    )
    .bind(year)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(results, 0);
    close_term(&pool, &actor, year, term).await;
    let workspace = get_year_workspace(&pool, &actor, year).await.unwrap();
    assert!(workspace.can_close, "{:?}", workspace.findings);
    let outcome = year_transitions::transition_year(
        &pool,
        &actor,
        year,
        YearTransitionRequest {
            request_id: Uuid::new_v4(),
            action: YearTransitionAction::Close,
            expected_year_version: workspace.context.row_version,
            readiness_checksum: workspace.source_checksum,
            acknowledged_warning_codes: workspace
                .findings
                .iter()
                .filter(|f| f.severity == LifecycleSeverity::Warning)
                .map(|f| f.code.clone())
                .collect(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        outcome.context.status,
        school_academic_core::models::AcademicYearStatus::Closed
    );
    assert_eq!(roster(&pool, year).await, original);
}

#[tokio::test]
async fn manual_period_next_term_activates_from_planning_without_promoting_or_recreating_annual_roster(
) {
    use school_academic_core::models::{AcademicTermType, CreateAcademicTermRequest};
    let (pool, actor, year, term) = fixture("manual_period_same_year").await;
    let original = roster(&pool, year).await;
    close_term(&pool, &actor, year, term).await;
    let bell = sqlx::query_scalar("SELECT bell_schedule_id FROM academic_terms WHERE id=$1")
        .bind(term)
        .fetch_one(&pool)
        .await
        .unwrap();
    let next = school_academic_core::services::years_terms::create_term(
        &pool,
        actor.user_id,
        CreateAcademicTermRequest {
            academic_year_id: year,
            term_type: AcademicTermType::Regular,
            custom_name: None,
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: bell,
        },
    )
    .await
    .unwrap();
    let workspace = get_activation_workspace(&pool, &actor, year, next.id)
        .await
        .unwrap();
    assert!(!workspace.opens_year);
    assert!(workspace.can_activate, "{:?}", workspace.findings);
    let outcome = term_transitions::transition_term(
        &pool,
        &actor,
        next.id,
        TermTransitionRequest {
            academic_year_id: year,
            request_id: Uuid::new_v4(),
            action: TermTransitionAction::Activate,
            expected_year_version: workspace.context.year_row_version,
            expected_term_version: workspace.context.term_row_version,
            readiness_checksum: workspace.source_checksum,
            acknowledged_warning_codes: vec![],
            reason: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        outcome.context.term_status,
        school_academic_core::models::AcademicTermStatus::Active
    );
    assert_eq!(roster(&pool, year).await, original);
}
