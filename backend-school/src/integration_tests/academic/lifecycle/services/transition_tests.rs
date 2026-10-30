use super::*;
use crate::integration_tests::academic::{
    core::{
        self,
        models::{AcademicTermStatus, TermTransitionAction, TermTransitionRequest},
        services::{
            activation_context_tests::{empty_school, ready_term_for_existing_year, ready_year},
            term_transitions,
        },
    },
    cutover_test_support::apply_migrations_through,
    lifecycle::models::{ExecutePromotionRunInput, PromotionDecisionOutcome},
};
use chrono::Duration;
use school_authorization::ActorContext;
use school_permissions::registry::codes;
use uuid::Uuid;

#[tokio::test]
async fn lifecycle_transition_rolls_back_state_and_receipt_when_audit_fails() {
    let pool = core::services_tests::prepare_core_fixture("lifecycle_atomic_audit").await;
    let (year, term): (Uuid, Uuid) =
        sqlx::query_as("SELECT academic_year_id,id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = ActorContext {
        user_id: id,
        permissions: vec![codes::WILDCARD.into()],
    };
    let request = transition_input(
        &pool,
        &actor,
        year,
        term,
        TermTransitionAction::BeginClosing,
    )
    .await;
    sqlx::raw_sql("CREATE FUNCTION fail_term_transition_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'LIFECYCLE_TEST_AUDIT_FAILURE'; END $$; CREATE TRIGGER fail_term_transition_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_term_transition_audit();").execute(&pool).await.unwrap();
    assert!(
        term_transitions::transition_term(&pool, &actor, term, request.clone())
            .await
            .is_err()
    );
    let workspace = get_workspace(&pool, &actor, year, term).await.unwrap();
    assert_eq!(workspace.context.term_status, AcademicTermStatus::Active);
    assert_eq!(
        workspace.context.term_row_version,
        request.expected_term_version
    );
    assert_eq!(
        workspace.context.year_row_version,
        request.expected_year_version
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_term_transition_receipts WHERE request_id=$1",
    )
    .bind(request.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

async fn transition_input(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
    action: TermTransitionAction,
) -> TermTransitionRequest {
    if action == TermTransitionAction::Activate {
        let workspace = get_activation_workspace(pool, actor, year, term)
            .await
            .unwrap();
        return TermTransitionRequest {
            academic_year_id: year,
            request_id: Uuid::new_v4(),
            action,
            expected_year_version: workspace.context.year_row_version,
            expected_term_version: workspace.context.term_row_version,
            readiness_checksum: workspace.source_checksum,
            acknowledged_warning_codes: vec![],
            reason: None,
        };
    }
    let workspace = get_workspace(pool, actor, year, term).await.unwrap();
    TermTransitionRequest {
        academic_year_id: year,
        request_id: Uuid::new_v4(),
        action,
        expected_year_version: workspace.context.year_row_version,
        expected_term_version: workspace.context.term_row_version,
        readiness_checksum: workspace.source_checksum,
        acknowledged_warning_codes: vec![],
        reason: None,
    }
}

#[tokio::test]
async fn lifecycle_activation_requires_closed_predecessors_and_blocks_reopening_after_successor() {
    let pool = core::services_tests::prepare_core_fixture("lifecycle_activation_guards").await;
    let (year, source): (Uuid, Uuid) =
        sqlx::query_as("SELECT academic_year_id,id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let target: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 AND sequence_no=2",
    )
    .bind(year)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
    let id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = ActorContext {
        user_id: id,
        permissions: vec![codes::WILDCARD.into()],
    };
    let request =
        transition_input(&pool, &actor, year, target, TermTransitionAction::MarkReady).await;
    let ready = term_transitions::transition_term(&pool, &actor, target, request)
        .await
        .unwrap();
    assert_eq!(ready.context.term_status, AcademicTermStatus::Ready);
    let request =
        transition_input(&pool, &actor, year, target, TermTransitionAction::Activate).await;
    assert!(
        term_transitions::transition_term(&pool, &actor, target, request)
            .await
            .is_err()
    );
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    let request =
        transition_input(&pool, &actor, year, target, TermTransitionAction::Activate).await;
    let active = term_transitions::transition_term(&pool, &actor, target, request)
        .await
        .unwrap();
    assert_eq!(active.context.term_status, AcademicTermStatus::Active);
    let mut reopen =
        transition_input(&pool, &actor, year, source, TermTransitionAction::Reopen).await;
    reopen.reason = Some("ตรวจผลแก้ไข".into());
    assert!(
        term_transitions::transition_term(&pool, &actor, source, reopen)
            .await
            .is_err()
    );
    let running: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_terms WHERE status IN ('active','closing')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(running, 1);
}

#[tokio::test]
async fn lifecycle_transition_rechecks_versions_replays_exact_receipt_and_keeps_scores_closed() {
    let pool = core::services_tests::prepare_core_fixture("lifecycle_transition_receipts").await;
    let (year, term): (Uuid, Uuid) =
        sqlx::query_as("SELECT academic_year_id,id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = ActorContext {
        user_id: id,
        permissions: vec![
            codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
            codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL.into(),
        ],
    };
    let workspace = get_workspace(&pool, &actor, year, term).await.unwrap();
    assert!(!workspace.coverage.ready);
    let request = TermTransitionRequest {
        academic_year_id: year,
        request_id: Uuid::new_v4(),
        action: TermTransitionAction::BeginClosing,
        expected_year_version: workspace.context.year_row_version,
        expected_term_version: workspace.context.term_row_version,
        readiness_checksum: workspace.source_checksum,
        acknowledged_warning_codes: vec![],
        reason: None,
    };
    let (first, concurrent) = tokio::join!(
        term_transitions::transition_term(&pool, &actor, term, request.clone()),
        term_transitions::transition_term(&pool, &actor, term, request.clone()),
    );
    let outcome = first.unwrap();
    assert_eq!(
        serde_json::to_value(&outcome).unwrap(),
        serde_json::to_value(concurrent.unwrap()).unwrap()
    );
    assert_eq!(outcome.context.term_status, AcademicTermStatus::Closing);
    assert_eq!(
        outcome.context.term_row_version,
        request.expected_term_version + 1
    );
    let replay = term_transitions::transition_term(&pool, &actor, term, request.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&outcome).unwrap(),
        serde_json::to_value(&replay).unwrap()
    );
    let mut changed = request.clone();
    changed.action = TermTransitionAction::CancelClosing;
    assert!(
        term_transitions::transition_term(&pool, &actor, term, changed)
            .await
            .is_err()
    );
    let mut stale = request.clone();
    stale.request_id = Uuid::new_v4();
    assert!(
        term_transitions::transition_term(&pool, &actor, term, stale)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_term_transition_receipts WHERE academic_term_id=$1",
    )
    .bind(term)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    assert!(
        sqlx::query("DELETE FROM academic_term_transition_receipts WHERE request_id=$1")
            .bind(request.request_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let reader = ActorContext {
        user_id: id,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    assert!(get_workspace(&pool, &reader, year, term)
        .await
        .unwrap()
        .available_actions
        .is_empty());
    assert!(matches!(
        term_transitions::transition_term(&pool, &reader, term, request.clone()).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    let teacher = ActorContext {
        user_id: id,
        permissions: vec![codes::ACADEMIC_RESULT_READ_ORGANIZATION_UNIT.into()],
    };
    assert!(matches!(
        get_workspace(&pool, &teacher, year, term).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    assert!(get_workspace(&pool, &actor, Uuid::new_v4(), term)
        .await
        .is_err());
    let latest = get_workspace(&pool, &actor, year, term).await.unwrap();
    let mut close = request;
    close.request_id = Uuid::new_v4();
    close.action = TermTransitionAction::Close;
    close.expected_term_version = latest.context.term_row_version;
    close.readiness_checksum = latest.source_checksum;
    assert!(matches!(
        term_transitions::transition_term(&pool, &actor, term, close.clone()).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    let mut closer = actor;
    closer
        .permissions
        .push(codes::ACADEMIC_LIFECYCLE_CLOSE_SCHOOL.into());
    assert!(matches!(
        term_transitions::transition_term(&pool, &closer, term, close).await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn activation_opens_promoted_enrollment_and_placement_atomically_and_replays_exactly() {
    let (pool, executor, _, _, calculation, approved) =
        super::promotion_execution::tests::approved_fixture(
            "lifecycle_atomic_activation",
            PromotionDecisionOutcome::Promote,
        )
        .await;
    apply_migrations_through(&pool, 109).await.unwrap();
    let execution = super::execute_run(
        &pool,
        &executor,
        calculation.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    let target_student_year = execution.receipts[0].target_student_year_id.unwrap();
    let target_placement = execution.receipts[0].target_placement_id.unwrap();

    sqlx::query(
        "UPDATE academic_terms SET status='closed',row_version=row_version+1
         WHERE academic_year_id=$1 AND status<>'cancelled'",
    )
    .bind(calculation.run.source_year_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_years SET status='closed',row_version=row_version+1 WHERE id=$1")
        .bind(calculation.run.source_year_id)
        .execute(&pool)
        .await
        .unwrap();
    // Place unrelated synthetic planning years after the requested successor;
    // ordering follows numeric identity without retiming placement facts.
    let target_number:i32=sqlx::query_scalar("SELECT year FROM academic_years WHERE id=$1").bind(calculation.run.target_year_id).fetch_one(&pool).await.unwrap();
    let intervening:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM academic_years WHERE year>(SELECT year FROM academic_years WHERE id=$1) AND year<$2 ORDER BY year,id").bind(calculation.run.source_year_id).bind(target_number).fetch_all(&pool).await.unwrap();
    for (index,year) in intervening.into_iter().enumerate() {
        sqlx::query("UPDATE academic_years SET year=$2 WHERE id=$1").bind(year).bind(target_number+1+index as i32).execute(&pool).await.unwrap();
    }
    let target_start = calculation.run.target_placement_starts_on;
    let term = ready_term_for_existing_year(
        &pool,
        executor.user_id,
        calculation.run.target_year_id,
        target_start,
    )
    .await;
    let actor = ActorContext {
        user_id: executor.user_id,
        permissions: vec![codes::WILDCARD.into()],
    };
    let workspace =
        get_activation_workspace(&pool, &actor, calculation.run.target_year_id, term.id)
            .await
            .unwrap();
    assert!(workspace.can_activate, "{:?}", workspace.findings);
    assert!(workspace.opens_year);
    assert_eq!(workspace.planned_students, 1);
    assert_eq!(workspace.eligible_placements, 1);
    let source_before: String = sqlx::query_scalar(
        "SELECT md5(jsonb_build_object(
           'studentYears',(SELECT jsonb_agg(to_jsonb(s) ORDER BY s.id) FROM student_academic_years s WHERE s.academic_year_id=$1),
           'placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM homeroom_placements p WHERE p.academic_year_id=$1),
           'courseResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY r.id) FROM academic_course_results r JOIN student_academic_years s ON s.id=r.student_academic_year_id WHERE s.academic_year_id=$1)
         )::text)",
    )
    .bind(calculation.run.source_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let stale_policy_request = TermTransitionRequest {
        academic_year_id: calculation.run.target_year_id,
        request_id: Uuid::new_v4(),
        action: TermTransitionAction::Activate,
        expected_year_version: workspace.context.year_row_version,
        expected_term_version: workspace.context.term_row_version,
        readiness_checksum: workspace.source_checksum.clone(),
        acknowledged_warning_codes: vec![],
        reason: None,
    };
    let policy = super::get_opening_policy(&pool, &actor).await.unwrap();
    super::update_opening_policy(
        &pool,
        &actor,
        super::super::models::UpdateOpeningPolicyInput {
            row_version: policy.row_version,
            require_homeroom_placements: !policy.require_homeroom_placements,
            require_published_offerings: policy.require_published_offerings,
            require_published_timetable: policy.require_published_timetable,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        term_transitions::transition_term(&pool, &actor, term.id, stale_policy_request.clone())
            .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let workspace =
        get_activation_workspace(&pool, &actor, calculation.run.target_year_id, term.id)
            .await
            .unwrap();
    let stale_room_request = TermTransitionRequest {
        request_id: Uuid::new_v4(),
        readiness_checksum: workspace.source_checksum,
        ..stale_policy_request
    };
    sqlx::query(
        "UPDATE homerooms SET row_version=row_version+1
         WHERE id=(SELECT homeroom_id FROM homeroom_placements WHERE id=$1)",
    )
    .bind(target_placement)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        term_transitions::transition_term(&pool, &actor, term.id, stale_room_request.clone()).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let workspace =
        get_activation_workspace(&pool, &actor, calculation.run.target_year_id, term.id)
            .await
            .unwrap();
    let request = TermTransitionRequest {
        request_id: Uuid::new_v4(),
        expected_year_version: workspace.context.year_row_version,
        expected_term_version: workspace.context.term_row_version,
        readiness_checksum: workspace.source_checksum,
        ..stale_room_request
    };
    sqlx::raw_sql(
        "CREATE FUNCTION fail_atomic_activation_audit() RETURNS trigger LANGUAGE plpgsql AS $$
           BEGIN
             IF NEW.event_code='academic.term.activate' THEN
               RAISE EXCEPTION 'ATOMIC_ACTIVATION_AUDIT_FAILURE';
             END IF;
             RETURN NEW;
           END $$;
         CREATE TRIGGER fail_atomic_activation_audit BEFORE INSERT ON academic_audit_events
         FOR EACH ROW EXECUTE FUNCTION fail_atomic_activation_audit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        term_transitions::transition_term(&pool, &actor, term.id, request.clone())
            .await
            .is_err()
    );
    sqlx::query("DROP TRIGGER fail_atomic_activation_audit ON academic_audit_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE FUNCTION fail_atomic_activation_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
           BEGIN RAISE EXCEPTION 'ATOMIC_ACTIVATION_RECEIPT_FAILURE'; END $$;
         CREATE TRIGGER fail_atomic_activation_receipt BEFORE INSERT ON academic_year_transition_receipts
         FOR EACH ROW EXECUTE FUNCTION fail_atomic_activation_receipt();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        term_transitions::transition_term(&pool, &actor, term.id, request.clone())
            .await
            .is_err()
    );
    sqlx::query("DROP TRIGGER fail_atomic_activation_receipt ON academic_year_transition_receipts")
        .execute(&pool)
        .await
        .unwrap();
    let rolled_back: (String, String, String, String, i64, i64) = sqlx::query_as(
        "SELECT year.status,term.status,student.status,placement.status,
           (SELECT count(*) FROM academic_term_transition_receipts WHERE request_id=$5),
           (SELECT count(*) FROM academic_year_transition_receipts WHERE request_id=$5)
         FROM academic_years year JOIN academic_terms term ON term.academic_year_id=year.id
         JOIN student_academic_years student ON student.id=$3
         JOIN homeroom_placements placement ON placement.id=$4
         WHERE year.id=$1 AND term.id=$2",
    )
    .bind(calculation.run.target_year_id)
    .bind(term.id)
    .bind(target_student_year)
    .bind(target_placement)
    .bind(request.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        rolled_back,
        (
            "planning".into(),
            "ready".into(),
            "planned".into(),
            "planned".into(),
            0,
            0
        )
    );
    let (first, replay) = tokio::join!(
        term_transitions::transition_term(&pool, &actor, term.id, request.clone()),
        term_transitions::transition_term(&pool, &actor, term.id, request.clone())
    );
    let outcome = first.unwrap();
    assert_eq!(
        serde_json::to_value(&outcome).unwrap(),
        serde_json::to_value(replay.unwrap()).unwrap()
    );
    assert_eq!(outcome.context.term_status, AcademicTermStatus::Active);
    let states: (String, String, String, String) = sqlx::query_as(
        "SELECT year.status,term.status,student.status,placement.status
         FROM academic_years year JOIN academic_terms term ON term.academic_year_id=year.id
         JOIN student_academic_years student ON student.id=$3
         JOIN homeroom_placements placement ON placement.id=$4
         WHERE year.id=$1 AND term.id=$2",
    )
    .bind(calculation.run.target_year_id)
    .bind(term.id)
    .bind(target_student_year)
    .bind(target_placement)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        states,
        (
            "active".into(),
            "active".into(),
            "active".into(),
            "current".into()
        )
    );
    let source_after: String = sqlx::query_scalar(
        "SELECT md5(jsonb_build_object(
           'studentYears',(SELECT jsonb_agg(to_jsonb(s) ORDER BY s.id) FROM student_academic_years s WHERE s.academic_year_id=$1),
           'placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM homeroom_placements p WHERE p.academic_year_id=$1),
           'courseResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY r.id) FROM academic_course_results r JOIN student_academic_years s ON s.id=r.student_academic_year_id WHERE s.academic_year_id=$1)
         )::text)",
    )
    .bind(calculation.run.source_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source_after, source_before);
    let receipts: (i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT count(*) FROM academic_term_transition_receipts WHERE request_id=$1),
           (SELECT count(*) FROM academic_year_transition_receipts WHERE request_id=$1)",
    )
    .bind(request.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipts, (1, 1));
    let policy = super::get_opening_policy(&pool, &actor).await.unwrap();
    super::update_opening_policy(
        &pool,
        &actor,
        super::super::models::UpdateOpeningPolicyInput {
            row_version: policy.row_version,
            require_homeroom_placements: !policy.require_homeroom_placements,
            require_published_offerings: policy.require_published_offerings,
            require_published_timetable: policy.require_published_timetable,
        },
    )
    .await
    .unwrap();
    let exact_replay = term_transitions::transition_term(&pool, &actor, term.id, request.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(exact_replay).unwrap(),
        serde_json::to_value(outcome).unwrap()
    );
    let foreign = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::WILDCARD.into()],
    };
    assert!(matches!(
        term_transitions::transition_term(&pool, &foreign, term.id, request).await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn activation_supports_a_new_school_then_a_later_term_without_reopening_enrollment() {
    let (pool, actor_id) = empty_school("lifecycle_new_school_then_term").await;
    let (year, first) = ready_year(
        &pool,
        actor_id,
        2569,
        chrono::NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(),
    )
    .await;
    let actor = ActorContext {
        user_id: actor_id,
        permissions: vec![codes::WILDCARD.into()],
    };
    let later = ready_term_for_existing_year(
        &pool,
        actor_id,
        year.id,
        chrono::Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive()
            + Duration::days(120),
    )
    .await;
    let first_workspace = get_activation_workspace(&pool, &actor, year.id, first.id)
        .await
        .unwrap();
    assert!(first_workspace.opens_year && first_workspace.can_activate);
    let first_request = TermTransitionRequest {
        academic_year_id: year.id,
        request_id: Uuid::new_v4(),
        action: TermTransitionAction::Activate,
        expected_year_version: first_workspace.context.year_row_version,
        expected_term_version: first_workspace.context.term_row_version,
        readiness_checksum: first_workspace.source_checksum,
        acknowledged_warning_codes: vec![],
        reason: None,
    };
    term_transitions::transition_term(&pool, &actor, first.id, first_request.clone())
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed',row_version=row_version+1 WHERE id=$1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    let later_workspace = get_activation_workspace(&pool, &actor, year.id, later.id)
        .await
        .unwrap();
    assert!(!later_workspace.opens_year && later_workspace.can_activate);
    let later_request = TermTransitionRequest {
        academic_year_id: year.id,
        request_id: Uuid::new_v4(),
        action: TermTransitionAction::Activate,
        expected_year_version: later_workspace.context.year_row_version,
        expected_term_version: later_workspace.context.term_row_version,
        readiness_checksum: later_workspace.source_checksum,
        acknowledged_warning_codes: vec![],
        reason: None,
    };
    let outcome = term_transitions::transition_term(&pool, &actor, later.id, later_request.clone())
        .await
        .unwrap();
    assert_eq!(outcome.context.term_status, AcademicTermStatus::Active);
    let receipts: (i64, i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT count(*) FROM academic_year_transition_receipts WHERE request_id=$1),
           (SELECT count(*) FROM academic_year_transition_receipts WHERE request_id=$2),
           (SELECT count(*) FROM academic_term_transition_receipts WHERE request_id=$2)",
    )
    .bind(first_request.request_id)
    .bind(later_request.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipts, (1, 0, 1));
}

#[tokio::test]
async fn historical_term_transition_replays_canonical_request_without_rewriting_evidence() {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct OldRequest<'a> {
        academic_year_id: Uuid,
        request_id: Uuid,
        action: TermTransitionAction,
        expected_year_version: i64,
        expected_term_version: i64,
        readiness_checksum: &'a str,
        acknowledged_warning_codes: Vec<String>,
        closed_on: Option<chrono::NaiveDate>,
        reason: Option<String>,
    }
    for action in [TermTransitionAction::Reopen,TermTransitionAction::Activate] {
        let pool=core::services_tests::prepare_core_fixture_through(&format!("lifecycle_historical_replay_{action:?}"),145).await;
        let (year,term,bell):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT academic_year_id,id,bell_schedule_id FROM academic_terms WHERE status='active'").fetch_one(&pool).await.unwrap();
        let actor=ActorContext{user_id:sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap(),permissions:vec![codes::WILDCARD.into()]};
        let request=TermTransitionRequest{academic_year_id:year,request_id:Uuid::new_v4(),action,expected_year_version:2,expected_term_version:3,readiness_checksum:"a".repeat(64),acknowledged_warning_codes:vec![],reason:if action==TermTransitionAction::Reopen{Some("แก้ไข  ผล\nวันที่จริง \"quoted\"".into())}else{None}};
        let original=OldRequest{academic_year_id:year,request_id:request.request_id,action,expected_year_version:2,expected_term_version:3,readiness_checksum:&request.readiness_checksum,acknowledged_warning_codes:vec![],closed_on:None,reason:request.reason.clone()};
        let checksum=school_academic_lifecycle::services::checksum(&(actor.user_id,term,&original)).unwrap();
        let original=serde_json::to_value(original).unwrap();
        let action_name=if action==TermTransitionAction::Activate{"activate"}else{"reopen"};
        let outcome=serde_json::json!({"requestId":request.request_id,"action":action,"completedAt":"2025-05-01T17:30:00Z","context":{"academicYearId":year,"academicTermId":term,"yearName":"Historical year","termName":"Historical term","yearStatus":"active","termStatus":if action==TermTransitionAction::Activate{"active"}else{"closing"},"yearRowVersion":3,"termRowVersion":4,"yearStartDate":"2025-05-01","yearEndDate":"2026-04-30","termStartDate":"2025-05-01","plannedEndDate":"2025-10-31","closedOn":null,"sequence":1,"bellScheduleId":bell,"includedInYearResult":true,"blocksYearClosure":true}});
        let accepted=serde_json::json!({"context":outcome["context"]});
        sqlx::query("INSERT INTO academic_term_transition_receipts(request_id,academic_year_id,academic_term_id,actor_user_id,action,request_checksum,accepted_readiness,outcome,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'2025-05-01T17:30:00Z')")
            .bind(request.request_id).bind(year).bind(term).bind(actor.user_id).bind(action_name).bind(&checksum).bind(sqlx::types::Json(&accepted)).bind(sqlx::types::Json(&outcome)).execute(&pool).await.unwrap();
        let audit=if action==TermTransitionAction::Activate{serde_json::json!({"request":original,"outcome":outcome,"openedYear":true,"activatedStudents":0,"activatedPlacements":0})}else{serde_json::json!([original,outcome])};
        sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES($1,'academic_term',$2,$3,$2,$4,$5)").bind(format!("academic.term.{action_name}")).bind(term).bind(year).bind(actor.user_id).bind(sqlx::types::Json(&audit)).execute(&pool).await.unwrap();
        if action==TermTransitionAction::Activate {
            sqlx::query("INSERT INTO academic_year_transition_receipts(request_id,academic_year_id,actor_user_id,action,request_checksum,accepted_readiness,outcome,created_at) VALUES($1,$2,$3,'activate',$4,$5,$6,'2025-05-01T17:30:00Z')")
                .bind(request.request_id).bind(year).bind(actor.user_id).bind(&checksum).bind(sqlx::types::Json(&accepted)).bind(sqlx::types::Json(&outcome)).execute(&pool).await.unwrap();
        }
        apply_migrations_through(&pool,school_migrations::latest_migration_version()).await.unwrap();
        let replay=term_transitions::transition_term(&pool,&actor,term,request.clone()).await.unwrap();
        assert_eq!(replay.context.operation_date,chrono::NaiveDate::from_ymd_opt(2025,5,2).unwrap());
        assert_eq!(replay.completed_at.to_rfc3339(),"2025-05-01T17:30:00+00:00");
        assert_eq!(replay.request_id,request.request_id);
        assert!(matches!(term_transitions::transition_term(&pool,&actor,Uuid::new_v4(),request.clone()).await,Err(school_errors::AppError::Conflict(_))));
        let mut wrong_year=request.clone();wrong_year.academic_year_id=Uuid::new_v4();
        assert!(matches!(term_transitions::transition_term(&pool,&actor,term,wrong_year).await,Err(school_errors::AppError::Conflict(_))));
        let foreign=ActorContext{user_id:Uuid::new_v4(),permissions:actor.permissions.clone()};
        assert!(matches!(term_transitions::transition_term(&pool,&foreign,term,request.clone()).await,Err(school_errors::AppError::Conflict(_))));
        let saved:(String,serde_json::Value)=sqlx::query_as("SELECT request_checksum,accepted_readiness FROM academic_term_transition_receipts WHERE request_id=$1").bind(request.request_id).fetch_one(&pool).await.unwrap();assert_eq!(saved,(checksum,accepted));
        let saved_audit:serde_json::Value=sqlx::query_scalar("SELECT payload FROM academic_audit_events WHERE event_code=$1 AND entity_id=$2").bind(format!("academic.term.{action_name}")).bind(term).fetch_one(&pool).await.unwrap();assert_eq!(saved_audit,audit);
    }
}
