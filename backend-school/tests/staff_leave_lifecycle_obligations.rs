#[path = "support/coverage_lifecycle_fixture.rs"]
mod fixture;

use chrono::{Duration, NaiveTime};
use fixture::{EmptyExternal, Fixture};
use school_academic_core::models::{AcademicTermStatus, AcademicYearStatus};
use school_academic_lifecycle::{models::*, services};
use school_academic_timetable::services::{dated_obligations, dated_teaching};
use school_errors::AppError;
use uuid::Uuid;

fn hour(h: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, 0, 0).unwrap()
}

async fn deny_term_close(f: &Fixture, request: Uuid) {
    let before = f.evidence(request).await;
    let input = f.term_input(TermTransitionAction::Close).await;
    assert!(matches!(
        services::term_transitions::transition_term(
            &EmptyExternal,
            &f.pool,
            &f.actor,
            f.term,
            input.clone()
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(before, f.evidence(request).await);
    let receipts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_term_transition_receipts WHERE request_id=$1",
    )
    .bind(input.request_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(receipts, 0);
    assert_ne!(
        f.term_workspace().await.context.term_status,
        AcademicTermStatus::Closed
    );
}

#[tokio::test]
async fn future_reserved_and_approved_teaching_blocks_term_close_without_attendance_day() {
    let f = Fixture::new("coverage_lifecycle_term_future", "active").await;
    let date = f.local_now().await.date() + Duration::days(7);
    let duty = f.duty(date, hour(9), hour(10));
    let request = f.reserve(&[duty]).await;
    f.assert_no_attendance_materialized().await;
    let ws = f.term_workspace().await;
    assert!(ws
        .findings
        .iter()
        .any(|r| r.code == "timetable.blocking" && r.count == 1 && r.resolution_url.is_none()));
    deny_term_close(&f, request).await;
    f.approve(request).await;
    deny_term_close(&f, request).await;
    // Existing begin-closing semantics still admit work that must be resolved
    // before the actual Close action.
    let input = f.term_input(TermTransitionAction::BeginClosing).await;
    let result = services::term_transitions::transition_term(
        &EmptyExternal,
        &f.pool,
        &f.actor,
        f.term,
        input,
    )
    .await
    .unwrap();
    assert_eq!(result.context.term_status, AcademicTermStatus::Closing);
    deny_term_close(&f, request).await;
}

#[tokio::test]
async fn begun_reciprocal_pair_retains_owed_return_and_blocks_term_close() {
    let f = Fixture::new("coverage_lifecycle_owed_pair", "active").await;
    let today = f.local_now().await.date();
    let pair = Uuid::now_v7();
    let mut first = f.duty(today - Duration::days(1), hour(9), hour(10));
    first.kind = "reciprocal".into();
    first.pair_id = Some(pair);
    first.pair_leg = Some(1);
    let mut back = f.duty(today + Duration::days(7), hour(9), hour(10));
    back.kind = "reciprocal".into();
    back.pair_id = Some(pair);
    back.pair_leg = Some(2);
    back.teacher_id = f.actor.user_id;
    back.original_teacher_id = f.teacher;
    let lesson = back.lesson.as_mut().unwrap();
    lesson.teacher_id = f.teacher;
    lesson.lesson_key = dated_teaching::lesson_key(lesson.date, lesson.allocation_id, f.teacher);
    let back_id = back.id;
    let request = f.reserve(&[first, back]).await;
    f.approve(request).await;
    f.release(request, true).await;
    f.assert_no_attendance_materialized().await;
    let mut connection = f.pool.acquire().await.unwrap();
    let pending = dated_obligations::pending_on_connection(&mut connection, f.year, Some(f.term))
        .await
        .unwrap();
    assert_eq!(
        pending.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![back_id]
    );
    drop(connection);
    deny_term_close(&f, request).await;
}

#[tokio::test]
async fn optional_planning_and_ready_terms_block_year_close_despite_acknowledged_warning() {
    for status in ["planning", "ready"] {
        let f = Fixture::new(&format!("coverage_lifecycle_optional_{status}"), status).await;
        let date = f.local_now().await.date() + Duration::days(14);
        let request = f.reserve(&[f.duty(date, hour(9), hour(10))]).await;
        f.approve(request).await;
        f.assert_no_attendance_materialized().await;
        let before = f.evidence(request).await;
        let ws = services::get_year_workspace(&f.pool, &f.actor, f.year)
            .await
            .unwrap();
        assert!(!ws.can_close);
        assert!(ws
            .findings
            .iter()
            .any(|r| r.code == "year.teaching_obligations"
                && r.severity == LifecycleSeverity::Blocking
                && r.resolution_url.is_none()));
        let input = f.year_input(YearTransitionAction::Close).await;
        assert_eq!(
            input.acknowledged_warning_codes,
            vec!["year.optional_terms"]
        );
        assert!(matches!(
            services::year_transitions::transition_year(&f.pool, &f.actor, f.year, input).await,
            Err(AppError::Conflict(_))
        ));
        assert_eq!(before, f.evidence(request).await);
        // Releasing this future commitment permits the same existing close path.
        f.release(request, false).await;
        let input = f.year_input(YearTransitionAction::Close).await;
        let result = services::year_transitions::transition_year(&f.pool, &f.actor, f.year, input)
            .await
            .unwrap();
        assert_eq!(result.context.status, AcademicYearStatus::Closed);
    }
}

#[tokio::test]
async fn released_future_and_fully_past_duties_allow_term_close_with_evidence_preserved() {
    let f = Fixture::new("coverage_lifecycle_resolved", "active").await;
    let today = f.local_now().await.date();
    let future = f
        .reserve(&[f.duty(today + Duration::days(7), hour(9), hour(10))])
        .await;
    f.release(future, false).await;
    let past = f
        .reserve(&[f.duty(today - Duration::days(1), hour(9), hour(10))])
        .await;
    f.approve(past).await;
    let mut absence = f.duty(today + Duration::days(14), hour(9), hour(10));
    absence.kind = "absence".into();
    absence.lesson = None;
    absence.teacher_id = f.actor.user_id;
    absence.taught_activity = None;
    let absence = f.reserve(&[absence]).await;
    let evidence = [
        f.evidence(future).await,
        f.evidence(past).await,
        f.evidence(absence).await,
    ];
    assert!(f.term_workspace().await.findings.is_empty());
    let input = f.term_input(TermTransitionAction::Close).await;
    let result = services::term_transitions::transition_term(
        &EmptyExternal,
        &f.pool,
        &f.actor,
        f.term,
        input,
    )
    .await
    .unwrap();
    assert_eq!(result.context.term_status, AcademicTermStatus::Closed);
    assert_eq!(
        evidence,
        [
            f.evidence(future).await,
            f.evidence(past).await,
            f.evidence(absence).await
        ]
    );
}

#[tokio::test]
async fn same_day_ongoing_blocks_but_actual_ended_interval_does_not() {
    let f = Fixture::new("coverage_lifecycle_intraday", "active").await;
    let now = f.local_now().await;
    let elapsed = now.time() - NaiveTime::MIN;
    // Persisted PostgreSQL times have microsecond precision. Halving an odd
    // microsecond without normalization invents a half-microsecond overlap
    // between the JSON-persisted end and the next in-memory start.
    let boundary = NaiveTime::MIN + Duration::microseconds(elapsed.num_microseconds().unwrap() / 2);
    let ended = f
        .reserve(&[f.duty(now.date(), NaiveTime::MIN, boundary)])
        .await;
    f.approve(ended).await;
    let ongoing = f.duty(
        now.date(),
        boundary,
        NaiveTime::from_hms_micro_opt(23, 59, 59, 999999).unwrap(),
    );
    let ongoing_id = ongoing.id;
    let request = f.reserve(&[ongoing]).await;
    f.approve(request).await;
    let mut connection = f.pool.acquire().await.unwrap();
    let pending = dated_obligations::pending_on_connection(&mut connection, f.year, Some(f.term))
        .await
        .unwrap();
    assert_eq!(
        pending.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![ongoing_id]
    );
    drop(connection);
    deny_term_close(&f, request).await;
    f.release(request, false).await;
    let input = f.term_input(TermTransitionAction::Close).await;
    assert!(services::term_transitions::transition_term(
        &EmptyExternal,
        &f.pool,
        &f.actor,
        f.term,
        input
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn new_duty_and_state_revision_invalidate_term_and_year_previews() {
    let f = Fixture::new("coverage_lifecycle_stale_term", "active").await;
    let input = f.term_input(TermTransitionAction::Close).await;
    let date = f.local_now().await.date() + Duration::days(7);
    let request = f.reserve(&[f.duty(date, hour(9), hour(10))]).await;
    let before = f.evidence(request).await;
    assert_ne!(
        input.readiness_checksum,
        f.term_workspace().await.source_checksum
    );
    assert!(matches!(
        services::term_transitions::transition_term(
            &EmptyExternal,
            &f.pool,
            &f.actor,
            f.term,
            input
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(before, f.evidence(request).await);
    let reserved_checksum = f.term_workspace().await.source_checksum;
    f.approve(request).await;
    assert_ne!(reserved_checksum, f.term_workspace().await.source_checksum);

    let f = Fixture::new("coverage_lifecycle_stale_year", "planning").await;
    let input = f.year_input(YearTransitionAction::Close).await;
    let request = f.reserve(&[f.duty(date, hour(9), hour(10))]).await;
    let before = f.evidence(request).await;
    let reserved_checksum = services::get_year_workspace(&f.pool, &f.actor, f.year)
        .await
        .unwrap()
        .source_checksum;
    assert_ne!(input.readiness_checksum, reserved_checksum);
    assert!(matches!(
        services::year_transitions::transition_year(&f.pool, &f.actor, f.year, input).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(before, f.evidence(request).await);
    f.approve(request).await;
    assert_ne!(
        reserved_checksum,
        services::get_year_workspace(&f.pool, &f.actor, f.year)
            .await
            .unwrap()
            .source_checksum
    );
}

#[tokio::test]
async fn pending_provider_keeps_every_obligation_across_bounded_pages() {
    let f = Fixture::new("coverage_lifecycle_paged", "active").await;
    let today = f.local_now().await.date();
    let duties: Vec<_> = (1..=257)
        .map(|day| f.duty(today + Duration::days(day), hour(9), hour(10)))
        .collect();
    f.reserve(&duties).await;
    let mut connection = f.pool.acquire().await.unwrap();
    let pending = dated_obligations::pending_on_connection(&mut connection, f.year, Some(f.term))
        .await
        .unwrap();
    let mut expected: Vec<_> = duties.iter().map(|d| d.id).collect();
    expected.sort();
    assert_eq!(pending.iter().map(|p| p.id).collect::<Vec<_>>(), expected);
    assert!(pending
        .iter()
        .all(|p| p.blocks_closure && p.revision.len() == 64));
}
