use super::*;
use crate::{
    middleware::permission::ActorContext,
    modules::academic::{
        core::{
            self,
            models::{AcademicYearStatus, YearTransitionAction, YearTransitionRequest},
            services::year_transitions,
        },
        cutover_test_support::apply_migrations_through,
    },
    permissions::registry::codes,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str) -> (PgPool, ActorContext, Uuid) {
    let pool = core::services_tests::prepare_core_fixture(name).await;
    apply_migrations_through(&pool, 70).await.unwrap();
    let year = sqlx::query_scalar("SELECT id FROM academic_years WHERE status='active'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let id = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    (
        pool,
        ActorContext {
            user_id: id,
            permissions: vec![codes::WILDCARD.into()],
        },
        year,
    )
}

async fn input(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    action: YearTransitionAction,
) -> YearTransitionRequest {
    let ws = get_year_workspace(pool, actor, year).await.unwrap();
    YearTransitionRequest {
        request_id: Uuid::new_v4(),
        action,
        expected_year_version: ws.context.row_version,
        readiness_checksum: ws.source_checksum,
        acknowledged_warning_codes: vec![],
    }
}

#[tokio::test]
async fn year_lifecycle_transition_is_actor_bound_and_does_not_close_missing_results() {
    let (pool, actor, year) = fixture("year_transition_replay").await;
    let request = input(&pool, &actor, year, YearTransitionAction::BeginClosing).await;
    let original = year_transitions::transition_year(&pool, &actor, year, request.clone())
        .await
        .unwrap();
    assert_eq!(original.context.status, AcademicYearStatus::Closing);
    assert_eq!(
        original.context.row_version,
        request.expected_year_version + 1
    );
    let replay = year_transitions::transition_year(&pool, &actor, year, request.clone())
        .await
        .unwrap();
    assert_eq!(replay.completed_at, original.completed_at);
    assert_eq!(replay.context.row_version, original.context.row_version);
    let mut changed = request.clone();
    changed.action = YearTransitionAction::CancelClosing;
    assert!(matches!(
        year_transitions::transition_year(&pool, &actor, year, changed).await,
        Err(crate::error::AppError::Conflict(_))
    ));
    let other = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: actor.permissions.clone(),
    };
    assert!(matches!(
        year_transitions::transition_year(&pool, &other, year, request.clone()).await,
        Err(crate::error::AppError::Conflict(_))
    ));
    let reader = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    assert!(matches!(
        year_transitions::transition_year(&pool, &reader, year, request).await,
        Err(crate::error::AppError::Forbidden(_))
    ));
    let close = input(&pool, &actor, year, YearTransitionAction::Close).await;
    assert!(matches!(
        year_transitions::transition_year(&pool, &actor, year, close).await,
        Err(crate::error::AppError::Conflict(_))
    ));
    let cancel = input(&pool, &actor, year, YearTransitionAction::CancelClosing).await;
    let outcome = year_transitions::transition_year(&pool, &actor, year, cancel)
        .await
        .unwrap();
    assert_eq!(outcome.context.status, AcademicYearStatus::Active);
    for kind in 0..4 {
        let mut invalid = input(&pool, &actor, year, YearTransitionAction::BeginClosing).await;
        match kind {
            0 => invalid.request_id = Uuid::nil(),
            1 => invalid.expected_year_version = 0,
            2 => invalid.readiness_checksum = "invalid".into(),
            _ => invalid.acknowledged_warning_codes = vec!["year.optional_terms".into()],
        }
        assert!(matches!(
            year_transitions::transition_year(&pool, &actor, year, invalid).await,
            Err(crate::error::AppError::ValidationError(_))
        ));
    }
    assert!(
        sqlx::query("UPDATE academic_year_transition_receipts SET action='close'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM academic_year_transition_receipts")
        .execute(&pool)
        .await
        .is_err());
}

#[tokio::test]
async fn year_lifecycle_transition_rolls_back_on_audit_failure_and_rejects_stale_source() {
    let (pool, actor, year) = fixture("year_transition_atomic").await;
    let mut request = input(&pool, &actor, year, YearTransitionAction::BeginClosing).await;
    request.readiness_checksum = "0".repeat(64);
    assert!(matches!(
        year_transitions::transition_year(&pool, &actor, year, request).await,
        Err(crate::error::AppError::Conflict(_))
    ));
    let request = input(&pool, &actor, year, YearTransitionAction::BeginClosing).await;
    sqlx::raw_sql("CREATE FUNCTION fail_year_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'YEAR_TEST_AUDIT_FAILURE'; END $$; CREATE TRIGGER fail_year_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_year_audit();").execute(&pool).await.unwrap();
    assert!(matches!(
        year_transitions::transition_year(&pool, &actor, year, request.clone()).await,
        Err(crate::error::AppError::DbError(_))
    ));
    let ws = get_year_workspace(&pool, &actor, year).await.unwrap();
    assert_eq!(ws.context.status, AcademicYearStatus::Active);
    assert_eq!(ws.context.row_version, request.expected_year_version);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_year_transition_receipts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn year_lifecycle_parallel_requests_accept_only_one_revision() {
    let (pool, actor, year) = fixture("year_transition_concurrent").await;
    let first = input(&pool, &actor, year, YearTransitionAction::BeginClosing).await;
    let mut second = first.clone();
    second.request_id = Uuid::new_v4();
    let (a, b) = tokio::join!(
        year_transitions::transition_year(&pool, &actor, year, first),
        year_transitions::transition_year(&pool, &actor, year, second)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let failed = if a.is_err() { a } else { b };
    assert!(matches!(failed, Err(crate::error::AppError::Conflict(_))));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_year_transition_receipts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
