use crate::modules::academic::{
    cutover_test_support::apply_migrations_through, results::services_tests::fixture,
};
use school_academic_results::{models::*, services::*};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use uuid::Uuid;

#[tokio::test]
async fn annual_revision_preserves_zero_replays_and_invalidates_after_source_correction() {
    let (pool, actor, context, student) =
        crate::modules::academic::results::aggregate_revision_tests::ready_aggregate_fixture(
            "annual_revision_history",
        )
        .await;
    apply_migrations_through(&pool, 69).await.unwrap();
    sqlx::query(
        "UPDATE academic_terms SET included_in_year_result=(id=$2) WHERE academic_year_id=$1",
    )
    .bind(context.academic_year_id)
    .bind(context.academic_term_id)
    .execute(&pool)
    .await
    .unwrap();
    let policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "Reviewed annual source".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        },
    )
    .await
    .unwrap();
    let query = AggregatePreviewQuery {
        academic_year_id: context.academic_year_id,
        academic_term_id: context.academic_term_id,
        policy_id: policy.id,
    };
    let term_preview = preview_aggregate(&pool, &actor, student, &query)
        .await
        .unwrap();
    let term = lock_term_aggregate(
        &pool,
        &actor,
        &context,
        student,
        AggregateLockInput {
            policy_id: policy.id,
            source_checksum: term_preview.source_checksum.clone(),
            expected_revision: None,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        },
    )
    .await
    .unwrap();
    let preview = preview_annual(&pool, &actor, context.academic_year_id, student)
        .await
        .unwrap();
    assert!(preview.can_lock && !preview.needs_hold);
    assert_eq!(preview.terms.len(), 1);
    assert_eq!(preview.terms[0].revision.as_ref().unwrap().id, term.id);
    assert_eq!(preview.totals.provisional_gpa.as_deref(), Some("0.00"));
    let input = AnnualLockInput {
        expected_revision: None,
        source_checksum: preview.source_checksum,
        request_id: Uuid::new_v4(),
        hold_reason: None,
    };
    let first = lock_annual(
        &pool,
        &actor,
        context.academic_year_id,
        student,
        input.clone(),
    )
    .await
    .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.official_gpa.as_deref(), Some("0.00"));
    assert!(first.is_current);
    let mut dependency_tx = pool.begin().await.unwrap();
    assert!(matches!(
        require_term_without_annual_results(
            &mut dependency_tx,
            context.academic_year_id,
            context.academic_term_id
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(require_term_without_annual_results(
        &mut dependency_tx,
        context.academic_year_id,
        Uuid::new_v4()
    )
    .await
    .is_ok());
    dependency_tx.rollback().await.unwrap();
    let retry = lock_annual(
        &pool,
        &actor,
        context.academic_year_id,
        student,
        input.clone(),
    )
    .await
    .unwrap();
    assert_eq!(retry.id, first.id);
    let count:i64 = sqlx::query_scalar("SELECT count(*) FROM academic_annual_result_term_sources WHERE annual_revision_id=$1 AND term_aggregate_revision_id=$2").bind(first.id).bind(term.id).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    let mut changed = input.clone();
    changed.expected_revision = Some(1);
    assert!(matches!(
        lock_annual(&pool, &actor, context.academic_year_id, student, changed).await,
        Err(AppError::Conflict(_))
    ));
    let reader = ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
        ],
    };
    assert!(matches!(
        lock_annual(&pool, &reader, context.academic_year_id, student, input).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(
        sqlx::query("DELETE FROM academic_annual_result_revisions WHERE id=$1")
            .bind(first.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query(
        "DELETE FROM academic_annual_result_term_sources WHERE annual_revision_id=$1"
    )
    .bind(first.id)
    .execute(&pool)
    .await
    .is_err());
    let course = &term_preview.results.courses[0];
    correct_result(
        &pool,
        &actor,
        &context,
        ResultCorrectionInput::Course {
            course_result_id: course.result_id.unwrap(),
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let stale = preview_annual(&pool, &reader, context.academic_year_id, student)
        .await
        .unwrap();
    assert!(!stale.can_lock && !stale.terms[0].is_current);
    let history = list_annual_revisions(&pool, &reader, context.academic_year_id, student)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert!(!history[0].is_current);
    assert_eq!(history[0].official_gpa.as_deref(), Some("0.00"));
    correct_result(
        &pool,
        &actor,
        &context,
        ResultCorrectionInput::Course {
            course_result_id: course.result_id.unwrap(),
            outcome: CourseOfficialOutcome::Incomplete,
            numeric_grade: None,
            expected_effective_version: 2,
        },
    )
    .await
    .unwrap();
    let held_policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "Reviewed holds".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: true,
        },
    )
    .await
    .unwrap();
    let held_preview = preview_aggregate(
        &pool,
        &actor,
        student,
        &AggregatePreviewQuery {
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            policy_id: held_policy.id,
        },
    )
    .await
    .unwrap();
    assert!(held_preview.can_lock && !held_preview.hold_findings.is_empty());
    lock_term_aggregate(
        &pool,
        &actor,
        &context,
        student,
        AggregateLockInput {
            policy_id: held_policy.id,
            source_checksum: held_preview.source_checksum,
            expected_revision: Some(1),
            request_id: Uuid::new_v4(),
            hold_reason: Some("Reviewed incomplete source".into()),
        },
    )
    .await
    .unwrap();
    let annual = preview_annual(&pool, &actor, context.academic_year_id, student)
        .await
        .unwrap();
    assert!(annual.can_lock && annual.needs_hold);
    assert_eq!(annual.totals.exceptional_result_count, 1);
    let mut held_input = AnnualLockInput {
        expected_revision: Some(1),
        source_checksum: annual.source_checksum,
        request_id: Uuid::new_v4(),
        hold_reason: None,
    };
    assert!(matches!(
        lock_annual(
            &pool,
            &actor,
            context.academic_year_id,
            student,
            held_input.clone()
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    held_input.hold_reason = Some("Reviewed annual hold".into());
    let held = lock_annual(&pool, &actor, context.academic_year_id, student, held_input)
        .await
        .unwrap();
    assert_eq!(held.revision, 2);
    assert!(held.official_gpa.is_none() && held.is_current);
    let history = list_annual_revisions(&pool, &reader, context.academic_year_id, student)
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert!(history[0].is_current && !history[1].is_current);
    assert_eq!(history[1].official_gpa.as_deref(), Some("0.00"));
    let mut coverage_tx = pool.begin().await.unwrap();
    let coverage = annual_closure_coverage(&mut coverage_tx, context.academic_year_id)
        .await
        .unwrap();
    let covered = coverage
        .students
        .iter()
        .find(|row| row.student_academic_year_id == student)
        .unwrap();
    assert_eq!(covered.revision_id, Some(held.id));
    assert!(covered.is_current && covered.hold_reason.is_some());
    coverage_tx.rollback().await.unwrap();
    // The public Core command must enforce the owner guard too, not only
    // its direct helper. Other terms are still unstarted in this fixture.
    sqlx::query("UPDATE academic_terms SET status=CASE WHEN id=$2 THEN 'closed' ELSE 'cancelled' END,closed_on=CASE WHEN id=$2 THEN start_date ELSE NULL END WHERE academic_year_id=$1")
            .bind(context.academic_year_id).bind(context.academic_term_id).execute(&pool).await.unwrap();
    let mut lifecycle_actor = actor.clone();
    lifecycle_actor.permissions.extend([
        codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
        codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL.into(),
    ]);
    let workspace = crate::modules::academic::lifecycle::services::get_workspace(
        &pool,
        &lifecycle_actor,
        context.academic_year_id,
        context.academic_term_id,
    )
    .await
    .unwrap();
    let reopen = crate::modules::academic::lifecycle::models::TermTransitionRequest {
        academic_year_id: context.academic_year_id,
        request_id: Uuid::new_v4(),
        action: crate::modules::academic::lifecycle::models::TermTransitionAction::Reopen,
        expected_year_version: workspace.context.year_row_version,
        expected_term_version: workspace.context.term_row_version,
        readiness_checksum: workspace.source_checksum,
        acknowledged_warning_codes: vec![],
        closed_on: None,
        reason: Some("Requested source review".into()),
    };
    assert!(matches!(
        crate::modules::academic::lifecycle::services::term_transitions::transition_term(
            &pool,
            &lifecycle_actor,
            context.academic_term_id,
            reopen
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn annual_preview_requires_current_applicable_term_revisions_and_exact_student_context() {
    let (pool, actor, context, group) = fixture("annual_preview_missing").await;
    apply_migrations_through(&pool, 69).await.unwrap();
    let student:Uuid=sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active' ORDER BY student_academic_year_id LIMIT 1").bind(group).fetch_one(&pool).await.unwrap();
    let office = ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
        ],
    };
    let preview = preview_annual(&pool, &office, context.academic_year_id, student)
        .await
        .unwrap();
    assert!(!preview.terms.is_empty());
    assert!(!preview.can_lock);
    assert!(preview
        .terms
        .iter()
        .any(|term| term.academic_term_id == context.academic_term_id
            && term.revision.is_none()
            && !term.is_current));
    assert!(preview.totals.provisional_gpa.is_none());
    assert!(!preview.totals.coverage_complete);
    let mut coverage_tx = pool.begin().await.unwrap();
    let coverage = annual_closure_coverage(&mut coverage_tx, context.academic_year_id)
        .await
        .unwrap();
    assert!(!coverage.ready);
    assert!(coverage
        .students
        .iter()
        .any(|row| row.student_academic_year_id == student
            && row.revision_id.is_none()
            && !row.is_current));
    assert!(annual_closure_coverage(&mut coverage_tx, Uuid::new_v4())
        .await
        .is_err());
    coverage_tx.rollback().await.unwrap();
    assert!(preview_annual(&pool, &office, Uuid::new_v4(), student)
        .await
        .is_err());
    assert!(
        preview_annual(&pool, &office, context.academic_year_id, Uuid::new_v4())
            .await
            .is_err()
    );
    let teacher = ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_RESULT_READ_ASSIGNED.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED.into(),
        ],
    };
    assert!(matches!(
        preview_annual(&pool, &teacher, context.academic_year_id, student).await,
        Err(AppError::Forbidden(_))
    ));
    // Only applicable included, non-cancelled terms can become annual sources.
    sqlx::query(
        "UPDATE academic_terms SET included_in_year_result=false WHERE academic_year_id=$1",
    )
    .bind(context.academic_year_id)
    .execute(&pool)
    .await
    .unwrap();
    let excluded = preview_annual(&pool, &office, context.academic_year_id, student)
        .await
        .unwrap();
    assert!(excluded.terms.is_empty() && !excluded.can_lock);
}

#[tokio::test]
async fn annual_preview_ignores_nonparticipant_summer_but_requires_participant_results() {
    let (pool, actor, context, group) = fixture("annual_preview_summer").await;
    apply_migrations_through(&pool, 69).await.unwrap();
    let student:Uuid=sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active' LIMIT 1").bind(group).fetch_one(&pool).await.unwrap();
    let office = ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
        ],
    };
    sqlx::query("UPDATE academic_terms SET included_in_year_result=(id=$2),term_type='summer' WHERE academic_year_id=$1").bind(context.academic_year_id).bind(context.academic_term_id).execute(&pool).await.unwrap();
    let empty:Uuid=sqlx::query_scalar("INSERT INTO academic_terms(academic_year_id,sequence_no,code,name,term_type,start_date,bell_schedule_id,status,included_in_year_result,blocks_year_closure) SELECT academic_year_id,(SELECT max(sequence_no)+1 FROM academic_terms WHERE academic_year_id=$1),'empty-summer','Empty summer','summer',start_date,bell_schedule_id,'planning',true,true FROM academic_terms WHERE id=$2 RETURNING id").bind(context.academic_year_id).bind(context.academic_term_id).fetch_one(&pool).await.unwrap();
    let preview = preview_annual(&pool, &office, context.academic_year_id, student)
        .await
        .unwrap();
    assert_eq!(preview.terms.len(), 1);
    assert_eq!(preview.terms[0].academic_term_id, context.academic_term_id);
    assert!(preview
        .terms
        .iter()
        .all(|term| term.academic_term_id != empty));
    assert!(!preview.can_lock);
    sqlx::query("UPDATE academic_terms SET status='cancelled' WHERE id=$1")
        .bind(context.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
    let cancelled = preview_annual(&pool, &office, context.academic_year_id, student)
        .await
        .unwrap();
    assert!(cancelled.terms.is_empty() && !cancelled.can_lock);
}
