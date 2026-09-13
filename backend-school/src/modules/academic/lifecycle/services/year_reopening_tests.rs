use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::{core, cutover_test_support::apply_migrations_through},
    permissions::registry::codes,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str) -> (PgPool, ActorContext, Uuid) {
    let pool = core::services_tests::prepare_core_fixture(name).await;
    apply_migrations_through(&pool, 76).await.unwrap();
    let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE status='active'")
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
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    (pool, actor, year)
}

pub(super) async fn set_closed(pool: &PgPool, year: Uuid) {
    sqlx::query("UPDATE academic_terms SET status='closed',row_version=row_version+1 WHERE academic_year_id=$1").bind(year).execute(pool).await.unwrap();
    sqlx::query("UPDATE academic_years SET status='closed',row_version=row_version+1 WHERE id=$1")
        .bind(year)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn year_reopening_reader_checks_state_context_and_exact_permissions() {
    let (pool, reader, year) = fixture("year_reopening_read").await;
    let active = super::get_year_reopening_workspace(&pool, &reader, year)
        .await
        .unwrap();
    assert!(!active.can_reopen);
    assert!(active
        .findings
        .iter()
        .any(|finding| finding.code == "year.reopen_state"));
    set_closed(&pool, year).await;
    let closed = super::get_year_reopening_workspace(&pool, &reader, year)
        .await
        .unwrap();
    assert!(closed.can_reopen);
    assert!(closed.findings.is_empty());
    let office = ActorContext {
        user_id: reader.user_id,
        permissions: vec![codes::WILDCARD.into()],
    };
    assert_eq!(
        closed.source_checksum,
        super::get_year_reopening_workspace(&pool, &office, year)
            .await
            .unwrap()
            .source_checksum
    );
    for permissions in [
        vec![],
        vec![codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL.into()],
        vec![codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL.into()],
    ] {
        let denied = ActorContext {
            user_id: reader.user_id,
            permissions,
        };
        assert!(matches!(
            super::get_year_reopening_workspace(&pool, &denied, year).await,
            Err(AppError::Forbidden(_))
        ));
    }
    assert!(matches!(
        super::get_year_reopening_workspace(&pool, &reader, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn year_reopening_reader_detects_successors_even_after_they_close() {
    let (pool, reader, year) = fixture("year_reopening_successor").await;
    set_closed(&pool, year).await;
    let original = super::get_year_reopening_workspace(&pool, &reader, year)
        .await
        .unwrap();
    assert!(original.can_reopen);
    let target: Uuid = sqlx::query_scalar("INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) SELECT max(year)+1,'E2E-LIFECYCLE-reopen-successor',max(end_date)+1,max(end_date)+366,'MON','planning' FROM academic_years RETURNING id").fetch_one(&pool).await.unwrap();
    assert!(
        super::get_year_reopening_workspace(&pool, &reader, year)
            .await
            .unwrap()
            .can_reopen
    );
    for status in ["active", "closing", "closed", "archived"] {
        sqlx::query("UPDATE academic_years SET status=$2,row_version=row_version+1 WHERE id=$1")
            .bind(target)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        let blocked = super::get_year_reopening_workspace(&pool, &reader, year)
            .await
            .unwrap();
        assert!(!blocked.can_reopen, "successor status {status}");
        assert!(blocked
            .findings
            .iter()
            .any(|finding| finding.code == "year.reopen_successor"));
        assert_ne!(blocked.source_checksum, original.source_checksum);
    }
}

fn reopener(user_id: Uuid) -> ActorContext {
    ActorContext {
        user_id,
        permissions: vec![
            codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
            codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL.into(),
            codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL.into(),
        ],
    }
}

#[tokio::test]
async fn year_reopening_command_rechecks_a_successor_started_after_preview() {
    let (pool, reader, year) = fixture("year_reopening_late_successor").await;
    set_closed(&pool, year).await;
    let actor = reopener(reader.user_id);
    let input = reopen_input(&pool, &actor, year).await;
    sqlx::query("INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) SELECT max(year)+1,'E2E-LIFECYCLE-late-successor',max(end_date)+1,max(end_date)+366,'MON','active' FROM academic_years").execute(&pool).await.unwrap();
    assert!(matches!(
        core::services::year_reopening::reopen_year(&pool, &actor, year, input).await,
        Err(AppError::Conflict(_))
    ));
    let state = super::get_year_reopening_workspace(&pool, &actor, year)
        .await
        .unwrap();
    assert_eq!(
        state.context.status,
        core::models::AcademicYearStatus::Closed
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_year_transition_receipts WHERE academic_year_id=$1",
    )
    .bind(year)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

async fn reopen_input(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
) -> core::models::YearReopeningRequest {
    let workspace = super::get_year_reopening_workspace(pool, actor, year)
        .await
        .unwrap();
    core::models::YearReopeningRequest {
        request_id: Uuid::new_v4(),
        expected_year_version: workspace.context.row_version,
        source_checksum: workspace.source_checksum,
        reason: "ตรวจทานการตั้งค่าปีก่อนยืนยันปิดใหม่".into(),
    }
}

async fn historical_hash(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT md5(jsonb_build_object(
        'students',(SELECT jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text) FROM student_academic_years r),
        'placements',(SELECT jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text) FROM homeroom_placements r),
        'courseResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text) FROM academic_course_results r),
        'activityResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text) FROM academic_activity_results r),
        'evaluationLocks',(SELECT jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text) FROM subject_term_evaluation_locks r),
        'annual',(SELECT jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text) FROM academic_annual_result_revisions r),
        'scores',(SELECT jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text) FROM learning_group_student_scores r),
        'controls',(SELECT jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text) FROM academic_gradebook_phase_controls r)
    )::text)").fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn year_reopening_command_preserves_locked_history_and_replays_exact_actor_and_action() {
    let (pool, owner, _, context, _) =
        super::promotion_run_review::tests::ready_run("year_reopening_command").await;
    let year = context.academic_year_id;
    set_closed(&pool, year).await;
    let actor = reopener(owner.user_id);
    let input = reopen_input(&pool, &actor, year).await;
    let before = historical_hash(&pool).await;
    for permissions in [
        vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
        vec![codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL.into()],
        vec![
            codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
            codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL.into(),
            codes::ACADEMIC_LIFECYCLE_CLOSE_SCHOOL.into(),
        ],
    ] {
        let denied = ActorContext {
            user_id: actor.user_id,
            permissions,
        };
        assert!(matches!(
            core::services::year_reopening::reopen_year(&pool, &denied, year, input.clone()).await,
            Err(AppError::Forbidden(_))
        ));
    }
    let outcome = core::services::year_reopening::reopen_year(&pool, &actor, year, input.clone())
        .await
        .unwrap();
    assert_eq!(
        outcome.context.status,
        core::models::AcademicYearStatus::Closing
    );
    assert_eq!(outcome.context.row_version, input.expected_year_version + 1);
    assert_eq!(historical_hash(&pool).await, before);
    let replay = core::services::year_reopening::reopen_year(&pool, &actor, year, input.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&outcome).unwrap(),
        serde_json::to_value(replay).unwrap()
    );
    let mut changed = input.clone();
    changed.reason = "ตรวจทานอีกเหตุผลหนึ่ง".into();
    assert!(matches!(
        core::services::year_reopening::reopen_year(&pool, &actor, year, changed).await,
        Err(AppError::Conflict(_))
    ));
    let other = reopener(Uuid::new_v4());
    assert!(matches!(
        core::services::year_reopening::reopen_year(&pool, &other, year, input.clone()).await,
        Err(AppError::Conflict(_))
    ));
    let closed_workspace = super::get_year_workspace(&pool, &actor, year)
        .await
        .unwrap();
    let collision = core::models::YearTransitionRequest {
        request_id: input.request_id,
        action: core::models::YearTransitionAction::CancelClosing,
        expected_year_version: outcome.context.row_version,
        readiness_checksum: closed_workspace.source_checksum,
        acknowledged_warning_codes: vec![],
    };
    assert!(
        matches!(
            core::services::year_transitions::transition_year(&pool, &actor, year, collision).await,
            Err(AppError::Conflict(_))
        ),
        "cross-action requests must conflict before decoding their different outcome shapes"
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_year_transition_receipts WHERE request_id=$1",
    )
    .bind(input.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    assert_eq!(historical_hash(&pool).await, before);
}

#[tokio::test]
async fn year_reopening_command_validates_reason_and_rolls_back_audit_before_retry() {
    let (pool, reader, year) = fixture("year_reopening_rollback").await;
    set_closed(&pool, year).await;
    let actor = reopener(reader.user_id);
    let input = reopen_input(&pool, &actor, year).await;
    for reason in [
        "".into(),
        "   ".into(),
        "ท".repeat(1001),
        "1234567890123".into(),
        "๑-๒๓๔๕-๖๗๘๙๐-๑๒-๓".into(),
    ] {
        let mut bad = input.clone();
        bad.reason = reason;
        assert!(matches!(
            core::services::year_reopening::reopen_year(&pool, &actor, year, bad).await,
            Err(AppError::ValidationError(_))
        ));
    }
    let mut bad = input.clone();
    bad.expected_year_version = 0;
    assert!(matches!(
        core::services::year_reopening::reopen_year(&pool, &actor, year, bad).await,
        Err(AppError::ValidationError(_))
    ));
    let mut bad = input.clone();
    bad.source_checksum = "z".repeat(64);
    assert!(matches!(
        core::services::year_reopening::reopen_year(&pool, &actor, year, bad).await,
        Err(AppError::ValidationError(_))
    ));
    let mut bad = input.clone();
    bad.request_id = Uuid::nil();
    assert!(matches!(
        core::services::year_reopening::reopen_year(&pool, &actor, year, bad).await,
        Err(AppError::ValidationError(_))
    ));
    sqlx::raw_sql("CREATE FUNCTION fail_year_reopen_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_code='academic.year.reopen' THEN RAISE EXCEPTION 'fixture audit unavailable'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_year_reopen_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_year_reopen_audit();").execute(&pool).await.unwrap();
    assert!(matches!(
        core::services::year_reopening::reopen_year(&pool, &actor, year, input.clone()).await,
        Err(AppError::DbError(_))
    ));
    let after = super::get_year_reopening_workspace(&pool, &actor, year)
        .await
        .unwrap();
    assert_eq!(
        after.context.status,
        core::models::AcademicYearStatus::Closed
    );
    assert_eq!(after.context.row_version, input.expected_year_version);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_year_transition_receipts WHERE request_id=$1",
    )
    .bind(input.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::raw_sql("DROP TRIGGER fail_year_reopen_audit ON academic_audit_events; DROP FUNCTION fail_year_reopen_audit();").execute(&pool).await.unwrap();
    let mut concurrent = input.clone();
    concurrent.request_id = Uuid::new_v4();
    let (first, second) = tokio::join!(
        core::services::year_reopening::reopen_year(&pool, &actor, year, input),
        core::services::year_reopening::reopen_year(&pool, &actor, year, concurrent)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(
        matches!(first, Err(AppError::Conflict(_))) || matches!(second, Err(AppError::Conflict(_)))
    );
}
