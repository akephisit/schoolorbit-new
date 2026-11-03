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

async fn wait_for_transition_waiter(pool: &PgPool) {
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND database=(SELECT oid FROM pg_database WHERE datname=current_database()) AND classid::bigint=$1 AND objid::bigint=$2 AND NOT granted)")
                .bind(i64::from(school_academic_core::services::lifecycle_guard::TRANSITION_NAMESPACE)).bind(i64::from(school_academic_core::services::lifecycle_guard::TRANSITION_KEY)).fetch_one(pool).await.unwrap();
            if waiting {break;}
            tokio::task::yield_now().await;
        }
    }).await.expect("owned operation must reach transition lock barrier");
}

#[tokio::test]
async fn manual_period_enable_waiting_behind_close_excludes_the_already_accepted_period() {
    let (pool, actor, year, term) = fixture("manual_period_enable_boundary").await;
    let mut closing = pool.begin().await.unwrap();
    school_academic_core::services::lifecycle_guard::lock_transition(&mut closing)
        .await
        .unwrap();
    let enable_pool = pool.clone();
    let enable_actor = actor.clone();
    let enabled = tokio::spawn(async move {
        school_academic_lifecycle::services::retention::update_policy(
            &enable_pool,
            &enable_actor,
            UpdateAcademicRetentionPolicy {
                expected_row_version: 1,
                enabled: true,
                primary_boundary: AcademicRetentionBoundary::Year,
                secondary_boundary: AcademicRetentionBoundary::Term,
                grace_days: 90,
            },
        )
        .await
    });
    wait_for_transition_waiter(&pool).await;
    // Synthetic predecessor transaction holds the canonical lock and accepts its
    // close after the enabling transaction has begun, before enable may commit.
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(term)
        .execute(&mut *closing)
        .await
        .unwrap();
    let closed:chrono::DateTime<chrono::Utc>=sqlx::query_scalar("INSERT INTO academic_term_transition_receipts(request_id,academic_year_id,academic_term_id,actor_user_id,action,request_checksum,accepted_readiness,outcome,canonical_request,created_at) VALUES(uuidv7(),$1,$2,$3,'close',repeat('a',64),'{}','{}','{}',clock_timestamp()) RETURNING created_at")
        .bind(year).bind(term).bind(actor.user_id).fetch_one(&mut *closing).await.unwrap();
    closing.commit().await.unwrap();
    let policy = enabled.await.unwrap().unwrap();
    assert!(policy.effective_from.unwrap() > closed);
    let workspace = school_academic_lifecycle::services::retention::get_workspace(
        &crate::adapters::academic_retention::ACADEMIC_RETENTION,
        &pool,
        &actor,
        year,
    )
    .await
    .unwrap();
    assert!(workspace
        .terms
        .iter()
        .find(|row| row.academic_term_id == term)
        .unwrap()
        .blocked_reasons
        .contains(&"outside_policy_effective_period".into()));
}

#[tokio::test]
async fn manual_period_close_waiting_behind_enable_uses_its_actual_acceptance_timestamp() {
    let (pool, actor, year, term) = fixture("manual_period_close_boundary").await;
    let external = super::provider_tests::StubExternalLifecyclePort {
        exam_revision: "acceptance-fixture",
        fail_exams: false,
    };
    let workspace =
        school_academic_lifecycle::services::get_workspace(&external, &pool, &actor, year, term)
            .await
            .unwrap();
    let request = TermTransitionRequest {
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
    };
    let request_id = request.request_id;
    let mut enabling = pool.begin().await.unwrap();
    school_academic_core::services::lifecycle_guard::lock_transition(&mut enabling)
        .await
        .unwrap();
    let close_pool = pool.clone();
    let close_actor = actor.clone();
    let closing = tokio::spawn(async move {
        school_academic_lifecycle::services::term_transitions::transition_term(
            &external,
            &close_pool,
            &close_actor,
            term,
            request,
        )
        .await
    });
    wait_for_transition_waiter(&pool).await;
    // Acceptance occurs after the competing close's BEGIN, while that close is
    // proven blocked; no sleeps or guessed scheduling order establish the race.
    let enabled:chrono::DateTime<chrono::Utc>=sqlx::query_scalar("UPDATE academic_retention_policy SET enabled=true,effective_from=clock_timestamp(),row_version=row_version+1,updated_by=$1 RETURNING effective_from")
        .bind(actor.user_id).fetch_one(&mut *enabling).await.unwrap();
    enabling.commit().await.unwrap();
    closing.await.unwrap().unwrap();
    let closed: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT created_at FROM academic_term_transition_receipts WHERE request_id=$1",
    )
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(closed > enabled);
    let workspace = school_academic_lifecycle::services::retention::get_workspace(
        &crate::adapters::academic_retention::ACADEMIC_RETENTION,
        &pool,
        &actor,
        year,
    )
    .await
    .unwrap();
    assert!(!workspace
        .terms
        .iter()
        .find(|row| row.academic_term_id == term)
        .unwrap()
        .blocked_reasons
        .contains(&"outside_policy_effective_period".into()));
}
