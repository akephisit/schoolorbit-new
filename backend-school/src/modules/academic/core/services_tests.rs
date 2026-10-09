use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use chrono::{NaiveDate, NaiveTime};
use school_academic_core::models::{
    AcademicTermStatus, AcademicTermType, AcademicYearStatus, BellSchedulePeriodInput,
    CatalogDisplayState, CopyStudyProgramRequest, CreateAcademicTermRequest,
    CreateActivityVersionRequest, CreateBellScheduleRequest, CreateCatalogActivityRequest,
    CreateCatalogSubjectRequest, CreateCurriculumLevelRequest, CreateCurriculumRequest,
    CreateHomeroomPlacementRequest, CreateHomeroomRequest, CreateStudentAcademicYearRequest,
    CreateStudyProgramRequest, CreateSubjectGroupRequest, CreateSubjectVersionRequest,
    CurriculumStructureRequirementInput, CurriculumTermSlotInput, HomeroomPlacementStatus,
    PublishCurriculumRequest, PublishVersionRequest, ReplaceBellSchedulePeriodsRequest,
    ReplaceCurriculumStructureRequest, ReplaceCurriculumTermSlotsRequest,
    ReplaceGradeProgressionsRequest, RequirementKind, RequirementResourceKind,
    StudentAcademicYearFilter, StudentYearCandidateQuery, TransferHomeroomPlacementRequest,
    UpdateAcademicTermRequest, UpdateAcademicYearRequest, UpdateActivityVersionRequest,
    UpdateCatalogActivityRequest, UpdateCatalogSubjectRequest, UpdateStudyProgramRequest,
    UpdateSubjectGroupRequest, UpdateSubjectVersionRequest, VersionStatus,
};
use school_academic_core::services::{
    bell_schedules, catalog, context, curriculum, curriculum_structure, ensure_draft_version,
    ensure_planning_delete, parse_row_version, progressions, student_years,
    validate_canonical_decimal, validate_date_containment, workspaces, years_terms,
};
use school_authorization::AcademicResourceListFilter;
use school_authorization::ActorContext;
use school_http::AppErrorHttpExt;
use school_permissions::registry::codes;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const CURRENT_YEAR_ID: Uuid = Uuid::from_u128(0x1000_0000_0000_0000_0000_0000_0000_0025);
const FUTURE_YEAR_ID: Uuid = Uuid::from_u128(0x1000_0000_0000_0000_0000_0000_0000_0026);
const DEFAULT_SUBJECT_GROUP_ID: Uuid = Uuid::from_u128(0x783a_4a9d_9ff1_4eac_b370_06b5_8daa_1eb7);

pub(crate) async fn prepare_core_fixture(name: &str) -> PgPool {
    prepare_core_fixture_through(name, 97).await
}

pub(crate) async fn prepare_current_core_fixture(name: &str) -> PgPool {
    prepare_core_fixture_through(name, 97).await
}

async fn prepare_core_fixture_through(name: &str, version: i64) -> PgPool {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 3).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, version).await.unwrap();
    pool
}

async fn resource_draft_id(pool: &PgPool, id: Uuid) -> Uuid {
    let token:Option<Uuid>=sqlx::query_scalar("SELECT draft_id FROM curriculum_editions WHERE id=$1 UNION ALL SELECT e.draft_id FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.id=$1 UNION ALL SELECT e.draft_id FROM study_programs p JOIN curriculum_levels l ON l.id=p.curriculum_level_id JOIN curriculum_editions e ON e.id=l.edition_id WHERE p.id=$1 LIMIT 1")
        .bind(id).fetch_one(pool).await.unwrap();
    // Published resources intentionally supply an invalid token to denial tests.
    token.unwrap_or_else(Uuid::nil)
}

async fn publish_level_fixture(
    pool: &PgPool,
    level_id: Uuid,
    _level_request: PublishVersionRequest,
) -> Result<school_academic_core::models::CurriculumEdition, school_errors::AppError> {
    let level = curriculum::get_level(pool, level_id).await?;
    let edition = curriculum::get(pool, level.edition_id).await?;
    curriculum::publish(
        pool,
        edition.id,
        PublishCurriculumRequest {
            draft_id: edition.draft_id.unwrap_or_else(Uuid::nil),
            change_note: "เผยแพร่ข้อมูลทดสอบ".into(),
            row_version: edition.row_version,
        },
        fixture_actor(pool).await,
    )
    .await
}

async fn fixture_actor(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users WHERE user_type = 'staff' ORDER BY id LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn promotion_policy_repeat_progression_accepts_the_same_existing_grade() {
    let pool = prepare_current_core_fixture("promotion_repeat_progression").await;
    let actor = fixture_actor(&pool).await;
    let grade: Uuid = sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let result = progressions::replace(
        &pool,
        actor,
        ReplaceGradeProgressionsRequest {
            row_version: 1,
            progressions: vec![school_academic_core::models::GradeProgressionInput {
                from_grade_level_id: grade,
                to_grade_level_id: Some(grade),
                transition_kind: school_academic_core::models::GradeProgressionKind::Repeat,
                curriculum_level_id: None,
                is_active: true,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(result.row_version, 2);
    assert_eq!(result.progressions.len(), 1);
    assert_eq!(result.progressions[0].from_grade_level_id, grade);
    assert_eq!(result.progressions[0].to_grade_level_id, Some(grade));
}

async fn deactivation_lifecycle_fixture(
    name: &str,
    historical_status: &str,
    future: bool,
) -> (PgPool, Uuid, Uuid, Option<Uuid>) {
    let pool = prepare_core_fixture_through(name, 96).await;
    let (student, grade, program): (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT student_id,grade_level_id,study_program_id FROM student_academic_years WHERE academic_year_id=$1 AND status='active' ORDER BY id LIMIT 1",
    ).bind(CURRENT_YEAR_ID).fetch_one(&pool).await.unwrap();
    let historical_year = Uuid::new_v4();
    let future_year = future.then(Uuid::new_v4);
    for (year, is_future) in
        std::iter::once((historical_year, false)).chain(future_year.map(|year| (year, true)))
    {
        sqlx::query("INSERT INTO academic_years (id,year,name,start_date,end_date,school_days,status) VALUES ($1,$2,'Deactivation test year',CURRENT_DATE+$3,CURRENT_DATE+$4,'MON,TUE,WED,THU,FRI',$5)")
            .bind(year).bind(if is_future {2598} else {2597}).bind(if is_future {365i32} else {-730i32}).bind(if is_future {730i32} else {-365i32}).bind(if is_future {"planning"} else {historical_status}).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_year_grade_levels (academic_year_id,grade_level_id) VALUES ($1,$2)").bind(year).bind(grade).execute(&pool).await.unwrap();
        let homeroom = Uuid::new_v4();
        sqlx::query("INSERT INTO homerooms (id,code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity) VALUES ($1,$2,'Deactivation room',$3,$4,'1',$5,40)")
            .bind(homeroom).bind(format!("DEACT-{}", year.simple())).bind(year).bind(grade).bind(program).execute(&pool).await.unwrap();
        let student_year = Uuid::new_v4();
        sqlx::query("INSERT INTO student_academic_years (id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(student_year).bind(student).bind(year).bind(grade).bind(program).bind(if is_future {"planned"} else {"active"}).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO homeroom_placements (id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) SELECT $1,$2,$3,$4,start_date,$5,'test' FROM academic_years WHERE id=$3")
            .bind(Uuid::new_v4()).bind(student_year).bind(year).bind(homeroom).bind(if is_future {"planned"} else {"current"}).execute(&pool).await.unwrap();
    }
    (pool, student, historical_year, future_year)
}

async fn student_year_history(pool: &PgPool, student: Uuid, year: Uuid) -> serde_json::Value {
    sqlx::query_scalar("SELECT jsonb_build_object('studentYear',to_jsonb(student_year),'placements',(SELECT jsonb_agg(to_jsonb(placement) ORDER BY placement.id) FROM homeroom_placements placement WHERE placement.student_academic_year_id=student_year.id)) FROM student_academic_years student_year WHERE student_id=$1 AND academic_year_id=$2")
        .bind(student).bind(year).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn lifecycle_student_deactivation_preserves_closed_and_archived_year_history() {
    for status in ["closed", "archived"] {
        let (pool, student, history_year, _) = deactivation_lifecycle_fixture(
            &format!("deactivation_history_{status}"),
            status,
            false,
        )
        .await;
        let before = student_year_history(&pool, student, history_year).await;
        let current_version: i64 = sqlx::query_scalar("SELECT row_version FROM student_academic_years WHERE student_id=$1 AND academic_year_id=$2")
            .bind(student).bind(CURRENT_YEAR_ID).fetch_one(&pool).await.unwrap();
        let actor = fixture_actor(&pool).await;
        school_students::services::delete_student(&pool, student, actor)
            .await
            .unwrap();
        assert_eq!(
            student_year_history(&pool, student, history_year).await,
            before,
            "deactivating an account must not rewrite {status} academic history"
        );
        let state: (String, String, i64) = sqlx::query_as("SELECT users.status,student_year.status,student_year.row_version FROM users JOIN student_academic_years student_year ON student_year.student_id=users.id WHERE users.id=$1 AND student_year.academic_year_id=$2")
            .bind(student).bind(CURRENT_YEAR_ID).fetch_one(&pool).await.unwrap();
        assert_eq!(state.0, "inactive");
        assert_eq!(state.1, "withdrawn");
        assert_eq!(
            state.2,
            current_version + 1,
            "withdrawal must invalidate optimistic academic versions"
        );
    }
}

#[tokio::test]
async fn lifecycle_student_deactivation_ends_future_placements_without_invalid_dates() {
    let (pool, student, history_year, future_year) =
        deactivation_lifecycle_fixture("deactivation_future", "closed", true).await;
    let before = student_year_history(&pool, student, history_year).await;
    let actor = fixture_actor(&pool).await;
    school_students::services::delete_student(&pool, student, actor)
        .await
        .expect("a future placement must not prevent account deactivation");
    assert_eq!(
        student_year_history(&pool, student, history_year).await,
        before
    );
    let state: (String, String, bool, i64, i64) = sqlx::query_as("SELECT student_year.status,placement.status,placement.end_date>=placement.start_date,student_year.row_version,placement.row_version FROM student_academic_years student_year JOIN homeroom_placements placement ON placement.student_academic_year_id=student_year.id WHERE student_year.student_id=$1 AND student_year.academic_year_id=$2")
        .bind(student).bind(future_year.unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(
        (state.0.as_str(), state.1.as_str(), state.2),
        ("withdrawn", "ended", true)
    );
    assert!(state.3 > 1 && state.4 > 1);
}

#[tokio::test]
async fn lifecycle_student_deactivation_coordinates_before_user_locks() {
    let (pool, student, _, future) =
        deactivation_lifecycle_fixture("deactivation_lock_order", "closed", true).await;
    let actor = fixture_actor(&pool).await;
    let mut source = pool.begin().await.unwrap();
    let source_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *source)
        .await
        .unwrap();
    school_academic_core::services::lifecycle_guard::require_year_write_exclusive(
        &mut source,
        future.unwrap(),
    )
    .await
    .unwrap();
    let worker_pool = pool.clone();
    let worker = tokio::spawn(async move {
        school_students::services::delete_student(&worker_pool, student, actor).await
    });
    let mut waiting = false;
    for _ in 0..200 {
        waiting = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
        )
        .bind(source_pid)
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let mut probe = pool.begin().await.unwrap();
    let user_free = sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(student)
        .execute(&mut *probe)
        .await
        .is_ok();
    probe.rollback().await.unwrap();
    source.commit().await.unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), worker)
        .await
        .unwrap()
        .unwrap();
    assert!(
        waiting && user_free,
        "cross-year coordination must precede the user lock"
    );
    result.unwrap();
}

#[tokio::test]
async fn lifecycle_student_deactivation_rolls_back_account_and_academic_records_on_audit_failure() {
    let (pool, student, historical, future) =
        deactivation_lifecycle_fixture("deactivation_atomic", "closed", true).await;
    let actor = fixture_actor(&pool).await;
    let years = [CURRENT_YEAR_ID, historical, future.unwrap()];
    let mut before = Vec::new();
    for year in years {
        before.push(student_year_history(&pool, student, year).await);
    }
    let user_before: (String, String) =
        sqlx::query_as("SELECT username,status FROM users WHERE id=$1")
            .bind(student)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_deactivation_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'DEACTIVATION_TEST_AUDIT_FAILURE'; END $$; CREATE TRIGGER fail_deactivation_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_deactivation_audit();").execute(&pool).await.unwrap();
    assert!(
        school_students::services::delete_student(&pool, student, actor)
            .await
            .is_err()
    );
    let user_after: (String, String) =
        sqlx::query_as("SELECT username,status FROM users WHERE id=$1")
            .bind(student)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(user_after, user_before);
    for (index, year) in years.into_iter().enumerate() {
        assert_eq!(
            student_year_history(&pool, student, year).await,
            before[index]
        );
    }
    sqlx::query("DROP TRIGGER fail_deactivation_audit ON academic_audit_events")
        .execute(&pool)
        .await
        .unwrap();
    school_students::services::delete_student(&pool, student, actor)
        .await
        .unwrap();
    let audits: Vec<(Uuid, serde_json::Value)> = sqlx::query_as("SELECT actor_user_id,payload FROM academic_audit_events WHERE event_code='student_academic_year.withdrawn_for_deactivation' AND entity_id=$1")
        .bind(student).fetch_all(&pool).await.unwrap();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].0, actor);
    let changed_ids: Vec<Uuid> =
        serde_json::from_value(audits[0].1["studentAcademicYearIds"].clone()).unwrap();
    let changed_years: Vec<Uuid> =
        sqlx::query_scalar("SELECT academic_year_id FROM student_academic_years WHERE id=ANY($1)")
            .bind(changed_ids)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(changed_years.contains(&CURRENT_YEAR_ID));
    assert!(changed_years.contains(&future.unwrap()));
    assert!(!changed_years.contains(&historical));
}

#[tokio::test]
async fn lifecycle_year_advisor_replacement_rejects_closed_years_and_retains_history() {
    use school_academic_core::models::ReplaceHomeroomAdvisorsRequest;
    let pool = prepare_current_core_fixture("lifecycle_year_advisors").await;
    let homeroom: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let room = student_years::get_homeroom(&pool, homeroom).await.unwrap();
    let before =
        serde_json::to_value(student_years::list_advisors(&pool, homeroom).await.unwrap()).unwrap();
    for status in ["closed", "archived"] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(CURRENT_YEAR_ID)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        let result = student_years::replace_advisors(
            &pool,
            homeroom,
            ReplaceHomeroomAdvisorsRequest {
                advisors: vec![],
                row_version: room.row_version,
            },
        )
        .await;
        assert!(
            matches!(result, Err(school_errors::AppError::Conflict(_))),
            "{result:?}"
        );
        assert_eq!(
            student_years::get_homeroom(&pool, homeroom)
                .await
                .unwrap()
                .row_version,
            room.row_version
        );
        assert_eq!(
            serde_json::to_value(student_years::list_advisors(&pool, homeroom).await.unwrap())
                .unwrap(),
            before
        );
    }
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(CURRENT_YEAR_ID)
        .execute(&pool)
        .await
        .unwrap();
    let mut boundary = pool.begin().await.unwrap();
    let boundary_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *boundary)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM academic_years WHERE id=$1 FOR UPDATE")
        .bind(CURRENT_YEAR_ID)
        .execute(&mut *boundary)
        .await
        .unwrap();
    let worker_pool = pool.clone();
    let worker = tokio::spawn(async move {
        student_years::replace_advisors(
            &worker_pool,
            homeroom,
            ReplaceHomeroomAdvisorsRequest {
                advisors: vec![],
                row_version: room.row_version,
            },
        )
        .await
    });
    let mut waiting = false;
    for _ in 0..200 {
        waiting = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))",
        )
        .bind(boundary_pid)
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let mut probe = pool.begin().await.unwrap();
    let homeroom_free = sqlx::query("SELECT id FROM homerooms WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(homeroom)
        .execute(&mut *probe)
        .await
        .is_ok();
    probe.rollback().await.unwrap();
    boundary.commit().await.unwrap();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), worker)
        .await
        .unwrap()
        .unwrap();
    assert!(
        waiting && homeroom_free,
        "year must lock before the homeroom"
    );
    assert!(outcome.is_ok(), "active year remains writable: {outcome:?}");
}

#[tokio::test]
async fn lifecycle_guard_rejects_closed_contexts_without_an_admin_override() {
    use school_academic_core::services::lifecycle_guard::require_term_write;
    let pool = prepare_current_core_fixture("lifecycle_guard_states").await;
    let term: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 AND status='active'",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    for year_status in [
        "planning", "ready", "active", "closing", "closed", "archived",
    ] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(CURRENT_YEAR_ID)
            .bind(year_status)
            .execute(&pool)
            .await
            .unwrap();
        for term_status in [
            "planning",
            "ready",
            "active",
            "closing",
            "closed",
            "cancelled",
        ] {
            sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN start_date ELSE NULL END WHERE id=$1")
                .bind(term).bind(term_status).execute(&pool).await.unwrap();
            let mut tx = pool.begin().await.unwrap();
            let outcome = require_term_write(&mut tx, CURRENT_YEAR_ID, term).await;
            let expected = !matches!(year_status, "closed" | "archived")
                && !matches!(term_status, "closed" | "cancelled");
            assert_eq!(
                outcome.is_ok(),
                expected,
                "{year_status}/{term_status}: {outcome:?}"
            );
            tx.rollback().await.unwrap();
        }
    }
    let mut tx = pool.begin().await.unwrap();
    assert!(require_term_write(&mut tx, FUTURE_YEAR_ID, term)
        .await
        .is_err());
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn lifecycle_guard_serializes_source_writes_and_transitions_in_both_orders() {
    use school_academic_core::services::lifecycle_guard::{
        lock_transition, require_term_write, TRANSITION_KEY, TRANSITION_NAMESPACE,
    };
    let pool =
        school_test_db::create_named_test_pool_with_max_connections("lifecycle_guard_locking", 2)
            .await;
    crate::modules::academic::cutover_test_support::seed_release_two_predecessor(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 60).await.unwrap();
    let term: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 AND status='active'",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut source = pool.begin().await.unwrap();
    require_term_write(&mut source, CURRENT_YEAR_ID, term)
        .await
        .unwrap();
    let mut transition = pool.begin().await.unwrap();
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1,$2)")
        .bind(TRANSITION_NAMESPACE)
        .bind(TRANSITION_KEY)
        .fetch_one(&mut *transition)
        .await
        .unwrap();
    assert!(
        !acquired,
        "A transition must wait for the complete source transaction"
    );
    transition.rollback().await.unwrap();
    let mut direct = pool.begin().await.unwrap();
    let error = sqlx::query("SELECT id FROM academic_terms WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(term)
        .execute(&mut *direct)
        .await
        .unwrap_err();
    assert_eq!(
        error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("55P03")
    );
    direct.rollback().await.unwrap();
    source.commit().await.unwrap();

    let mut transition = pool.begin().await.unwrap();
    lock_transition(&mut transition).await.unwrap();
    let mut next_source = pool.begin().await.unwrap();
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock_shared($1,$2)")
        .bind(TRANSITION_NAMESPACE)
        .bind(TRANSITION_KEY)
        .fetch_one(&mut *next_source)
        .await
        .unwrap();
    assert!(
        !acquired,
        "New source writes must wait for transition commit"
    );
    next_source.rollback().await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=start_date,row_version=row_version+1 WHERE id=$1")
        .bind(term).execute(&mut *transition).await.unwrap();
    transition.commit().await.unwrap();
    let mut source = pool.begin().await.unwrap();
    assert!(require_term_write(&mut source, CURRENT_YEAR_ID, term)
        .await
        .is_err());
    source.rollback().await.unwrap();
}

#[tokio::test]
async fn lifecycle_exclusive_term_writers_serialize_without_shared_lock_upgrades() {
    use school_academic_core::services::lifecycle_guard::{
        require_term_write, require_term_write_exclusive,
    };
    let pool =
        school_test_db::create_named_test_pool_with_max_connections("lifecycle_exclusive_term", 2)
            .await;
    crate::modules::academic::cutover_test_support::seed_release_two_predecessor(&pool)
        .await
        .unwrap();
    let term: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 AND status='active'",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    for first_exclusive in [false, true] {
        let mut first = pool.begin().await.unwrap();
        if first_exclusive {
            require_term_write_exclusive(&mut first, CURRENT_YEAR_ID, term)
                .await
                .unwrap();
        } else {
            require_term_write(&mut first, CURRENT_YEAR_ID, term)
                .await
                .unwrap();
        }
        let mut second = pool.begin().await.unwrap();
        sqlx::query("SET LOCAL lock_timeout = '200ms'")
            .execute(&mut *second)
            .await
            .unwrap();
        let error = require_term_write_exclusive(&mut second, CURRENT_YEAR_ID, term)
            .await
            .unwrap_err();
        let school_errors::AppError::DbError(error) = error else {
            panic!("expected lock contention");
        };
        assert_eq!(
            error
                .as_database_error()
                .and_then(|error| error.code())
                .as_deref(),
            Some("55P03")
        );
        second.rollback().await.unwrap();
        first.commit().await.unwrap();
        let mut next = pool.begin().await.unwrap();
        require_term_write_exclusive(&mut next, CURRENT_YEAR_ID, term)
            .await
            .unwrap();
        // The exclusive entry point must really hold the write mode, not a
        // shared lock that is upgraded later by a domain mutation.
        let mut probe = pool.begin().await.unwrap();
        assert!(
            sqlx::query("SELECT id FROM academic_terms WHERE id=$1 FOR SHARE NOWAIT")
                .bind(term)
                .execute(&mut *probe)
                .await
                .is_err()
        );
        probe.rollback().await.unwrap();
        next.commit().await.unwrap();
    }
}

async fn create_published_program_option_fixture(
    pool: &PgPool,
    _owner_id: Uuid,
    _legacy_start_academic_year_id: Uuid,
    _legacy_end_academic_year_id: Option<Uuid>,
    code: &str,
) -> (Uuid, Uuid) {
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(pool)
            .await
            .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE status = 'published' ORDER BY id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        pool,
        CreateCurriculumRequest {
            name: format!("หลักสูตรตัวเลือก {code}"),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_level(
        pool,
        curriculum_row.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(pool, curriculum_row.id).await,
            name_th: format!("ฉบับ {code}"),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        pool,
        version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(pool, version.id).await,
            name_th: format!("แผนการเรียน {code}"),

            is_default: true,
        },
    )
    .await
    .unwrap();
    let mut workspace = curriculum_structure::get_workspace(pool, version.id)
        .await
        .unwrap();
    if workspace.term_slots.is_empty() {
        workspace = curriculum_structure::replace_term_slots(
            pool,
            version.id,
            ReplaceCurriculumTermSlotsRequest {
                draft_id: resource_draft_id(pool, version.id).await,
                slots: vec![CurriculumTermSlotInput {
                    id: None,
                    sequence: 1,
                    term_type: AcademicTermType::Regular,
                    type_occurrence: 1,
                    name: "ภาคเรียนที่ 1".to_string(),
                }],
                row_version: workspace.row_version,
            },
        )
        .await
        .unwrap();
    }
    let term_slot_id = workspace.term_slots[0].id;
    curriculum_structure::replace_program_structure(
        pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(pool, program.id).await,
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject_version_id,
                grade_level_id,
                term_slot_id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();
    publish_level_fixture(
        pool,
        version.id,
        PublishVersionRequest {
            row_version: workspace.row_version,
        },
    )
    .await
    .unwrap();
    (curriculum_row.id, program.id)
}

async fn create_curriculum_overview_fixture(
    pool: &PgPool,
    _owner_id: Uuid,
    grade_level_id: Uuid,
    subject_version_id: Uuid,
    code: &str,
    _legacy_start_academic_year_id: Uuid,
    _legacy_end_academic_year_id: Option<Uuid>,
    publish: bool,
    program_count: usize,
) -> (Uuid, Uuid) {
    let curriculum_row = curriculum::create(
        pool,
        CreateCurriculumRequest {
            name: format!("หลักสูตร {code}"),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_level(
        pool,
        curriculum_row.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(pool, curriculum_row.id).await,
            name_th: format!("ฉบับ {code}"),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(pool)
    .await
    .unwrap();
    let mut workspace = curriculum_structure::get_workspace(pool, version.id)
        .await
        .unwrap();
    if workspace.term_slots.is_empty() {
        workspace = curriculum_structure::replace_term_slots(
            pool,
            version.id,
            ReplaceCurriculumTermSlotsRequest {
                draft_id: resource_draft_id(pool, version.id).await,
                slots: vec![CurriculumTermSlotInput {
                    id: None,
                    sequence: 1,
                    term_type: AcademicTermType::Regular,
                    type_occurrence: 1,
                    name: "ภาคเรียนที่ 1".to_string(),
                }],
                row_version: workspace.row_version,
            },
        )
        .await
        .unwrap();
    }
    let term_slot_id = workspace.term_slots[0].id;
    for index in 0..program_count {
        let program = curriculum::create_program(
            pool,
            version.id,
            CreateStudyProgramRequest {
                draft_id: resource_draft_id(pool, version.id).await,
                name_th: format!("แผน {}", index + 1),

                is_default: index == 0,
            },
        )
        .await
        .unwrap();
        curriculum_structure::replace_program_structure(
            pool,
            program.id,
            ReplaceCurriculumStructureRequest {
                draft_id: resource_draft_id(pool, program.id).await,
                requirements: vec![CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Course,
                    catalog_version_id: subject_version_id,
                    grade_level_id,
                    term_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 1,
                }],
                row_version: program.row_version,
            },
        )
        .await
        .unwrap();
    }
    if publish {
        publish_level_fixture(
            pool,
            version.id,
            PublishVersionRequest {
                row_version: workspace.row_version,
            },
        )
        .await
        .unwrap();
    }
    (curriculum_row.id, version.id)
}

#[tokio::test]
async fn curriculum_publication_validates_all_levels_before_changing_any_plan() {
    let pool = prepare_current_core_fixture("curriculum_edition_atomic_publish").await;
    let grade: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE is_active ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject: Uuid = sqlx::query_scalar("SELECT id FROM subject_versions WHERE status='published' AND periods_per_week > 0 AND hours_per_semester > 0 ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();
    let (edition_id, first_level_id) = create_curriculum_overview_fixture(
        &pool,
        Uuid::nil(),
        grade,
        subject,
        "ATOMIC",
        CURRENT_YEAR_ID,
        None,
        false,
        1,
    )
    .await;
    let second = curriculum::create_level(
        &pool,
        edition_id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, edition_id).await,
            name_th: "ระดับเพิ่มเติม".into(),
            grade_level_ids: vec![grade],
            description: None,
        },
    )
    .await
    .unwrap();
    let before = curriculum_structure::get_workspace(&pool, first_level_id)
        .await
        .unwrap();
    let edition = curriculum::get(&pool, edition_id).await.unwrap();
    assert!(matches!(
        curriculum::publish(
            &pool,
            edition_id,
            PublishCurriculumRequest {
                draft_id: edition.draft_id.unwrap_or_else(Uuid::nil),
                change_note: "เผยแพร่ข้อมูลทดสอบ".into(),
                row_version: edition.row_version
            },
            fixture_actor(&pool).await
        )
        .await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    let after = curriculum_structure::get_workspace(&pool, first_level_id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(&after).unwrap(),
        "a failure in another level must not partially publish the valid level"
    );
    assert_eq!(
        curriculum::get(&pool, edition_id).await.unwrap().status,
        VersionStatus::Draft
    );
    let program = curriculum::create_program(
        &pool,
        second.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, second.id).await,
            name_th: "แผนเพิ่มเติม".into(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    let second_workspace = curriculum_structure::get_workspace(&pool, second.id)
        .await
        .unwrap();
    curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, program.id).await,
            row_version: program.row_version,
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject,
                grade_level_id: grade,
                term_slot_id: second_workspace.term_slots[0].id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
        },
    )
    .await
    .unwrap();
    let published = curriculum::publish(
        &pool,
        edition_id,
        PublishCurriculumRequest {
            draft_id: edition.draft_id.unwrap_or_else(Uuid::nil),
            change_note: "เผยแพร่ข้อมูลทดสอบ".into(),
            row_version: edition.row_version,
        },
        fixture_actor(&pool).await,
    )
    .await
    .unwrap();
    assert_eq!(published.status, VersionStatus::Published);
    for id in [first_level_id, second.id] {
        let workspace = curriculum_structure::get_workspace(&pool, id)
            .await
            .unwrap();
        assert_eq!(workspace.level.status, VersionStatus::Published);
        assert!(workspace
            .programs
            .iter()
            .all(|p| p.status == VersionStatus::Published));
    }
    assert!(matches!(
        curriculum::publish(
            &pool,
            edition_id,
            PublishCurriculumRequest {
                draft_id: edition.draft_id.unwrap_or_else(Uuid::nil),
                change_note: "เผยแพร่ข้อมูลทดสอบ".into(),
                row_version: published.row_version
            },
            fixture_actor(&pool).await
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn published_program_copy_preserves_selected_courses_activities_and_source() {
    let pool = prepare_current_core_fixture("academic_core_curriculum_clone_draft").await;
    let owner_id: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let grade_level_id: Uuid = sqlx::query_scalar(
        r#"SELECT grade.value::uuid
           FROM activity_versions version
           CROSS JOIN LATERAL jsonb_array_elements_text(
               COALESCE(version.grade_level_ids, '[]'::jsonb)
           ) grade(value)
           WHERE version.status = 'published'
           ORDER BY grade.value LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .expect("fixture must include a published activity with a supported grade");
    let subject_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE status = 'published' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let activity_version_id: Uuid = sqlx::query_scalar(
        r#"SELECT version.id
           FROM activity_versions version
           WHERE version.status = 'published'
             AND EXISTS (
                 SELECT 1
                 FROM jsonb_array_elements_text(
                     COALESCE(version.grade_level_ids, '[]'::jsonb)
                 ) grade(value)
                 WHERE grade.value::uuid = $1
             )
           ORDER BY version.id LIMIT 1"#,
    )
    .bind(grade_level_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (_curriculum_id, source_version_id) = create_curriculum_overview_fixture(
        &pool,
        owner_id,
        grade_level_id,
        subject_version_id,
        "CLONE-SOURCE",
        CURRENT_YEAR_ID,
        None,
        false,
        2,
    )
    .await;
    let source_before_activity = curriculum_structure::get_workspace(&pool, source_version_id)
        .await
        .unwrap();
    let first_program = source_before_activity.programs[0].clone();
    let first_slot_id = source_before_activity.term_slots[0].id;
    let mut slot_inputs = source_before_activity
        .term_slots
        .iter()
        .map(|slot| CurriculumTermSlotInput {
            id: Some(slot.id),
            sequence: slot.sequence,
            term_type: slot.term_type,
            type_occurrence: slot.type_occurrence,
            name: slot.name.clone(),
        })
        .collect::<Vec<_>>();
    slot_inputs.push(CurriculumTermSlotInput {
        id: None,
        sequence: 3,
        term_type: AcademicTermType::Regular,
        type_occurrence: 3,
        name: "ภาคเรียนเพิ่มเติม".into(),
    });
    let source_slots = curriculum_structure::replace_term_slots(
        &pool,
        source_version_id,
        ReplaceCurriculumTermSlotsRequest {
            draft_id: resource_draft_id(&pool, source_version_id).await,
            row_version: source_before_activity.row_version,
            slots: slot_inputs,
        },
    )
    .await
    .unwrap();
    let activity_slot_id = source_slots
        .term_slots
        .iter()
        .find(|slot| slot.type_occurrence == 3)
        .unwrap()
        .id;
    curriculum_structure::replace_program_structure(
        &pool,
        first_program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, first_program.id).await,
            requirements: vec![
                CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Course,
                    catalog_version_id: subject_version_id,
                    grade_level_id,
                    term_slot_id: first_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 1,
                },
                CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Activity,
                    catalog_version_id: activity_version_id,
                    grade_level_id,
                    term_slot_id: activity_slot_id,
                    requirement_kind: RequirementKind::Optional,
                    display_order: 2,
                },
            ],
            row_version: first_program.row_version,
        },
    )
    .await
    .unwrap();
    let _published = publish_level_fixture(
        &pool,
        source_version_id,
        PublishVersionRequest {
            row_version: source_before_activity.row_version,
        },
    )
    .await
    .unwrap();
    let source_workspace = curriculum_structure::get_workspace(&pool, source_version_id)
        .await
        .unwrap();
    assert_eq!(source_workspace.programs.len(), 2);
    assert_eq!(source_workspace.requirements.len(), 3);
    assert!(source_workspace
        .requirements
        .iter()
        .any(|requirement| requirement.resource_kind == RequirementResourceKind::Activity));

    let destination = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "ฉบับปรับปรุง พุทธศักราช 2570".into(),
            revision_year: 2570,
            description: None,
        },
    )
    .await
    .unwrap();
    let target = curriculum::create_level(
        &pool,
        destination.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, destination.id).await,
            name_th: "มัธยมศึกษาตอนต้น".into(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let source = curriculum::get_program(&pool, first_program.id)
        .await
        .unwrap();
    let other_grade: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active AND id<>$1 ORDER BY id LIMIT 1",
    )
    .bind(grade_level_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let wrong_level = curriculum::create_level(
        &pool,
        destination.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, destination.id).await,
            name_th: "ชั้นที่ไม่ครอบคลุมต้นทาง".into(),
            grade_level_ids: vec![other_grade],
            description: None,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        curriculum::copy_program(
            &pool,
            wrong_level.id,
            CopyStudyProgramRequest {
                draft_id: resource_draft_id(&pool, wrong_level.id).await,
                source_program_id: source.id,
                source_row_version: source.row_version,
                destination_row_version: wrong_level.row_version,
                name_th: None
            }
        )
        .await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    assert!(curriculum::list_programs(&pool, wrong_level.id)
        .await
        .unwrap()
        .is_empty());
    let request = CopyStudyProgramRequest {
        draft_id: resource_draft_id(&pool, target.id).await,
        source_program_id: source.id,
        source_row_version: source.row_version,
        destination_row_version: target.row_version,
        name_th: None,
    };
    let mut stale = request.clone();
    stale.source_row_version += 1;
    assert!(matches!(
        curriculum::copy_program(&pool, target.id, stale).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let copied = curriculum::copy_program(&pool, target.id, request.clone())
        .await
        .unwrap();
    assert_ne!(copied.id, source.id);
    assert_eq!(copied.status, VersionStatus::Draft);
    assert_eq!(copied.name_th, source.name_th);
    let copied_workspace = curriculum_structure::get_workspace(&pool, target.id)
        .await
        .unwrap();
    assert_eq!(copied_workspace.programs.len(), 1);
    assert_eq!(copied_workspace.requirements.len(), 2);
    let shape = |w: &school_academic_core::models::CurriculumStructureWorkspace, pid: Uuid| {
        w.requirements
            .iter()
            .filter(|r| r.study_program_id == pid)
            .map(|r| {
                (
                    r.resource_kind,
                    r.catalog_version_id,
                    r.grade_level.id,
                    r.requirement_kind,
                    r.display_order,
                    w.term_slots
                        .iter()
                        .find(|slot| slot.id == r.term_slot_id)
                        .map(|slot| (slot.term_type, slot.type_occurrence)),
                    serde_json::to_value(&r.metrics).unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        shape(&source_workspace, source.id),
        shape(&copied_workspace, copied.id)
    );
    assert!(copied_workspace
        .requirements
        .iter()
        .all(|r| !source_workspace
            .requirements
            .iter()
            .any(|old| old.id == r.id)));
    assert_eq!(
        copied_workspace.term_slots.len(),
        3,
        "copy must add a missing semantic term slot once"
    );
    assert!(
        matches!(
            curriculum::copy_program(&pool, target.id, request).await,
            Err(school_errors::AppError::Conflict(_))
        ),
        "stale retry must not duplicate the plan"
    );
    assert!(matches!(
        curriculum::copy_program(
            &pool,
            target.id,
            CopyStudyProgramRequest {
                draft_id: resource_draft_id(&pool, target.id).await,
                source_program_id: copied.id,
                source_row_version: copied.row_version,
                destination_row_version: copied_workspace.row_version,
                name_th: None
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let source_after = curriculum_structure::get_workspace(&pool, source_version_id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&source_after.requirements).unwrap(),
        serde_json::to_value(&source_workspace.requirements).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&source_after.programs).unwrap(),
        serde_json::to_value(&source_workspace.programs).unwrap()
    );
    let requirements = copied_workspace
        .requirements
        .iter()
        .map(|r| CurriculumStructureRequirementInput {
            resource_kind: r.resource_kind,
            catalog_version_id: r.catalog_version_id,
            grade_level_id: r.grade_level.id,
            term_slot_id: r.term_slot_id,
            requirement_kind: r.requirement_kind,
            display_order: r.display_order,
        })
        .collect();
    curriculum_structure::replace_program_structure(
        &pool,
        copied.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, copied.id).await,
            requirements,
            row_version: copied.row_version,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn curriculum_structure_workspace_reads_catalog_metrics_and_dynamic_term_slots() {
    let pool = prepare_current_core_fixture("academic_core_structure_workspace").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        r#"SELECT id
           FROM subject_versions
           WHERE status = 'published'
             AND periods_per_week IS NOT NULL
             AND hours_per_semester IS NOT NULL
           ORDER BY version_no DESC, id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2)
           ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรโครงสร้าง".to_string(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_level(
        &pool,
        curriculum_row.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, curriculum_row.id).await,
            name_th: "ฉบับโครงสร้าง".to_string(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            name_th: "แผนทั่วไป".to_string(),

            is_default: true,
        },
    )
    .await
    .unwrap();

    let empty_workspace = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    let first_regular_slot = empty_workspace
        .term_slots
        .iter()
        .find(|slot| slot.term_type == AcademicTermType::Regular && slot.type_occurrence == 1)
        .expect("the start academic year must seed its first regular curriculum slot");

    let workspace = curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, program.id).await,
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject_version_id,
                grade_level_id,
                term_slot_id: first_regular_slot.id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();

    assert!(workspace.term_slots.len() >= 2);
    assert_eq!(workspace.programs.len(), 1);
    assert_eq!(workspace.requirements.len(), 1);
    assert_eq!(
        workspace.requirements[0].metrics.credit.as_deref(),
        Some("1.50")
    );
    assert_eq!(
        workspace.requirements[0].metrics.total_hours.as_deref(),
        Some("60")
    );
    assert!(workspace.validation.blockers.is_empty());
}

#[tokio::test]
async fn curriculum_term_slots_are_draft_only_and_cannot_remove_a_referenced_slot() {
    let pool = prepare_current_core_fixture("academic_core_term_slot_replace").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE status = 'published' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรภาคเรียนยืดหยุ่น".to_string(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_level(
        &pool,
        curriculum_row.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, curriculum_row.id).await,
            name_th: "ฉบับภาคเรียนยืดหยุ่น".to_string(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let initial = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    let first_slot = initial.term_slots[0].clone();
    let mut slot_inputs = initial
        .term_slots
        .iter()
        .map(|slot| CurriculumTermSlotInput {
            id: Some(slot.id),
            sequence: slot.sequence,
            term_type: slot.term_type,
            type_occurrence: slot.type_occurrence,
            name: slot.name.clone(),
        })
        .collect::<Vec<_>>();
    slot_inputs.push(CurriculumTermSlotInput {
        id: None,
        sequence: slot_inputs.len() as i32 + 1,
        term_type: AcademicTermType::Custom,
        type_occurrence: 1,
        name: "ภาคเรียนโครงงาน".to_string(),
    });
    let with_custom = curriculum_structure::replace_term_slots(
        &pool,
        version.id,
        ReplaceCurriculumTermSlotsRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            slots: slot_inputs,
            row_version: initial.row_version,
        },
    )
    .await
    .unwrap();
    assert!(with_custom
        .term_slots
        .iter()
        .any(|slot| slot.name == "ภาคเรียนโครงงาน"));

    let program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            name_th: "แผนทั่วไป".to_string(),

            is_default: true,
        },
    )
    .await
    .unwrap();
    curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, program.id).await,
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject_version_id,
                grade_level_id,
                term_slot_id: first_slot.id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();

    let without_referenced = with_custom
        .term_slots
        .iter()
        .filter(|slot| slot.id != first_slot.id)
        .map(|slot| CurriculumTermSlotInput {
            id: Some(slot.id),
            sequence: slot.sequence,
            term_type: slot.term_type,
            type_occurrence: slot.type_occurrence,
            name: slot.name.clone(),
        })
        .collect();
    let removal = curriculum_structure::replace_term_slots(
        &pool,
        version.id,
        ReplaceCurriculumTermSlotsRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            slots: without_referenced,
            row_version: with_custom.row_version,
        },
    )
    .await;
    assert!(matches!(removal, Err(school_errors::AppError::Conflict(_))));
}

#[test]
fn academic_enums_serialize_as_snake_case() {
    assert_eq!(
        serde_json::to_value(AcademicYearStatus::Planning).unwrap(),
        json!("planning")
    );
    assert_eq!(
        serde_json::to_value(AcademicTermStatus::Cancelled).unwrap(),
        json!("cancelled")
    );
    assert_eq!(
        serde_json::to_value(AcademicTermType::Summer).unwrap(),
        json!("summer")
    );
}

#[test]
fn academic_foundation_identity_derives_standard_and_custom_labels() {
    assert_eq!(
        years_terms::derive_academic_year_name(2571, None).unwrap(),
        "ปีการศึกษา 2571"
    );
    assert_eq!(
        years_terms::derive_academic_year_name(2571, Some("ปีแห่งการอ่าน")).unwrap(),
        "ปีแห่งการอ่าน"
    );
    assert!(years_terms::derive_academic_year_name(2571, Some("   ")).is_err());

    let regular = years_terms::derive_term_identity(AcademicTermType::Regular, 2, None).unwrap();
    assert_eq!(regular.code, "2");
    assert_eq!(regular.name, "ภาคเรียนที่ 2");

    let summer =
        years_terms::derive_term_identity(AcademicTermType::Summer, 3, Some("ภาคฤดูร้อนเพิ่มเติม"))
            .unwrap();
    assert_eq!(summer.code, "SUMMER");
    assert_eq!(summer.name, "ภาคฤดูร้อนเพิ่มเติม");
}

#[test]
fn academic_foundation_period_overlap_is_scoped_to_shared_school_days() {
    let period =
        |order_index, start_hour, end_hour, days: &[&str], is_active| BellSchedulePeriodInput {
            name: Some(format!("คาบ {order_index}")),
            start_time: NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(end_hour, 0, 0).unwrap(),
            order_index,
            applicable_days: days.iter().map(|day| (*day).to_string()).collect(),
            is_active,
        };

    assert!(bell_schedules::validate_periods(&[
        period(1, 8, 10, &["MON"], true),
        period(2, 9, 11, &["TUE"], true),
    ])
    .is_ok());
    assert!(bell_schedules::validate_periods(&[
        period(1, 8, 10, &["MON"], true),
        period(2, 9, 11, &["MON"], true),
    ])
    .is_err());
    assert!(bell_schedules::validate_periods(&[
        period(1, 8, 10, &["MON"], true),
        period(2, 9, 11, &["MON"], false),
    ])
    .is_ok());
    assert!(bell_schedules::validate_periods(&[period(1, 8, 10, &["MON", "MON"], true,)]).is_err());
    assert!(bell_schedules::validate_periods(&[period(1, 8, 10, &["HOLIDAY"], true)]).is_err());
}

#[test]
fn academic_foundation_homeroom_identity_uses_grade_and_room_number() {
    let secondary = student_years::derive_homeroom_identity("secondary", 1, "3", None).unwrap();
    assert_eq!(secondary.code, "M1-3");
    assert_eq!(secondary.name, "ม.1/3");

    let primary =
        student_years::derive_homeroom_identity("primary", 2, " 1 ", Some("ห้องส่งเสริมวิทยาศาสตร์"))
            .unwrap();
    assert_eq!(primary.code, "P2-1");
    assert_eq!(primary.name, "ห้องส่งเสริมวิทยาศาสตร์");

    assert!(student_years::derive_homeroom_identity("secondary", 1, " ", None).is_err());
    assert!(student_years::derive_homeroom_identity("other", 1, "1", None).is_err());
    assert!(student_years::derive_homeroom_identity("primary", 1, "1", Some("   "),).is_err());
}

#[test]
fn request_dtos_use_camel_case_and_reject_unknown_fields() {
    let payload = json!({
        "year": 2570,
        "customName": "ปีแห่งการอ่าน",
        "startDate": "2027-05-01",
        "endDate": "2028-03-31",
        "schoolDays": ["MON", "TUE"],
        "rowVersion": 3
    });
    let request: UpdateAcademicYearRequest = serde_json::from_value(payload).unwrap();
    assert_eq!(request.row_version, 3);

    let unknown = json!({
        "name": "ปีการศึกษา 2570",
        "year": 2570,
        "startDate": "2027-05-01",
        "endDate": "2028-03-31",
        "schoolDays": ["MON"],
        "rowVersion": 3,
    });
    assert!(serde_json::from_value::<UpdateAcademicYearRequest>(unknown).is_err());

    let term = json!({
        "academicYearId": Uuid::new_v4(),
        "termType": "regular",
        "customName": null,
        "startDate": "2027-05-01",
        "plannedEndDate": null,
        "includedInYearResult": true,
        "blocksYearClosure": true,
        "bellScheduleId": Uuid::new_v4()
    });
    assert!(serde_json::from_value::<CreateAcademicTermRequest>(term).is_ok());
    let retired_term = json!({
        "academicYearId": Uuid::new_v4(),
        "termType": "regular",
        "customName": null,
        "startDate": "2027-05-01",
        "endDate": "2027-09-30",
        "includedInYearResult": true,
        "blocksYearClosure": true,
        "bellScheduleId": Uuid::new_v4()
    });
    assert!(serde_json::from_value::<CreateAcademicTermRequest>(retired_term).is_err());

    let schedule = json!({
        "academicYearId": Uuid::new_v4(),
        "name": "ตารางเวลาปกติ",
        "owningOrganizationUnitId": null
    });
    assert!(serde_json::from_value::<CreateBellScheduleRequest>(schedule).is_ok());

    let homeroom = json!({
        "academicYearId": Uuid::new_v4(),
        "customName": null,
        "gradeLevelId": Uuid::new_v4(),
        "roomNumber": "1",
        "studyProgramId": Uuid::new_v4(),
        "capacity": 30
    });
    assert!(serde_json::from_value::<CreateHomeroomRequest>(homeroom).is_ok());

    let legacy_homeroom = json!({
        "academicYearId": Uuid::new_v4(),
        "code": "M1-1",
        "name": "ม.1/1",
        "gradeLevelId": Uuid::new_v4(),
        "roomNumber": "1",
        "studyProgramId": Uuid::new_v4(),
        "capacity": 30
    });
    assert!(serde_json::from_value::<CreateHomeroomRequest>(legacy_homeroom).is_err());
}

#[test]
fn canonical_decimal_validation_rejects_ambiguous_wire_values() {
    assert_eq!(
        validate_canonical_decimal("2.50", 2).unwrap().to_string(),
        "2.50"
    );
    assert_eq!(validate_canonical_decimal("0", 2).unwrap().to_string(), "0");
    for invalid in ["02.50", "+2.5", "2.500", "2e1", " 2.5", "-0"] {
        assert!(validate_canonical_decimal(invalid, 2).is_err(), "{invalid}");
    }
}

#[test]
fn child_dates_must_be_contained_by_parent_dates() {
    let year_start = NaiveDate::from_ymd_opt(2027, 5, 1).unwrap();
    let year_end = NaiveDate::from_ymd_opt(2028, 3, 31).unwrap();
    let term_start = NaiveDate::from_ymd_opt(2027, 10, 1).unwrap();
    let term_end = NaiveDate::from_ymd_opt(2028, 1, 31).unwrap();
    assert!(validate_date_containment(year_start, year_end, term_start, term_end).is_ok());
    assert!(validate_date_containment(
        year_start,
        year_end,
        year_start.pred_opt().unwrap(),
        term_end
    )
    .is_err());
    assert!(validate_date_containment(
        year_start,
        year_end,
        term_start,
        year_end.succ_opt().unwrap()
    )
    .is_err());
}

#[test]
fn optimistic_versions_must_be_positive() {
    assert_eq!(parse_row_version(1).unwrap(), 1);
    assert!(parse_row_version(0).is_err());
    assert!(parse_row_version(-1).is_err());
}

#[test]
fn published_versions_are_immutable() {
    assert!(ensure_draft_version(VersionStatus::Draft).is_ok());
    assert!(ensure_draft_version(VersionStatus::Published).is_err());
    assert!(ensure_draft_version(VersionStatus::Archived).is_err());
}

#[test]
fn term_delete_requires_planning_and_no_dependencies() {
    assert!(ensure_planning_delete(AcademicTermStatus::Planning, 0).is_ok());
    assert!(ensure_planning_delete(AcademicTermStatus::Ready, 0).is_err());
    assert!(ensure_planning_delete(AcademicTermStatus::Planning, 1).is_err());
}

#[test]
fn draft_replacement_and_subject_group_updates_require_row_versions() {
    let progression = json!({ "progressions": [], "rowVersion": 4 });
    let request: ReplaceGradeProgressionsRequest = serde_json::from_value(progression).unwrap();
    assert_eq!(request.row_version, 4);
    assert!(
        serde_json::from_value::<ReplaceGradeProgressionsRequest>(json!({
            "progressions": []
        }))
        .is_err()
    );

    let group = json!({
        "code": "MA",
        "nameTh": "คณิตศาสตร์",
        "nameEn": "Mathematics",
        "displayOrder": 2,
        "isActive": true,
        "rowVersion": 3
    });
    let request: UpdateSubjectGroupRequest = serde_json::from_value(group).unwrap();
    assert_eq!(request.row_version, 3);
}

#[test]
fn flat_version_update_contract_rejects_unknown_fields() {
    let payload = json!({
        "nameTh": "คณิตศาสตร์พื้นฐาน",
        "nameEn": null,
        "credit": "1.50",
        "hoursPerSemester": 60,
        "subjectType": "BASIC",
        "description": null,
        "effectiveFrom": "2027-05-01",
        "effectiveUntil": null,
        "termCode": "T1",
        "periodsPerWeek": 3,
        "gradeLevelIds": [],
        "rowVersion": 2
    });
    let request: UpdateSubjectVersionRequest = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(request.row_version, 2);

    let mut unknown = payload;
    unknown["legacyTerm"] = json!("1");
    assert!(serde_json::from_value::<UpdateSubjectVersionRequest>(unknown).is_err());
}

#[tokio::test]
async fn context_options_keep_closing_term_as_current_for_staff_and_students() {
    let pool = prepare_current_core_fixture("context_closing_current").await;
    let (year, term): (Uuid, Uuid) =
        sqlx::query_as("SELECT academic_year_id,id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let student: Uuid = sqlx::query_scalar(
        "SELECT student_id FROM student_academic_years WHERE academic_year_id=$1 LIMIT 1",
    )
    .bind(year)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closing' WHERE id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    for options in [
        context::list_options(&pool).await.unwrap(),
        context::list_public_options(&pool).await.unwrap(),
        context::list_options_for_student(&pool, student)
            .await
            .unwrap(),
    ] {
        assert_eq!(options.active_academic_year_id, Some(year));
        assert_eq!(options.active_academic_term_id, Some(term));
    }
}

#[tokio::test]
async fn context_options_keep_closing_year_as_current_for_staff_and_students() {
    let pool = prepare_core_fixture_through("context_closing_year_current", 70).await;
    let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE status='active'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let student: Uuid = sqlx::query_scalar(
        "SELECT student_id FROM student_academic_years WHERE academic_year_id=$1 LIMIT 1",
    )
    .bind(year)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_years SET status='closing' WHERE id=$1")
        .bind(year)
        .execute(&pool)
        .await
        .unwrap();
    for options in [
        context::list_options(&pool).await.unwrap(),
        context::list_public_options(&pool).await.unwrap(),
        context::list_options_for_student(&pool, student)
            .await
            .unwrap(),
    ] {
        assert_eq!(options.active_academic_year_id, Some(year));
    }
}

#[tokio::test]
async fn context_options_are_read_only_and_keep_active_state_unchanged() {
    let pool = prepare_current_core_fixture("academic_core_context_read_only").await;
    let audit_before: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let active_before: (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT year.id, term.id
           FROM academic_years year
           JOIN academic_terms term ON term.academic_year_id = year.id
           WHERE year.status = 'active' AND term.status = 'active'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let options = context::list_options(&pool).await.unwrap();

    let audit_after: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let active_after: (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT year.id, term.id
           FROM academic_years year
           JOIN academic_terms term ON term.academic_year_id = year.id
           WHERE year.status = 'active' AND term.status = 'active'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_after, audit_before);
    assert_eq!(active_after, active_before);
    assert_eq!(options.active_academic_year_id, Some(active_before.0));
    assert_eq!(options.active_academic_term_id, Some(active_before.1));
    assert_eq!(options.years.len(), 4);
    assert_eq!(options.terms.len(), 9);
}

#[tokio::test]
async fn create_term_seeds_phase_controls() {
    let pool = prepare_current_core_fixture("academic_term_all_controls").await;
    let actor = fixture_actor(&pool).await;
    let bell_schedule_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM bell_schedules WHERE academic_year_id = $1 AND is_default",
    )
    .bind(FUTURE_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let created = years_terms::create_term(
        &pool,
        actor,
        CreateAcademicTermRequest {
            academic_year_id: FUTURE_YEAR_ID,
            term_type: AcademicTermType::Remedial,
            custom_name: None,
            start_date: NaiveDate::from_ymd_opt(2027, 4, 1).unwrap(),
            planned_end_date: Some(NaiveDate::from_ymd_opt(2027, 4, 15).unwrap()),
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id,
        },
    )
    .await
    .unwrap();
    for (table, code, enabled, expected) in [
        (
            "academic_assessment_phase_controls",
            "phase_code",
            "plan_editing_enabled",
            vec!["after_midterm", "before_midterm", "final", "midterm"],
        ),
        (
            "academic_gradebook_phase_controls",
            "phase_code",
            "score_entry_enabled",
            vec!["after_midterm", "before_midterm", "final", "midterm"],
        ),
        (
            "academic_learner_evaluation_controls",
            "domain",
            "entry_enabled",
            vec!["desirable_characteristic", "reading_thinking_writing"],
        ),
    ] {
        let controls: Vec<(String, bool, i64, Uuid, Option<Uuid>)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {code}, {enabled}, row_version, academic_year_id, updated_by FROM {table} WHERE academic_term_id = $1 ORDER BY {code}"
        ))).bind(created.id).fetch_all(&pool).await.unwrap();
        assert_eq!(
            controls
                .iter()
                .map(|row| row.0.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{table}"
        );
        assert!(
            controls
                .iter()
                .all(|row| !row.1 && row.2 == 1 && row.3 == FUTURE_YEAR_ID && row.4 == Some(actor)),
            "{table}"
        );
    }
    years_terms::delete_term(&pool, actor, created.id)
        .await
        .unwrap();
    assert!(years_terms::get_term(&pool, created.id).await.is_err());
}

#[tokio::test]
async fn future_term_planning_in_active_year_does_not_activate_or_open_windows() {
    let pool = prepare_current_core_fixture("future_term_active_year").await;
    let actor = fixture_actor(&pool).await;
    let year = years_terms::get_year(&pool, CURRENT_YEAR_ID).await.unwrap();
    assert_eq!(year.status, AcademicYearStatus::Active);
    let active_before: Uuid =
        sqlx::query_scalar("SELECT id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let schedule: Uuid = sqlx::query_scalar(
        "SELECT id FROM bell_schedules WHERE academic_year_id=$1 AND is_default",
    )
    .bind(year.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let created = years_terms::create_term(
        &pool,
        actor,
        CreateAcademicTermRequest {
            academic_year_id: year.id,
            term_type: AcademicTermType::Summer,
            custom_name: None,
            start_date: year.end_date,
            planned_end_date: None,
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: schedule,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.status, AcademicTermStatus::Planning);
    assert!(created.planned_end_date.is_none());
    assert!(created.closed_on.is_none());
    let updated = years_terms::update_term(
        &pool,
        actor,
        created.id,
        UpdateAcademicTermRequest {
            term_type: created.term_type,
            custom_name: Some("ภาคฤดูร้อนสำหรับเตรียมงาน".into()),
            start_date: created.start_date,
            planned_end_date: None,
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: schedule,
            row_version: created.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.row_version, created.row_version + 1);
    assert_eq!(updated.status, AcademicTermStatus::Planning);
    let active = years_terms::get_term(&pool, active_before).await.unwrap();
    let denied = years_terms::update_term(
        &pool,
        actor,
        active.id,
        UpdateAcademicTermRequest {
            term_type: active.term_type,
            custom_name: Some("ห้ามแก้ภาคที่ใช้อยู่".into()),
            start_date: active.start_date,
            planned_end_date: active.planned_end_date,
            included_in_year_result: active.included_in_year_result,
            blocks_year_closure: active.blocks_year_closure,
            bell_schedule_id: active.bell_schedule_id,
            row_version: active.row_version,
        },
    )
    .await;
    assert!(matches!(denied, Err(school_errors::AppError::Conflict(_))));
    assert_eq!(
        years_terms::get_term(&pool, active.id)
            .await
            .unwrap()
            .row_version,
        active.row_version
    );
    let active_after: Uuid =
        sqlx::query_scalar("SELECT id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(active_before, active_after);
    for (table, enabled, count) in [
        (
            "academic_assessment_phase_controls",
            "plan_editing_enabled",
            4_i64,
        ),
        (
            "academic_gradebook_phase_controls",
            "score_entry_enabled",
            4,
        ),
        ("academic_learner_evaluation_controls", "entry_enabled", 2),
    ] {
        let totals: (i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT count(*),count(*) FILTER (WHERE {enabled}) FROM {table} WHERE academic_term_id=$1")))
            .bind(created.id).fetch_one(&pool).await.unwrap();
        assert_eq!(totals, (count, 0));
    }
    assert_eq!(
        years_terms::get_year(&pool, year.id).await.unwrap().status,
        AcademicYearStatus::Active
    );
    years_terms::delete_term(&pool, actor, created.id)
        .await
        .unwrap();
    assert!(years_terms::get_term(&pool, created.id).await.is_err());
}

#[tokio::test]
async fn future_term_annual_inclusion_requires_closure_and_repairs_existing_flags() {
    let pool = prepare_core_fixture_through("future_term_annual_inclusion", 60).await;
    let actor = fixture_actor(&pool).await;
    let year = years_terms::get_year(&pool, FUTURE_YEAR_ID).await.unwrap();
    let schedule: Uuid = sqlx::query_scalar(
        "SELECT id FROM bell_schedules WHERE academic_year_id=$1 AND is_default",
    )
    .bind(year.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let request = CreateAcademicTermRequest {
        academic_year_id: year.id,
        term_type: AcademicTermType::Summer,
        custom_name: None,
        start_date: year.end_date,
        planned_end_date: None,
        included_in_year_result: true,
        blocks_year_closure: false,
        bell_schedule_id: schedule,
    };
    assert!(matches!(
        years_terms::create_term(&pool, actor, request.clone()).await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    let created = years_terms::create_term(
        &pool,
        actor,
        CreateAcademicTermRequest {
            included_in_year_result: false,
            ..request
        },
    )
    .await
    .unwrap();
    let invalid_update = UpdateAcademicTermRequest {
        term_type: created.term_type,
        custom_name: None,
        start_date: created.start_date,
        planned_end_date: None,
        included_in_year_result: true,
        blocks_year_closure: false,
        bell_schedule_id: schedule,
        row_version: created.row_version,
    };
    assert!(matches!(
        years_terms::update_term(&pool, actor, created.id, invalid_update).await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    // Reproduce an inconsistent pre-067 configuration, then exercise the forward repair.
    sqlx::query("UPDATE academic_terms SET included_in_year_result=true WHERE id=$1")
        .bind(created.id)
        .execute(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 67).await.unwrap();
    let repaired = years_terms::get_term(&pool, created.id).await.unwrap();
    assert!(repaired.included_in_year_result && repaired.blocks_year_closure);
    assert_eq!(repaired.row_version, created.row_version + 1);
    let rejected = sqlx::query("UPDATE academic_terms SET blocks_year_closure=false WHERE id=$1")
        .bind(created.id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        rejected
            .as_database_error()
            .and_then(|error| error.constraint()),
        Some("academic_terms_included_blocks_closure_check")
    );
}

#[tokio::test]
async fn future_term_configuration_rejects_ready_closing_closed_and_archived_years() {
    let pool = prepare_current_core_fixture("future_term_year_guards").await;
    let actor = fixture_actor(&pool).await;
    let year = years_terms::get_year(&pool, FUTURE_YEAR_ID).await.unwrap();
    let schedule: Uuid = sqlx::query_scalar(
        "SELECT id FROM bell_schedules WHERE academic_year_id=$1 AND is_default",
    )
    .bind(year.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let request = CreateAcademicTermRequest {
        academic_year_id: year.id,
        term_type: AcademicTermType::Custom,
        custom_name: None,
        start_date: year.end_date,
        planned_end_date: None,
        included_in_year_result: false,
        blocks_year_closure: false,
        bell_schedule_id: schedule,
    };
    let created = years_terms::create_term(&pool, actor, request.clone())
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=COALESCE(planned_end_date,start_date) WHERE academic_year_id<>$1 AND status IN ('active','closing')")
        .bind(year.id).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE academic_years SET status='closed' WHERE id<>$1 AND status IN ('active','closing')",
    )
    .bind(year.id)
    .execute(&pool)
    .await
    .unwrap();
    for status in ["ready", "closing", "closed", "archived"] {
        sqlx::query("UPDATE academic_years SET status=$1 WHERE id=$2")
            .bind(status)
            .bind(year.id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            matches!(
                years_terms::create_term(&pool, actor, request.clone()).await,
                Err(school_errors::AppError::Conflict(_))
            ),
            "{status}"
        );
        assert!(
            matches!(
                years_terms::update_term(
                    &pool,
                    actor,
                    created.id,
                    UpdateAcademicTermRequest {
                        term_type: created.term_type,
                        custom_name: None,
                        start_date: created.start_date,
                        planned_end_date: None,
                        included_in_year_result: false,
                        blocks_year_closure: false,
                        bell_schedule_id: schedule,
                        row_version: created.row_version,
                    }
                )
                .await,
                Err(school_errors::AppError::Conflict(_))
            ),
            "{status}"
        );
        assert!(
            matches!(
                years_terms::delete_term(&pool, actor, created.id).await,
                Err(school_errors::AppError::Conflict(_))
            ),
            "{status}"
        );
    }
    assert_eq!(
        years_terms::get_term(&pool, created.id)
            .await
            .unwrap()
            .row_version,
        created.row_version
    );
}

#[tokio::test]
async fn planning_year_and_term_updates_reject_stale_versions_and_unused_term_deletes() {
    let pool = prepare_current_core_fixture("academic_core_year_term_mutations").await;
    let actor = fixture_actor(&pool).await;
    let future = years_terms::get_year(&pool, FUTURE_YEAR_ID).await.unwrap();
    let update_year = UpdateAcademicYearRequest {
        year: future.year,
        custom_name: Some("ปีการศึกษา 2026 เตรียมการ".to_string()),
        start_date: future.start_date,
        end_date: future.end_date,
        school_days: future.school_days,
        row_version: future.row_version,
    };
    let updated = years_terms::update_year(&pool, actor, FUTURE_YEAR_ID, update_year)
        .await
        .unwrap();
    assert_eq!(updated.row_version, future.row_version + 1);
    let stale = years_terms::update_year(
        &pool,
        actor,
        FUTURE_YEAR_ID,
        UpdateAcademicYearRequest {
            year: future.year,
            custom_name: Some("ข้อมูลเก่า".to_string()),
            start_date: future.start_date,
            end_date: future.end_date,
            school_days: vec!["MON".to_string()],
            row_version: future.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(stale.public_message().contains("ถูกแก้ไขแล้ว"));

    let bell_schedule_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM bell_schedules WHERE academic_year_id = $1 AND is_default",
    )
    .bind(FUTURE_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let created = years_terms::create_term(
        &pool,
        actor,
        CreateAcademicTermRequest {
            academic_year_id: FUTURE_YEAR_ID,
            term_type: AcademicTermType::Remedial,
            custom_name: None,
            start_date: NaiveDate::from_ymd_opt(2027, 4, 1).unwrap(),
            planned_end_date: Some(NaiveDate::from_ymd_opt(2027, 4, 15).unwrap()),
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id,
        },
    )
    .await
    .unwrap();
    let phase_controls: Vec<(String, bool)> = sqlx::query_as(
        r#"SELECT phase_code, plan_editing_enabled
           FROM academic_assessment_phase_controls
           WHERE academic_term_id = $1
           ORDER BY phase_code"#,
    )
    .bind(created.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        phase_controls,
        vec![
            ("after_midterm".to_string(), false),
            ("before_midterm".to_string(), false),
            ("final".to_string(), false),
            ("midterm".to_string(), false),
        ]
    );
    let update_term = UpdateAcademicTermRequest {
        term_type: AcademicTermType::Custom,
        custom_name: Some("ภาคซ่อมเสริมปรับปรุง".to_string()),
        start_date: created.start_date,
        planned_end_date: created.planned_end_date,
        included_in_year_result: created.included_in_year_result,
        blocks_year_closure: created.blocks_year_closure,
        bell_schedule_id: created.bell_schedule_id,
        row_version: created.row_version,
    };
    let updated_term = years_terms::update_term(&pool, actor, created.id, update_term)
        .await
        .unwrap();
    assert_eq!(updated_term.row_version, created.row_version + 1);
    assert_eq!(updated_term.term_type, AcademicTermType::Custom);
    assert_eq!(updated_term.code, created.code);
    let stale_term = years_terms::update_term(
        &pool,
        actor,
        created.id,
        UpdateAcademicTermRequest {
            term_type: created.term_type,
            custom_name: Some("ข้อมูลเก่า".to_string()),
            start_date: created.start_date,
            planned_end_date: created.planned_end_date,
            included_in_year_result: created.included_in_year_result,
            blocks_year_closure: created.blocks_year_closure,
            bell_schedule_id: created.bell_schedule_id,
            row_version: created.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(stale_term.public_message().contains("ถูกแก้ไขแล้ว"));

    years_terms::delete_term(&pool, actor, created.id)
        .await
        .expect("an unused planning term must remain deletable after its audit events exist");
    assert!(years_terms::get_term(&pool, created.id).await.is_err());
    let durable_audit: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id = $1")
            .bind(created.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(durable_audit, 3);
}

#[tokio::test]
async fn student_year_read_models_are_human_readable() {
    let pool = prepare_current_core_fixture("academic_core_student_year_read_model").await;
    let records = student_years::list_student_years(
        &pool,
        StudentAcademicYearFilter {
            academic_year_id: CURRENT_YEAR_ID,
            student_id: None,
            grade_level_id: None,
            study_program_id: None,
            homeroom_id: None,
            status: None,
        },
    )
    .await
    .unwrap();
    let record = records.first().expect("fixture student-year record");
    assert!(!record.student_name.is_empty());
    assert!(!record.grade_level_name.is_empty());
    assert!(!record.study_program_name.is_empty());
    assert_ne!(record.student_name, record.student_id.to_string());
}

#[tokio::test]
async fn student_year_candidates_include_only_students_missing_the_target_year() {
    let pool = prepare_current_core_fixture("academic_core_student_year_candidates").await;
    let candidate_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, first_name, last_name, user_type, status) \
         VALUES ($1, $2, $3, $4, $5, 'student', 'active')",
    )
    .bind(candidate_id)
    .bind(format!("candidate-{candidate_id}"))
    .bind("test-password-hash")
    .bind("พร้อม")
    .bind("เรียน")
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO student_info (user_id, student_id) VALUES ($1, '67099')")
        .bind(candidate_id)
        .execute(&pool)
        .await
        .unwrap();

    let query = StudentYearCandidateQuery {
        academic_year_id: FUTURE_YEAR_ID,
        search: Some("67099".to_string()),
        limit: Some(10),
    };
    let candidates = student_years::list_student_year_candidates(&pool, query.clone())
        .await
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].id, candidate_id);
    assert_eq!(candidates[0].student_code.as_deref(), Some("67099"));
    assert_eq!(candidates[0].name, "พร้อม เรียน");

    let (grade_level_id, study_program_id): (Uuid, Uuid) = sqlx::query_as(
        r#"
        SELECT grade.id, program.id
        FROM grade_levels grade
        CROSS JOIN study_programs program
        JOIN curriculum_levels version ON version.id = program.curriculum_level_id
        JOIN curriculum_editions curriculum ON curriculum.id=version.edition_id
        WHERE program.status='published' AND curriculum.status='published'
          AND version.grade_level_ids @> jsonb_build_array(grade.id::text)
          AND (EXISTS (SELECT 1 FROM curriculum_course_requirements r WHERE r.study_program_id=program.id AND r.grade_level_id=grade.id)
            OR EXISTS (SELECT 1 FROM curriculum_activity_requirements r WHERE r.study_program_id=program.id AND r.grade_level_id=grade.id))
        ORDER BY grade.level_type, grade.year, program.id
        LIMIT 1
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    student_years::create_student_year(
        &pool,
        fixture_actor(&pool).await,
        CreateStudentAcademicYearRequest {
            academic_year_id: FUTURE_YEAR_ID,
            student_id: candidate_id,
            grade_level_id,
            study_program_id,
        },
    )
    .await
    .unwrap();

    let after = student_years::list_student_year_candidates(&pool, query)
        .await
        .unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn grade_progression_replacement_uses_one_optimistic_set_revision() {
    let pool = prepare_current_core_fixture("academic_core_progression_revision").await;
    let actor = fixture_actor(&pool).await;
    let before = progressions::list(&pool).await.unwrap();
    let after = progressions::replace(
        &pool,
        actor,
        ReplaceGradeProgressionsRequest {
            progressions: Vec::new(),
            row_version: before.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(after.row_version, before.row_version + 1);
    assert!(after.progressions.is_empty());

    let stale = progressions::replace(
        &pool,
        actor,
        ReplaceGradeProgressionsRequest {
            progressions: Vec::new(),
            row_version: before.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(stale.public_message().contains("ผู้ใช้อื่น"));
}

#[tokio::test]
async fn bell_schedule_period_replacement_is_atomic_and_rejects_stale_revisions() {
    let pool = prepare_current_core_fixture("academic_core_bell_schedule_runtime").await;
    let actor = fixture_actor(&pool).await;
    let schedule = bell_schedules::create(
        &pool,
        actor,
        CreateBellScheduleRequest {
            academic_year_id: FUTURE_YEAR_ID,
            name: "ตารางคาบทางเลือก".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let request = ReplaceBellSchedulePeriodsRequest {
        periods: vec![
            BellSchedulePeriodInput {
                name: Some("คาบ 1".to_string()),
                start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
                order_index: 1,
                applicable_days: vec!["MON".to_string(), "TUE".to_string()],
                is_active: true,
            },
            BellSchedulePeriodInput {
                name: Some("คาบ 2".to_string()),
                start_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(10, 10, 0).unwrap(),
                order_index: 2,
                applicable_days: vec!["MON".to_string(), "TUE".to_string()],
                is_active: true,
            },
        ],
        row_version: schedule.row_version,
    };

    let periods = bell_schedules::replace_periods(&pool, actor, schedule.id, request.clone())
        .await
        .unwrap();
    assert_eq!(periods.len(), 2);
    assert_eq!(periods[0].applicable_days.as_deref(), Some("MON,TUE"));
    let current = bell_schedules::get(&pool, schedule.id).await.unwrap();
    assert_eq!(current.row_version, schedule.row_version + 1);
    let stale = bell_schedules::replace_periods(&pool, actor, schedule.id, request.clone())
        .await
        .unwrap_err();
    assert!(stale.public_message().contains("ผู้ใช้อื่น"));
    assert_eq!(
        bell_schedules::list_periods(&pool, schedule.id)
            .await
            .unwrap()
            .len(),
        2
    );
    for status in ["closed", "archived"] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(schedule.academic_year_id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        let mut replacement = request.clone();
        replacement.row_version = current.row_version;
        assert!(matches!(
            bell_schedules::replace_periods(&pool, actor, schedule.id, replacement).await,
            Err(school_errors::AppError::Conflict(_))
        ));
        assert_eq!(
            bell_schedules::get(&pool, schedule.id)
                .await
                .unwrap()
                .row_version,
            current.row_version
        );
    }
    sqlx::query("UPDATE academic_years SET status='planning' WHERE id=$1")
        .bind(schedule.academic_year_id)
        .execute(&pool)
        .await
        .unwrap();
    for replace in [false, true] {
        let current = bell_schedules::get(&pool, schedule.id).await.unwrap();
        let mut replacement = request.clone();
        replacement.row_version = current.row_version;
        let update = school_academic_core::models::UpdateBellScheduleRequest {
            name: current.name,
            is_default: current.is_default,
            owning_organization_unit_id: current.owning_organization_unit_id,
            row_version: current.row_version,
        };
        let mut boundary = pool.begin().await.unwrap();
        let boundary_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *boundary)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM academic_years WHERE id=$1 FOR UPDATE")
            .bind(schedule.academic_year_id)
            .execute(&mut *boundary)
            .await
            .unwrap();
        let worker_pool = pool.clone();
        let schedule_id = schedule.id;
        let worker = tokio::spawn(async move {
            if replace {
                bell_schedules::replace_periods(&worker_pool, actor, schedule_id, replacement)
                    .await
                    .map(|_| ())
            } else {
                bell_schedules::update(&worker_pool, actor, schedule_id, update)
                    .await
                    .map(|_| ())
            }
        });
        let mut waiting = false;
        for _ in 0..200 {
            waiting = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))")
                .bind(boundary_pid).fetch_one(&pool).await.unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let mut probe = pool.begin().await.unwrap();
        let schedule_free =
            sqlx::query("SELECT id FROM bell_schedules WHERE id=$1 FOR UPDATE NOWAIT")
                .bind(schedule.id)
                .execute(&mut *probe)
                .await
                .is_ok();
        probe.rollback().await.unwrap();
        boundary.commit().await.unwrap();
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), worker)
            .await
            .unwrap()
            .unwrap();
        assert!(
            waiting && schedule_free,
            "year must precede schedule, replace={replace}"
        );
        assert!(outcome.is_ok(), "{outcome:?}");
    }
}

#[tokio::test]
async fn subject_group_updates_use_optimistic_revisions() {
    let pool = prepare_current_core_fixture("academic_core_subject_group_revision").await;
    let group = catalog::create_subject_group(
        &pool,
        CreateSubjectGroupRequest {
            code: "TEST-GROUP".to_string(),
            name_th: "กลุ่มสาระทดสอบ".to_string(),
            name_en: "Test Group".to_string(),
            display_order: 99,
            is_active: true,
        },
    )
    .await
    .unwrap();
    let update = UpdateSubjectGroupRequest {
        code: group.code.clone(),
        name_th: "กลุ่มสาระทดสอบปรับปรุง".to_string(),
        name_en: group.name_en.clone(),
        display_order: 99,
        is_active: true,
        row_version: group.row_version,
    };
    let updated = catalog::update_subject_group(&pool, group.id, update.clone())
        .await
        .unwrap();
    assert_eq!(updated.row_version, group.row_version + 1);

    let stale = catalog::update_subject_group(&pool, group.id, update)
        .await
        .unwrap_err();
    assert!(stale.public_message().contains("ผู้ใช้อื่น"));
}

#[tokio::test]
async fn lifecycle_year_future_student_preparation_preserves_current_year_and_idempotent_transfers()
{
    let pool = prepare_current_core_fixture("academic_core_student_year_transfer").await;
    let actor = fixture_actor(&pool).await;
    let existing_context: (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT student_year.grade_level_id, student_year.study_program_id, placement.homeroom_id
           FROM student_academic_years student_year
           JOIN homeroom_placements placement
             ON placement.student_academic_year_id = student_year.id
            AND placement.status = 'current'
           WHERE student_year.academic_year_id = $1
           ORDER BY student_year.student_id
           LIMIT 1"#,
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let student_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name, user_type, status
           ) VALUES ($1, $2, $2, 'fixture-not-a-login', 'นักเรียน', 'ทดสอบ', 'student', 'active')"#,
    )
    .bind(student_id)
    .bind(format!("future-student-{student_id}@example.invalid"))
    .execute(&pool)
    .await
    .unwrap();
    let current_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO student_academic_years (
               id, student_id, academic_year_id, grade_level_id, study_program_id, status
           ) VALUES ($1, $2, $3, $4, $5, 'active')"#,
    )
    .bind(current_id)
    .bind(student_id)
    .bind(CURRENT_YEAR_ID)
    .bind(existing_context.0)
    .bind(existing_context.1)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homeroom_placements (
               id, student_academic_year_id, academic_year_id, homeroom_id,
               start_date, status, enrollment_type, class_number
           ) VALUES ($1, $2, $3, $4, '2025-05-01', 'current', 'regular', 99)"#,
    )
    .bind(Uuid::new_v4())
    .bind(current_id)
    .bind(CURRENT_YEAR_ID)
    .bind(existing_context.2)
    .execute(&pool)
    .await
    .unwrap();
    let current = (
        current_id,
        student_id,
        existing_context.0,
        existing_context.1,
        1_i64,
    );
    let current_placement_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM homeroom_placements WHERE student_academic_year_id = $1",
    )
    .bind(current.0)
    .fetch_one(&pool)
    .await
    .unwrap();

    let future = student_years::create_student_year(
        &pool,
        actor,
        CreateStudentAcademicYearRequest {
            academic_year_id: FUTURE_YEAR_ID,
            student_id: current.1,
            grade_level_id: current.2,
            study_program_id: current.3,
        },
    )
    .await
    .unwrap();
    let current_after: i64 =
        sqlx::query_scalar("SELECT row_version FROM student_academic_years WHERE id = $1")
            .bind(current.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    let current_placements_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM homeroom_placements WHERE student_academic_year_id = $1",
    )
    .bind(current.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(current_after, current.4);
    assert_eq!(current_placements_after, current_placement_count);

    let homeroom_a = student_years::create_homeroom(
        &pool,
        CreateHomeroomRequest {
            academic_year_id: FUTURE_YEAR_ID,
            custom_name: Some("ห้องอนาคต A".to_string()),
            grade_level_id: current.2,
            room_number: "A".to_string(),
            study_program_id: current.3,
            capacity: 40,
        },
    )
    .await
    .unwrap();
    let homeroom_b = student_years::create_homeroom(
        &pool,
        CreateHomeroomRequest {
            academic_year_id: FUTURE_YEAR_ID,
            custom_name: Some("ห้องอนาคต B".to_string()),
            grade_level_id: current.2,
            room_number: "B".to_string(),
            study_program_id: current.3,
            capacity: 40,
        },
    )
    .await
    .unwrap();
    let wrong_year_homeroom: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let wrong_year = student_years::create_placement(
        &pool,
        actor,
        future.id,
        CreateHomeroomPlacementRequest {
            homeroom_id: wrong_year_homeroom,
            start_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            status: HomeroomPlacementStatus::Planned,
            enrollment_type: "promotion".to_string(),
            class_number: None,
            row_version: future.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(wrong_year.public_message().contains("ปี"));

    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(FUTURE_YEAR_ID)
        .execute(&pool)
        .await
        .unwrap();
    let blocked_placement = student_years::create_placement(
        &pool,
        actor,
        future.id,
        CreateHomeroomPlacementRequest {
            homeroom_id: homeroom_a.id,
            start_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            status: HomeroomPlacementStatus::Current,
            enrollment_type: "promotion".into(),
            class_number: Some(1),
            row_version: future.row_version,
        },
    )
    .await;
    assert!(
        matches!(blocked_placement, Err(school_errors::AppError::Conflict(_))),
        "{blocked_placement:?}"
    );
    sqlx::query("UPDATE academic_years SET status='planning' WHERE id=$1")
        .bind(FUTURE_YEAR_ID)
        .execute(&pool)
        .await
        .unwrap();
    let placement = student_years::create_placement(
        &pool,
        actor,
        future.id,
        CreateHomeroomPlacementRequest {
            homeroom_id: homeroom_a.id,
            start_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            status: HomeroomPlacementStatus::Current,
            enrollment_type: "promotion".to_string(),
            class_number: Some(1),
            row_version: future.row_version,
        },
    )
    .await
    .unwrap();
    let idempotency_key = Uuid::new_v4();
    let blank_reason = student_years::transfer_placement(
        &pool,
        actor,
        placement.id,
        TransferHomeroomPlacementRequest {
            target_homeroom_id: homeroom_b.id,
            transfer_date: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            enrollment_type: "room_transfer".to_string(),
            class_number: Some(2),
            reason: "  ".to_string(),
            row_version: placement.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap_err();
    assert!(blank_reason.public_message().contains("เหตุผล"));

    let transfer_request = TransferHomeroomPlacementRequest {
        target_homeroom_id: homeroom_b.id,
        transfer_date: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        enrollment_type: "room_transfer".to_string(),
        class_number: Some(2),
        reason: "ปรับห้องให้เหมาะกับแผนการเรียน".to_string(),
        row_version: placement.row_version,
        idempotency_key,
    };
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(FUTURE_YEAR_ID)
        .execute(&pool)
        .await
        .unwrap();
    let blocked_transfer =
        student_years::transfer_placement(&pool, actor, placement.id, transfer_request.clone())
            .await;
    assert!(
        matches!(blocked_transfer, Err(school_errors::AppError::Conflict(_))),
        "{blocked_transfer:?}"
    );
    sqlx::query("UPDATE academic_years SET status='planning' WHERE id=$1")
        .bind(FUTURE_YEAR_ID)
        .execute(&pool)
        .await
        .unwrap();
    let first =
        student_years::transfer_placement(&pool, actor, placement.id, transfer_request.clone())
            .await
            .unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(FUTURE_YEAR_ID)
        .execute(&pool)
        .await
        .unwrap();
    let replay = student_years::transfer_placement(&pool, actor, placement.id, transfer_request)
        .await
        .unwrap();
    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(first.new_placement.id, replay.new_placement.id);
    let placement_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM homeroom_placements WHERE student_academic_year_id = $1",
    )
    .bind(future.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(placement_count, 2);

    let history = student_years::list_placements(&pool, future.id)
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].id, first.ended_placement.id);
    assert_eq!(history[1].id, first.new_placement.id);

    let audit_reason: String = sqlx::query_scalar(
        "SELECT payload->>'reason' FROM academic_audit_events \
         WHERE event_code = 'homeroom_placement.transferred' AND entity_id = $1",
    )
    .bind(first.ended_placement.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_reason, "ปรับห้องให้เหมาะกับแผนการเรียน");
}

#[tokio::test]
async fn year_relationship_collections_do_not_leak_across_years() {
    let pool = prepare_current_core_fixture("academic_core_year_relationship_collections").await;
    let (grade_level_id, study_program_id, current_homeroom_id): (Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT grade_level_id, study_program_id, id FROM homerooms \
             WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
        )
        .bind(CURRENT_YEAR_ID)
        .fetch_one(&pool)
        .await
        .unwrap();
    let future_homeroom_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(FUTURE_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let staff_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type = 'staff' AND status = 'active' ORDER BY id LIMIT 2",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(staff_ids.len(), 2, "fixture must provide two active staff");

    let second_current_homeroom_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO homerooms (
               id, code, name, academic_year_id, grade_level_id, room_number,
               study_program_id, capacity, is_active
           ) VALUES ($1, 'BATCH-ADVISOR', 'ห้องทดสอบครูที่ปรึกษา', $2, $3, 'BATCH', $4, 40, true)"#,
    )
    .bind(second_current_homeroom_id)
    .bind(CURRENT_YEAR_ID)
    .bind(grade_level_id)
    .bind(study_program_id)
    .execute(&pool)
    .await
    .unwrap();
    let advisor_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    sqlx::query(
        r#"INSERT INTO homeroom_advisors (id, homeroom_id, user_id, role)
           VALUES ($1, $2, $3, 'primary'),
                  ($4, $5, $6, 'secondary'),
                  ($7, $8, $3, 'primary')"#,
    )
    .bind(advisor_ids[0])
    .bind(current_homeroom_id)
    .bind(staff_ids[0])
    .bind(advisor_ids[1])
    .bind(second_current_homeroom_id)
    .bind(staff_ids[1])
    .bind(advisor_ids[2])
    .bind(future_homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    let student_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name, user_type, status
           ) VALUES ($1, $2, $2, 'fixture-not-a-login', 'นักเรียน', 'ชุดข้อมูล', 'student', 'active')"#,
    )
    .bind(student_id)
    .bind(format!("batch-year-{student_id}@example.invalid"))
    .execute(&pool)
    .await
    .unwrap();
    let student_year_id = Uuid::new_v4();
    let placement_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO student_academic_years (
               id, student_id, academic_year_id, grade_level_id, study_program_id, status
           ) VALUES ($1, $2, $3, $4, $5, 'active')"#,
    )
    .bind(student_year_id)
    .bind(student_id)
    .bind(CURRENT_YEAR_ID)
    .bind(grade_level_id)
    .bind(study_program_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homeroom_placements (
               id, student_academic_year_id, academic_year_id, homeroom_id,
               start_date, status, enrollment_type, class_number
           ) VALUES ($1, $2, $3, $4, '2025-05-01', 'current', 'regular', 98)"#,
    )
    .bind(placement_id)
    .bind(student_year_id)
    .bind(CURRENT_YEAR_ID)
    .bind(second_current_homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    let placements = student_years::list_placements_for_year(&pool, CURRENT_YEAR_ID)
        .await
        .unwrap();
    let expected_placement_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM homeroom_placements WHERE academic_year_id = $1")
            .bind(CURRENT_YEAR_ID)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(placements.len() as i64, expected_placement_count);
    assert!(placements.len() >= 2);
    assert!(placements
        .iter()
        .all(|placement| placement.academic_year_id == CURRENT_YEAR_ID));
    assert!(placements
        .iter()
        .any(|placement| placement.id == placement_id));

    let future_placements = student_years::list_placements_for_year(&pool, FUTURE_YEAR_ID)
        .await
        .unwrap();
    assert!(future_placements
        .iter()
        .all(|placement| placement.academic_year_id == FUTURE_YEAR_ID));
    assert!(future_placements
        .iter()
        .all(|future| { placements.iter().all(|current| current.id != future.id) }));

    let advisors = student_years::list_advisors_for_year(&pool, CURRENT_YEAR_ID)
        .await
        .unwrap();
    assert!(advisors.iter().any(|advisor| advisor.id == advisor_ids[0]));
    assert!(advisors.iter().any(|advisor| advisor.id == advisor_ids[1]));
    assert!(!advisors.iter().any(|advisor| advisor.id == advisor_ids[2]));
    let current_homeroom_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM homerooms WHERE academic_year_id = $1")
            .bind(CURRENT_YEAR_ID)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(advisors
        .iter()
        .all(|advisor| current_homeroom_ids.contains(&advisor.homeroom_id)));

    let unknown_year_id = Uuid::new_v4();
    assert!(matches!(
        student_years::list_placements_for_year(&pool, unknown_year_id).await,
        Err(school_errors::AppError::NotFound(_))
    ));
    assert!(matches!(
        student_years::list_advisors_for_year(&pool, unknown_year_id).await,
        Err(school_errors::AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn year_relationship_collections_reject_oversized_workspaces() {
    let pool = prepare_current_core_fixture("academic_core_year_relationship_limits").await;
    let (homeroom_id, grade_level_id, study_program_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT id, grade_level_id, study_program_id FROM homerooms \
         WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name, user_type, status
           )
           SELECT gen_random_uuid(),
                  'batch-limit-staff-' || sequence || '@example.invalid',
                  'batch-limit-staff-' || sequence,
                  'fixture-not-a-login', 'ครู', sequence::text, 'staff', 'active'
           FROM generate_series(1, 2001) sequence"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homeroom_advisors (id, homeroom_id, user_id, role)
           SELECT gen_random_uuid(), $1, id, 'secondary'
           FROM users WHERE username LIKE 'batch-limit-staff-%'"#,
    )
    .bind(homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name, user_type, status
           )
           SELECT gen_random_uuid(),
                  'batch-limit-student-' || sequence || '@example.invalid',
                  'batch-limit-student-' || sequence,
                  'fixture-not-a-login', 'นักเรียน', sequence::text, 'student', 'active'
           FROM generate_series(1, 2001) sequence"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO student_academic_years (
               id, student_id, academic_year_id, grade_level_id, study_program_id, status
           )
           SELECT gen_random_uuid(), id, $1, $2, $3, 'active'
           FROM users WHERE username LIKE 'batch-limit-student-%'"#,
    )
    .bind(CURRENT_YEAR_ID)
    .bind(grade_level_id)
    .bind(study_program_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homeroom_placements (
               id, student_academic_year_id, academic_year_id, homeroom_id,
               start_date, status, enrollment_type
           )
           SELECT gen_random_uuid(), student_year.id, $1, $2,
                  '2025-05-01', 'current', 'regular'
           FROM student_academic_years student_year
           JOIN users student ON student.id = student_year.student_id
           WHERE student.username LIKE 'batch-limit-student-%'"#,
    )
    .bind(CURRENT_YEAR_ID)
    .bind(homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    let advisors = student_years::list_advisors_for_year(&pool, CURRENT_YEAR_ID).await;
    let placements = student_years::list_placements_for_year(&pool, CURRENT_YEAR_ID).await;
    assert!(matches!(
        advisors,
        Err(school_errors::AppError::ValidationError(_))
    ));
    assert!(matches!(
        placements,
        Err(school_errors::AppError::ValidationError(_))
    ));
}

#[tokio::test]
async fn study_program_options_are_published_effective_and_authorized() {
    let pool = prepare_current_core_fixture("academic_core_program_options").await;
    let owner_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        owner_ids.len(),
        2,
        "fixture must provide two active organization units"
    );
    let owner_one_id = owner_ids[0];
    let owner_two_id = owner_ids[1];

    let (current_curriculum_id, current_program_id) = create_published_program_option_fixture(
        &pool,
        owner_two_id,
        CURRENT_YEAR_ID,
        None,
        "CURRENT",
    )
    .await;
    let (_, future_program_id) = create_published_program_option_fixture(
        &pool,
        owner_two_id,
        FUTURE_YEAR_ID,
        None,
        "FUTURE",
    )
    .await;
    let (_, expired_program_id) = create_published_program_option_fixture(
        &pool,
        owner_two_id,
        CURRENT_YEAR_ID,
        Some(CURRENT_YEAR_ID),
        "EXPIRED",
    )
    .await;

    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let draft_curriculum = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรตัวเลือกร่าง".to_string(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let draft_version = curriculum::create_level(
        &pool,
        draft_curriculum.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, draft_curriculum.id).await,
            name_th: "ฉบับร่าง".to_string(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let draft_program = curriculum::create_program(
        &pool,
        draft_version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, draft_version.id).await,
            name_th: "แผนการเรียนร่าง".to_string(),

            is_default: true,
        },
    )
    .await
    .unwrap();

    let school_options = curriculum::list_study_program_options_for_year(
        &pool,
        CURRENT_YEAR_ID,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    let current_option = school_options
        .iter()
        .find(|option| option.id == current_program_id)
        .expect("current published program must be selectable");
    assert!(current_option.code.starts_with("PLAN-"));
    assert_eq!(current_option.name, "แผนการเรียน CURRENT");
    assert_eq!(current_option.edition_id, current_curriculum_id);
    assert_eq!(current_option.edition_name, "หลักสูตรตัวเลือก CURRENT");
    assert!(school_options
        .iter()
        .any(|option| option.id == future_program_id));
    assert!(!school_options
        .iter()
        .any(|option| option.id == draft_program.id));

    for filter in [
        AcademicResourceListFilter::default(),
        AcademicResourceListFilter {
            organization_unit_ids: vec![owner_one_id],
            ..Default::default()
        },
        AcademicResourceListFilter {
            organization_tree_unit_ids: vec![owner_two_id],
            ..Default::default()
        },
    ] {
        assert!(matches!(
            curriculum::list_study_program_options_for_year(&pool, CURRENT_YEAR_ID, &filter).await,
            Err(school_errors::AppError::Forbidden(_))
        ));
    }

    let future_options = curriculum::list_study_program_options_for_year(
        &pool,
        FUTURE_YEAR_ID,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    assert!(future_options
        .iter()
        .any(|option| option.id == future_program_id));
    assert!(future_options
        .iter()
        .any(|option| option.id == expired_program_id));

    let unknown_year_id = Uuid::new_v4();
    assert!(matches!(
        curriculum::list_study_program_options_for_year(
            &pool,
            unknown_year_id,
            &AcademicResourceListFilter {
                includes_school_owned: true,
                ..AcademicResourceListFilter::default()
            },
        )
        .await,
        Err(school_errors::AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn catalog_versions_round_trip_exact_values_and_published_rows_are_immutable() {
    let pool = prepare_current_core_fixture("academic_core_catalog_runtime").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject_group_id: Uuid =
        sqlx::query_scalar("SELECT id FROM subject_groups WHERE code = 'MA'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "TEST-EXACT".to_string(),
            subject_group_id,
        },
    )
    .await
    .unwrap();
    let expected_subject_owner_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM organization_units WHERE subject_group_id = $1 AND is_active = true",
    )
    .bind(subject_group_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(subject.subject_group_id, subject_group_id);
    assert_eq!(
        subject.owning_organization_unit_id,
        expected_subject_owner_id
    );
    let version = catalog::create_subject_version(
        &pool,
        subject.id,
        CreateSubjectVersionRequest {
            name_th: "วิชาทดสอบค่าทศนิยม".to_string(),
            name_en: Some("Exact Decimal Test".to_string()),
            credit: "1.50".to_string(),
            hours_per_semester: Some(60),
            subject_type: "BASIC".to_string(),
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: Some("T1".to_string()),
            periods_per_week: Some(3),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    assert_eq!(version.credit, "1.50");
    assert_eq!(version.periods_per_week, Some(3));
    assert_eq!(version.grade_level_ids, vec![grade_level_id]);

    let published = catalog::publish_subject_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(published.status, VersionStatus::Published);
    let immutable = catalog::update_subject_version(
        &pool,
        version.id,
        UpdateSubjectVersionRequest {
            name_th: published.name_th,
            name_en: published.name_en,
            credit: published.credit,
            hours_per_semester: published.hours_per_semester,
            subject_type: published.subject_type,
            description: published.description,
            effective_from: published.effective_from,
            effective_until: published.effective_until,
            term_code: published.term_code,
            periods_per_week: published.periods_per_week,
            grade_level_ids: published.grade_level_ids,
            row_version: published.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(immutable, school_errors::AppError::Conflict(_)));

    let archived = catalog::update_subject(
        &pool,
        subject.id,
        UpdateCatalogSubjectRequest {
            code: subject.code,
            subject_group_id: subject.subject_group_id,
            archived: true,
            row_version: subject.row_version,
        },
    )
    .await
    .unwrap();
    assert!(archived.archived_at.is_some());
}

#[tokio::test]
async fn activity_catalog_versions_round_trip_exact_hours_and_archive_stably() {
    let pool = prepare_current_core_fixture("academic_core_activity_catalog_runtime").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "TEST-GUIDANCE".to_string(),
            activity_type: "guidance".to_string(),
        },
    )
    .await
    .unwrap();
    let expected_activity_owner_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM organization_units WHERE code = 'ACAD-ACT' AND is_active = true",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        activity.owning_organization_unit_id,
        expected_activity_owner_id
    );
    let version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรมแนะแนวทดสอบ".to_string(),
            description: None,
            hours_per_week: "1.50".to_string(),
            hours_per_term: Some("30.00".to_string()),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: Some("T1".to_string()),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    assert_eq!(version.hours_per_week, "1.50");
    assert_eq!(version.hours_per_term.as_deref(), Some("30.00"));
    assert_eq!(version.grade_level_ids, vec![grade_level_id]);

    let published = catalog::publish_activity_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap();
    let immutable = catalog::update_activity_version(
        &pool,
        published.id,
        UpdateActivityVersionRequest {
            name: published.name,
            description: published.description,
            hours_per_week: published.hours_per_week,
            hours_per_term: published.hours_per_term,
            scheduling_mode: published.scheduling_mode,
            effective_from: published.effective_from,
            effective_until: published.effective_until,
            term_code: published.term_code,
            grade_level_ids: published.grade_level_ids,
            row_version: published.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(immutable.public_message().contains("เผยแพร่แล้ว"));

    let archived = catalog::update_activity(
        &pool,
        activity.id,
        UpdateCatalogActivityRequest {
            code: activity.code,
            activity_type: activity.activity_type,
            archived: true,
            row_version: activity.row_version,
        },
    )
    .await
    .unwrap();
    assert!(archived.archived_at.is_some());
}

#[tokio::test]
async fn activity_catalog_requires_total_hours_before_publishing_a_new_version() {
    let pool = prepare_current_core_fixture("academic_core_activity_total_hours_publish").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "TOTAL-HOURS-REQUIRED".to_string(),
            activity_type: "guidance".to_string(),
        },
    )
    .await
    .unwrap();
    let version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรมที่ยังขาดชั่วโมงรวม".to_string(),
            description: None,
            hours_per_week: "1.00".to_string(),
            hours_per_term: None,
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();

    let result = catalog::publish_activity_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await;
    assert!(matches!(
        result,
        Err(school_errors::AppError::ValidationError(message))
            if message.contains("ชั่วโมงรวมต่อภาคเรียน")
    ));
}

#[tokio::test]
async fn catalog_publication_requires_positive_official_workload() {
    let pool = prepare_current_core_fixture("academic_core_catalog_workload_publish").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    let missing_period_subject = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "WORKLOAD-NO-PERIOD".to_string(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    let missing_period_version = catalog::create_subject_version(
        &pool,
        missing_period_subject.id,
        CreateSubjectVersionRequest {
            name_th: "รายวิชาขาดคาบมาตรฐาน".to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".to_string(),
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            periods_per_week: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let missing_period_error = catalog::publish_subject_version(
        &pool,
        missing_period_version.id,
        PublishVersionRequest {
            row_version: missing_period_version.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(missing_period_error
        .public_message()
        .contains("คาบมาตรฐานต่อสัปดาห์"));

    let missing_hours_subject = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "WORKLOAD-NO-HOURS".to_string(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    let missing_hours_version = catalog::create_subject_version(
        &pool,
        missing_hours_subject.id,
        CreateSubjectVersionRequest {
            name_th: "รายวิชาขาดชั่วโมงรวม".to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: None,
            subject_type: "BASIC".to_string(),
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let missing_hours_error = catalog::publish_subject_version(
        &pool,
        missing_hours_version.id,
        PublishVersionRequest {
            row_version: missing_hours_version.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(missing_hours_error
        .public_message()
        .contains("ชั่วโมงรวมต่อภาคเรียน"));

    let zero_hours_activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "WORKLOAD-ZERO-ACTIVITY".to_string(),
            activity_type: "guidance".to_string(),
        },
    )
    .await
    .unwrap();
    let zero_hours_activity_version = catalog::create_activity_version(
        &pool,
        zero_hours_activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรมชั่วโมงเป็นศูนย์".to_string(),
            description: None,
            hours_per_week: "0.00".to_string(),
            hours_per_term: Some("0.00".to_string()),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let zero_hours_error = catalog::publish_activity_version(
        &pool,
        zero_hours_activity_version.id,
        PublishVersionRequest {
            row_version: zero_hours_activity_version.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(zero_hours_error.public_message().contains("มากกว่า 0"));
}

async fn create_overview_subject_version(
    pool: &PgPool,
    subject_id: Uuid,
    grade_level_id: Uuid,
    name: &str,
    effective_from: NaiveDate,
    effective_until: Option<NaiveDate>,
    publish: bool,
) {
    let version = catalog::create_subject_version(
        pool,
        subject_id,
        CreateSubjectVersionRequest {
            name_th: name.to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".to_string(),
            description: None,
            effective_from,
            effective_until,
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    if publish {
        catalog::publish_subject_version(
            pool,
            version.id,
            PublishVersionRequest {
                row_version: version.row_version,
            },
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn catalog_overview_selects_effective_versions_without_promoting_drafts() {
    let pool = prepare_current_core_fixture("catalog_overview_version_states").await;
    let today = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
    let grade_level_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active = true ORDER BY level_type, year, id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let current = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-CURRENT".to_string(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    create_overview_subject_version(
        &pool,
        current.id,
        grade_level_id,
        "รุ่นที่ใช้อยู่",
        NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
        true,
    )
    .await;
    create_overview_subject_version(
        &pool,
        current.id,
        grade_level_id,
        "ร่างที่ยังไม่เผยแพร่",
        NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 4, 30).unwrap()),
        false,
    )
    .await;

    let upcoming = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-UPCOMING".to_string(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    create_overview_subject_version(
        &pool,
        upcoming.id,
        grade_level_id,
        "รุ่นอนาคต",
        NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(),
        None,
        true,
    )
    .await;

    let expired = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-EXPIRED".to_string(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    create_overview_subject_version(
        &pool,
        expired.id,
        grade_level_id,
        "รุ่นเดิม",
        NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()),
        true,
    )
    .await;

    let unpublished = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-UNPUBLISHED".to_string(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();

    let school_filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..AcademicResourceListFilter::default()
    };
    let overview = catalog::list_subject_overview(&pool, &school_filter, &school_filter, today)
        .await
        .unwrap();

    let current_item = overview
        .items
        .iter()
        .find(|item| item.subject.id == current.id)
        .unwrap();
    assert_eq!(current_item.display_state, CatalogDisplayState::Current);
    assert_eq!(
        current_item.display_version.as_ref().unwrap().name_th,
        "รุ่นที่ใช้อยู่"
    );
    assert_eq!(current_item.draft_count, 1);
    assert!(current_item.can_manage);
    assert_eq!(current_item.grade_levels[0].id, grade_level_id);
    assert!(!current_item.grade_levels[0].name.is_empty());

    let upcoming_item = overview
        .items
        .iter()
        .find(|item| item.subject.id == upcoming.id)
        .unwrap();
    assert_eq!(upcoming_item.display_state, CatalogDisplayState::Upcoming);
    assert_eq!(upcoming_item.draft_count, 0);

    let expired_item = overview
        .items
        .iter()
        .find(|item| item.subject.id == expired.id)
        .unwrap();
    assert_eq!(expired_item.display_state, CatalogDisplayState::Expired);

    let unpublished_item = overview
        .items
        .iter()
        .find(|item| item.subject.id == unpublished.id)
        .unwrap();
    assert_eq!(
        unpublished_item.display_state,
        CatalogDisplayState::Unpublished
    );
    assert!(unpublished_item.display_version.is_none());
    assert!(unpublished_item.grade_levels.is_empty());
    assert!(overview
        .grade_level_options
        .iter()
        .any(|level| level.id == grade_level_id && level.short_name.is_some()));
    assert!(overview
        .subject_group_options
        .iter()
        .any(|option| option.subject_group_id == DEFAULT_SUBJECT_GROUP_ID && option.can_manage));

    let read_only_overview = catalog::list_subject_overview(
        &pool,
        &school_filter,
        &AcademicResourceListFilter::default(),
        today,
    )
    .await
    .unwrap();
    assert!(read_only_overview
        .subject_group_options
        .iter()
        .any(|option| option.subject_group_id == DEFAULT_SUBJECT_GROUP_ID && !option.can_manage));

    sqlx::query("UPDATE grade_levels SET is_active = false WHERE id = $1")
        .bind(grade_level_id)
        .execute(&pool)
        .await
        .unwrap();
    let overview_after_grade_archive =
        catalog::list_subject_overview(&pool, &school_filter, &school_filter, today)
            .await
            .unwrap();
    let archived_grade_item = overview_after_grade_archive
        .items
        .iter()
        .find(|item| item.subject.id == current.id)
        .unwrap();
    assert_eq!(archived_grade_item.grade_levels[0].id, grade_level_id);
    assert!(!overview_after_grade_archive
        .grade_level_options
        .iter()
        .any(|level| level.id == grade_level_id));
}

#[tokio::test]
async fn catalog_overview_keeps_activity_owner_scope_and_grade_options() {
    let pool = prepare_current_core_fixture("catalog_overview_activity_scope").await;
    let today = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
    let grade_level_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active = true ORDER BY level_type, year, id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "OVERVIEW-ACTIVITY".to_string(),
            activity_type: "guidance".to_string(),
        },
    )
    .await
    .unwrap();
    let owner_id = activity.owning_organization_unit_id;
    let version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "แนะแนวที่ใช้อยู่".to_string(),
            description: None,
            hours_per_week: "1.00".to_string(),
            hours_per_term: Some("20.00".to_string()),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    catalog::publish_activity_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap();

    let no_access = catalog::list_activity_overview(
        &pool,
        &AcademicResourceListFilter::default(),
        &AcademicResourceListFilter::default(),
        today,
    )
    .await
    .unwrap();
    assert!(!no_access
        .items
        .iter()
        .any(|item| item.activity.id == activity.id));

    let school_read_filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..AcademicResourceListFilter::default()
    };
    let school_scope = catalog::list_activity_overview(
        &pool,
        &school_read_filter,
        &AcademicResourceListFilter::default(),
        today,
    )
    .await
    .unwrap();
    let school_read_item = school_scope
        .items
        .iter()
        .find(|item| item.activity.id == activity.id)
        .unwrap();
    assert!(!school_read_item.can_manage);
    assert!(!school_scope.can_create);

    let owner_filter = AcademicResourceListFilter {
        organization_unit_ids: vec![owner_id],
        ..AcademicResourceListFilter::default()
    };
    let owner_scope = catalog::list_activity_overview(&pool, &owner_filter, &owner_filter, today)
        .await
        .unwrap();
    let item = owner_scope
        .items
        .iter()
        .find(|item| item.activity.id == activity.id)
        .unwrap();
    assert_eq!(item.display_state, CatalogDisplayState::Current);
    assert!(item.can_manage);
    assert_eq!(item.grade_levels[0].id, grade_level_id);
    assert!(owner_scope
        .grade_level_options
        .iter()
        .any(|level| level.id == grade_level_id));
    assert!(owner_scope.can_create);
}

#[tokio::test]
async fn curriculum_overview_lists_editions_with_level_and_program_counts() {
    let pool = prepare_current_core_fixture("academic_core_curriculum_overview").await;
    let edition = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "ฉบับปรับปรุง พุทธศักราช 2570".into(),
            revision_year: 2570,
            description: None,
        },
    )
    .await
    .unwrap();
    let grades: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE is_active ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    for (index, grade) in grades.iter().enumerate() {
        let level = curriculum::create_level(
            &pool,
            edition.id,
            CreateCurriculumLevelRequest {
                draft_id: resource_draft_id(&pool, edition.id).await,
                name_th: format!("ระดับ {}", index),
                grade_level_ids: vec![*grade],
                description: None,
            },
        )
        .await
        .unwrap();
        curriculum::create_program(
            &pool,
            level.id,
            CreateStudyProgramRequest {
                draft_id: resource_draft_id(&pool, level.id).await,
                name_th: "แผนหลัก".into(),
                is_default: true,
            },
        )
        .await
        .unwrap();
    }
    let overview = workspaces::curriculum_overview(
        &pool,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let item = overview
        .items
        .iter()
        .find(|item| item.edition.id == edition.id)
        .unwrap();
    assert_eq!(item.level_count, 2);
    assert_eq!(item.study_program_count, 2);
    assert_eq!(item.edition.revision_year, Some(2570));
    assert!(
        workspaces::curriculum_overview(&pool, &AcademicResourceListFilter::default())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn curriculum_management_options_are_published_scoped_and_ordered() {
    let pool = prepare_current_core_fixture("academic_core_curriculum_management_options").await;
    let affiliations: Vec<(Uuid, Uuid)> = sqlx::query_as(
        r#"SELECT subject_group.id, owner.id
           FROM subject_groups subject_group
           JOIN organization_units owner ON owner.subject_group_id = subject_group.id
           WHERE subject_group.code <> 'AC' AND owner.is_active = true
           ORDER BY subject_group.display_order, subject_group.id
           LIMIT 2"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(affiliations.len(), 2);
    let owner_ids = [affiliations[0].1, affiliations[1].1];
    let activity_owner_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM organization_units WHERE code = 'ACAD-ACT' AND is_active = true",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let grade_level_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active = true ORDER BY level_type, year, id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรตัวเลือก".to_string(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_level(
        &pool,
        curriculum_row.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, curriculum_row.id).await,
            name_th: "ฉบับตัวเลือก".to_string(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();

    let subject = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OPTION-SUBJECT".to_string(),
            subject_group_id: affiliations[0].0,
        },
    )
    .await
    .unwrap();
    let subject_version = catalog::create_subject_version(
        &pool,
        subject.id,
        CreateSubjectVersionRequest {
            name_th: "รายวิชาตัวเลือก".to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".to_string(),
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let subject_version = catalog::publish_subject_version(
        &pool,
        subject_version.id,
        PublishVersionRequest {
            row_version: subject_version.row_version,
        },
    )
    .await
    .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "OPTION-ACTIVITY".to_string(),
            activity_type: "guidance".to_string(),
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรมตัวเลือก".to_string(),
            description: None,
            hours_per_week: "1.00".to_string(),
            hours_per_term: Some("20.00".to_string()),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::publish_activity_version(
        &pool,
        activity_version.id,
        PublishVersionRequest {
            row_version: activity_version.row_version,
        },
    )
    .await
    .unwrap();
    let outside = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OPTION-OUTSIDE".to_string(),
            subject_group_id: affiliations[1].0,
        },
    )
    .await
    .unwrap();
    let outside_version = catalog::create_subject_version(
        &pool,
        outside.id,
        CreateSubjectVersionRequest {
            name_th: "รายวิชานอกขอบเขต".to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".to_string(),
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let outside_version = catalog::publish_subject_version(
        &pool,
        outside_version.id,
        PublishVersionRequest {
            row_version: outside_version.row_version,
        },
    )
    .await
    .unwrap();

    let owner_filter = AcademicResourceListFilter {
        includes_school_owned: true,
        organization_unit_ids: vec![owner_ids[0], activity_owner_id],
        ..AcademicResourceListFilter::default()
    };
    let create_options = workspaces::curriculum_create_options(&pool, &owner_filter)
        .await
        .unwrap();
    assert!(!create_options.grade_levels.is_empty());
    let active_option_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM grade_levels WHERE id = ANY($1) AND is_active = true",
    )
    .bind(
        &create_options
            .grade_levels
            .iter()
            .map(|level| level.id)
            .collect::<Vec<_>>(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_option_count,
        create_options.grade_levels.len() as i64
    );

    let options = workspaces::curriculum_management_options(&pool, version.id, &owner_filter)
        .await
        .unwrap();
    assert!(options
        .catalog_versions
        .iter()
        .any(|option| option.id == subject_version.id
            && option.resource_kind == RequirementResourceKind::Course));
    assert!(options
        .catalog_versions
        .iter()
        .any(|option| option.id == activity_version.id
            && option.resource_kind == RequirementResourceKind::Activity));
    assert!(options
        .catalog_versions
        .iter()
        .any(|option| option.id == outside_version.id));
    assert!(matches!(
        workspaces::curriculum_management_options(
            &pool,
            version.id,
            &AcademicResourceListFilter::default(),
        )
        .await,
        Err(school_errors::AppError::Forbidden(_))
    ));
}

#[tokio::test]
async fn curriculum_program_workspace_resolves_requirement_labels() {
    let pool = prepare_current_core_fixture("academic_core_workspace_reads").await;
    let actor_user_id = fixture_actor(&pool).await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE status = 'published' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "WORKSPACE-ACTIVITY".to_string(),
            activity_type: "guidance".to_string(),
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรม workspace".to_string(),
            description: None,
            hours_per_week: "1.00".to_string(),
            hours_per_term: Some("20.00".to_string()),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::publish_activity_version(
        &pool,
        activity_version.id,
        PublishVersionRequest {
            row_version: activity_version.row_version,
        },
    )
    .await
    .unwrap();
    let activity_version_id = activity_version.id;
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรทดสอบ workspace".to_string(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_level(
        &pool,
        curriculum_row.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, curriculum_row.id).await,
            name_th: "ฉบับ workspace".to_string(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let default_program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            name_th: "แผนหลัก".to_string(),

            is_default: true,
        },
    )
    .await
    .unwrap();
    let alternative_program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            name_th: "แผนทางเลือก".to_string(),

            is_default: false,
        },
    )
    .await
    .unwrap();
    let structure = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    let first_slot_id = structure.term_slots[0].id;
    let second_slot_id = structure
        .term_slots
        .get(1)
        .map_or(first_slot_id, |slot| slot.id);
    curriculum_structure::replace_program_structure(
        &pool,
        default_program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, default_program.id).await,
            requirements: vec![
                CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Activity,
                    catalog_version_id: activity_version_id,
                    grade_level_id,
                    term_slot_id: second_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 2,
                },
                CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Course,
                    catalog_version_id: subject_version_id,
                    grade_level_id,
                    term_slot_id: first_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 1,
                },
            ],
            row_version: default_program.row_version,
        },
    )
    .await
    .unwrap();
    curriculum_structure::replace_program_structure(
        &pool,
        alternative_program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, alternative_program.id).await,
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject_version_id,
                grade_level_id,
                term_slot_id: first_slot_id,
                requirement_kind: RequirementKind::Elective,
                display_order: 1,
            }],
            row_version: alternative_program.row_version,
        },
    )
    .await
    .unwrap();

    let other_curriculum = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรอื่น".to_string(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let other_version = curriculum::create_level(
        &pool,
        other_curriculum.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, other_curriculum.id).await,
            name_th: "ฉบับอื่น".to_string(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let other_program = curriculum::create_program(
        &pool,
        other_version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, other_version.id).await,
            name_th: "แผนอื่น".to_string(),

            is_default: true,
        },
    )
    .await
    .unwrap();

    let program_workspace = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    assert_eq!(
        program_workspace
            .programs
            .iter()
            .map(|program| program.id)
            .collect::<Vec<_>>(),
        vec![default_program.id, alternative_program.id]
    );
    assert_eq!(program_workspace.requirements.len(), 3);
    assert_eq!(
        program_workspace
            .requirements
            .iter()
            .map(|requirement| requirement.study_program_id)
            .collect::<Vec<_>>(),
        vec![
            default_program.id,
            default_program.id,
            alternative_program.id
        ]
    );
    assert_eq!(program_workspace.requirements[0].display_order, 1);
    assert_eq!(program_workspace.requirements[1].display_order, 2);
    let course_requirement = &program_workspace.requirements[0];
    assert_eq!(course_requirement.grade_level.id, grade_level_id);
    assert!(!course_requirement.grade_level.name.is_empty());
    assert_eq!(course_requirement.catalog_version_id, subject_version_id);
    assert_eq!(
        course_requirement.resource_kind,
        RequirementResourceKind::Course
    );
    assert!(!course_requirement.code.is_empty());
    assert!(!course_requirement.name.is_empty());
    let activity_requirement = &program_workspace.requirements[1];
    assert_eq!(activity_requirement.catalog_version_id, activity_version_id);
    assert_eq!(
        activity_requirement.resource_kind,
        RequirementResourceKind::Activity
    );
    assert!(program_workspace
        .programs
        .iter()
        .all(|program| program.id != other_program.id));
    assert!(program_workspace
        .requirements
        .iter()
        .all(|requirement| requirement.study_program_id != other_program.id));
    let serialized_program_workspace = serde_json::to_value(&program_workspace).unwrap();
    let serialized_requirement = &serialized_program_workspace["requirements"][0];
    assert_eq!(
        serialized_requirement["studyProgramId"],
        default_program.id.to_string()
    );
    assert!(serialized_requirement.get("displayOrder").is_some());
    assert!(serialized_requirement.get("gradeLevel").is_some());
    assert!(serialized_requirement.get("metrics").is_some());

    let extra_schedule = bell_schedules::create(
        &pool,
        actor_user_id,
        CreateBellScheduleRequest {
            academic_year_id: FUTURE_YEAR_ID,
            name: "ตารางคาบ workspace".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    bell_schedules::replace_periods(
        &pool,
        actor_user_id,
        extra_schedule.id,
        ReplaceBellSchedulePeriodsRequest {
            periods: vec![BellSchedulePeriodInput {
                name: Some("คาบ workspace".to_string()),
                start_time: NaiveTime::from_hms_opt(7, 30, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(8, 20, 0).unwrap(),
                order_index: 1,
                applicable_days: vec!["MON".to_string()],
                is_active: true,
            }],
            row_version: extra_schedule.row_version,
        },
    )
    .await
    .unwrap();

    let full_access = ActorContext {
        user_id: actor_user_id,
        permissions: vec![
            codes::ACADEMIC_YEAR_READ_SCHOOL.to_string(),
            codes::ACADEMIC_TERM_READ_SCHOOL.to_string(),
        ],
    };
    let setup_workspace = workspaces::setup_workspace(&pool, &full_access)
        .await
        .unwrap();
    let expected_years = years_terms::list_years(&pool).await.unwrap();
    let mut expected_terms = Vec::new();
    let mut expected_schedules = Vec::new();
    for year in &expected_years {
        expected_terms.extend(years_terms::list_terms(&pool, year.id).await.unwrap());
        expected_schedules.extend(bell_schedules::list(&pool, year.id).await.unwrap());
    }
    assert_eq!(
        setup_workspace
            .years
            .iter()
            .map(|year| year.id)
            .collect::<Vec<_>>(),
        expected_years
            .iter()
            .map(|year| year.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        setup_workspace
            .terms
            .iter()
            .map(|term| term.id)
            .collect::<Vec<_>>(),
        expected_terms
            .iter()
            .map(|term| term.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        setup_workspace
            .bell_schedules
            .iter()
            .map(|schedule| schedule.id)
            .collect::<Vec<_>>(),
        expected_schedules
            .iter()
            .map(|schedule| schedule.id)
            .collect::<Vec<_>>()
    );
    assert!(setup_workspace
        .bell_schedules
        .iter()
        .any(|schedule| schedule.id == extra_schedule.id));
    let serialized_setup = serde_json::to_value(&setup_workspace).unwrap();
    assert!(serialized_setup["bellSchedules"]
        .as_array()
        .unwrap()
        .iter()
        .all(|schedule| schedule.get("periods").is_none()));

    for permissions in [
        vec![codes::ACADEMIC_YEAR_READ_SCHOOL.to_string()],
        vec![codes::ACADEMIC_TERM_READ_SCHOOL.to_string()],
    ] {
        let incomplete_actor = ActorContext {
            user_id: actor_user_id,
            permissions,
        };
        assert!(matches!(
            workspaces::setup_workspace(&pool, &incomplete_actor).await,
            Err(school_errors::AppError::Forbidden(_))
        ));
    }
}

#[tokio::test]
async fn curriculum_version_supports_multiple_programs_and_freezes_them_on_publish() {
    let pool = prepare_current_core_fixture("academic_core_curriculum_runtime").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE status = 'published' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรทดสอบหลายแผน".to_string(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_level(
        &pool,
        curriculum_row.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, curriculum_row.id).await,
            name_th: "ฉบับ 2026".to_string(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let default_program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            name_th: "แผนทั่วไป".to_string(),

            is_default: true,
        },
    )
    .await
    .unwrap();
    let science_program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            name_th: "แผนวิทย์-คณิต".to_string(),

            is_default: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        curriculum::list_programs(&pool, version.id)
            .await
            .unwrap()
            .len(),
        2
    );

    let mut structure = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    if structure.term_slots.is_empty() {
        structure = curriculum_structure::replace_term_slots(
            &pool,
            version.id,
            ReplaceCurriculumTermSlotsRequest {
                draft_id: resource_draft_id(&pool, version.id).await,
                slots: vec![CurriculumTermSlotInput {
                    id: None,
                    sequence: 1,
                    term_type: AcademicTermType::Regular,
                    type_occurrence: 1,
                    name: "ภาคเรียนที่ 1".to_string(),
                }],
                row_version: structure.row_version,
            },
        )
        .await
        .unwrap();
    }
    let term_slot_id = structure.term_slots[0].id;

    for program in [&default_program, &science_program] {
        structure = curriculum_structure::replace_program_structure(
            &pool,
            program.id,
            ReplaceCurriculumStructureRequest {
                draft_id: resource_draft_id(&pool, program.id).await,
                requirements: vec![CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Course,
                    catalog_version_id: subject_version_id,
                    grade_level_id,
                    term_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 1,
                }],
                row_version: program.row_version,
            },
        )
        .await
        .unwrap();
        let requirement = structure
            .requirements
            .iter()
            .find(|requirement| requirement.study_program_id == program.id)
            .unwrap();
        assert!(requirement.metrics.credit.is_some());
        assert!(requirement.metrics.total_hours.is_some());
    }

    let published = publish_level_fixture(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: structure.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(published.status, VersionStatus::Published);
    let frozen_program = curriculum::get_program(&pool, science_program.id)
        .await
        .unwrap();
    let immutable = curriculum::update_program(
        &pool,
        frozen_program.id,
        UpdateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, frozen_program.id).await,
            name_th: frozen_program.name_th,

            is_default: frozen_program.is_default,

            row_version: frozen_program.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(immutable, school_errors::AppError::Conflict(_)));
}

#[tokio::test]
async fn curriculum_publication_rejects_missing_official_catalog_metrics() {
    let pool = prepare_current_core_fixture("academic_core_curriculum_metric_blocker").await;
    let (activity_version_id, grade_level_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT version.id, grade.value::uuid
           FROM activity_versions version
           CROSS JOIN LATERAL jsonb_array_elements_text(version.grade_level_ids) grade(value)
           WHERE version.status = 'published'
           ORDER BY version.id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE activity_versions DISABLE TRIGGER activity_versions_published_immutable",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE activity_versions SET hours_per_term = 0 WHERE id = $1")
        .bind(activity_version_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "ALTER TABLE activity_versions ENABLE TRIGGER activity_versions_published_immutable",
    )
    .execute(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรทดสอบข้อมูลทางการ".to_string(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_level(
        &pool,
        curriculum_row.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, curriculum_row.id).await,
            name_th: "ฉบับข้อมูลไม่ครบ".to_string(),
            grade_level_ids: vec![grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, version.id).await,
            name_th: "แผนหลัก".to_string(),

            is_default: true,
        },
    )
    .await
    .unwrap();
    let workspace = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    let workspace = curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, program.id).await,
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Activity,
                catalog_version_id: activity_version_id,
                grade_level_id,
                term_slot_id: workspace.term_slots[0].id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();

    let error = publish_level_fixture(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: workspace.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(error.public_message().contains("ชั่วโมงรวม"));
}

async fn publication_actual_evidence(pool: &PgPool) -> serde_json::Value {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
        'rooms',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM homerooms r),
        'students',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM student_academic_years r),
        'targets',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_offering_targets r),
        'deliveries',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_delivery_versions r),
        'timetables',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_timetable_versions r),
        'blocks',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_timetable_blocks r))",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn curriculum_publication_baseline_preserves_every_graph_and_actual_reference() {
    let pool = prepare_core_fixture_through("curriculum_publication_baseline", 94).await;
    let actual = publication_actual_evidence(&pool).await;
    let levels:serde_json::Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]') FROM curriculum_levels r JOIN curriculum_editions e ON e.id=r.edition_id WHERE e.status='published'").fetch_one(&pool).await.unwrap();
    let programs:serde_json::Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]') FROM study_programs r JOIN curriculum_levels l ON l.id=r.curriculum_level_id JOIN curriculum_editions e ON e.id=l.edition_id WHERE e.status='published'").fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 96).await.unwrap();
    assert_eq!(actual, publication_actual_evidence(&pool).await);
    let captured_levels:serde_json::Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(r)-'publication_id' ORDER BY id),'[]') FROM curriculum_publication_levels r").fetch_one(&pool).await.unwrap();
    let captured_programs:serde_json::Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(r)-'publication_id' ORDER BY id),'[]') FROM curriculum_publication_programs r").fetch_one(&pool).await.unwrap();
    assert_eq!(levels, captured_levels);
    assert_eq!(programs, captured_programs);
    let invented_actors:i64=sqlx::query_scalar("SELECT count(*) FROM curriculum_publications WHERE NOT is_baseline OR published_by IS NOT NULL OR publication_no<>1").fetch_one(&pool).await.unwrap();
    assert_eq!(invented_actors, 0);
    let invented_delivery_sources: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learning_offering_curriculum_sources")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(invented_delivery_sources, 0);
}

async fn publication_fixture(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let grade: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE is_active ORDER BY id LIMIT 1")
            .fetch_one(pool)
            .await
            .unwrap();
    let subject:Uuid=sqlx::query_scalar("SELECT id FROM subject_versions WHERE status='published' AND periods_per_week>0 AND hours_per_semester>0 ORDER BY id LIMIT 1").fetch_one(pool).await.unwrap();
    let (edition, level) = create_curriculum_overview_fixture(
        pool,
        Uuid::nil(),
        grade,
        subject,
        code,
        CURRENT_YEAR_ID,
        None,
        false,
        1,
    )
    .await;
    let workspace = curriculum_structure::get_workspace(pool, level)
        .await
        .unwrap();
    let slot = &workspace.term_slots[0];
    curriculum_structure::replace_term_slots(
        pool,
        level,
        ReplaceCurriculumTermSlotsRequest {
            draft_id: workspace.level.draft_id.unwrap(),
            row_version: workspace.row_version,
            slots: vec![CurriculumTermSlotInput {
                id: Some(slot.id),
                sequence: 1,
                term_type: slot.term_type,
                type_occurrence: slot.type_occurrence,
                name: slot.name.clone(),
            }],
        },
    )
    .await
    .unwrap();
    publish_level_fixture(pool, level, PublishVersionRequest { row_version: 1 })
        .await
        .unwrap();
    (edition, level)
}

#[tokio::test]
async fn curriculum_publication_amendment_adds_term_two_preserves_history_and_actual_delivery() {
    use school_academic_core::models::{CurriculumViewQuery, OpenCurriculumDraftRequest};
    use school_academic_core::services::curriculum_publications as publications;
    let pool = prepare_current_core_fixture("curriculum_publication_term_two").await;
    let (id, level_id) = publication_fixture(&pool, "TERM-TWO").await;
    let before = publications::read_workspace(&pool, level_id, &CurriculumViewQuery::default())
        .await
        .unwrap();
    let program_id = before.programs[0].id;
    let room:Uuid=sqlx::query_scalar("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,capacity,is_active) VALUES(gen_random_uuid(),'PUB-ROOM','ห้องทดสอบ',$1,$2,$3,40,true) RETURNING id")
        .bind(CURRENT_YEAR_ID).bind(before.grade_levels[0].id).bind(program_id).fetch_one(&pool).await.unwrap();
    let actual = publication_actual_evidence(&pool).await;
    let edition = curriculum::get(&pool, id).await.unwrap();
    let draft = publications::open_draft(
        &pool,
        id,
        OpenCurriculumDraftRequest {
            row_version: edition.row_version,
        },
    )
    .await
    .unwrap();
    let token = draft.draft_id.unwrap();
    let view = CurriculumViewQuery {
        draft_id: Some(token),
        publication_id: None,
    };
    let workspace = publications::read_workspace(&pool, level_id, &view)
        .await
        .unwrap();
    let first = &workspace.term_slots[0];
    let workspace = curriculum_structure::replace_term_slots(
        &pool,
        level_id,
        ReplaceCurriculumTermSlotsRequest {
            draft_id: token,
            row_version: workspace.row_version,
            slots: vec![
                CurriculumTermSlotInput {
                    id: Some(first.id),
                    sequence: 1,
                    term_type: first.term_type,
                    type_occurrence: first.type_occurrence,
                    name: first.name.clone(),
                },
                CurriculumTermSlotInput {
                    id: None,
                    sequence: 2,
                    term_type: AcademicTermType::Regular,
                    type_occurrence: 2,
                    name: "ภาคเรียนที่ 2".into(),
                },
            ],
        },
    )
    .await
    .unwrap();
    let first_requirement = &workspace.requirements[0];
    let input = |term_slot_id| CurriculumStructureRequirementInput {
        resource_kind: first_requirement.resource_kind,
        catalog_version_id: first_requirement.catalog_version_id,
        grade_level_id: first_requirement.grade_level.id,
        term_slot_id,
        requirement_kind: first_requirement.requirement_kind,
        display_order: 1,
    };
    curriculum_structure::replace_program_structure(
        &pool,
        program_id,
        ReplaceCurriculumStructureRequest {
            draft_id: token,
            row_version: workspace.programs[0].row_version,
            requirements: vec![
                input(first_requirement.term_slot_id),
                input(workspace.term_slots[1].id),
            ],
        },
    )
    .await
    .unwrap();
    let current_before_publish =
        publications::read_workspace(&pool, level_id, &CurriculumViewQuery::default())
            .await
            .unwrap();
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(current_before_publish).unwrap()
    );
    let second = curriculum::publish(
        &pool,
        id,
        PublishCurriculumRequest {
            draft_id: token,
            row_version: draft.row_version,
            change_note: "เพิ่มภาคเรียนที่ 2".into(),
        },
        fixture_actor(&pool).await,
    )
    .await
    .unwrap();
    assert_eq!(second.publication_count, 2);
    assert_eq!(second.revision_year, Some(2569));
    assert!(second.draft_id.is_none());
    let current = publications::read_workspace(&pool, level_id, &CurriculumViewQuery::default())
        .await
        .unwrap();
    assert_eq!(current.programs[0].id, program_id);
    assert_eq!(current.term_slots.len(), 2);
    assert_eq!(current.requirements.len(), 2);
    let old = publications::read_workspace(
        &pool,
        level_id,
        &CurriculumViewQuery {
            publication_id: edition.current_publication_id,
            draft_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(old).unwrap()
    );
    assert_eq!(actual, publication_actual_evidence(&pool).await);
    let room_program: Uuid =
        sqlx::query_scalar("SELECT study_program_id FROM homerooms WHERE id=$1")
            .bind(room)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(room_program, program_id);
    let history = publications::history(&pool, id, second.current_publication_id.unwrap())
        .await
        .unwrap();
    assert_eq!(
        history.publication.published_by,
        Some(fixture_actor(&pool).await)
    );
    assert_eq!(
        history
            .changes
            .iter()
            .filter(|c| c.before.is_none() && c.resource_kind == "course")
            .count(),
        1
    );
    assert!(
        !history
            .changes
            .iter()
            .any(|c| c.resource_kind == "course" && c.before.is_some()),
        "saving an unchanged first-term requirement must not appear as an edit"
    );
    let next = publications::open_draft(
        &pool,
        id,
        OpenCurriculumDraftRequest {
            row_version: second.row_version,
        },
    )
    .await
    .unwrap();
    assert_ne!(next.draft_id, Some(token));
    let latest = publications::read_workspace(
        &pool,
        level_id,
        &CurriculumViewQuery {
            draft_id: next.draft_id,
            publication_id: None,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        curriculum_structure::replace_program_structure(
            &pool,
            program_id,
            ReplaceCurriculumStructureRequest {
                draft_id: token,
                row_version: latest.programs[0].row_version,
                requirements: vec![]
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert!(publications::read_workspace(&pool, level_id, &view)
        .await
        .is_err());
    // Discard restores publication two, never the baseline or an empty edition.
    sqlx::query("UPDATE curriculum_term_slots SET name='ร่างที่จะทิ้ง' WHERE curriculum_level_id=$1")
        .bind(level_id)
        .execute(&pool)
        .await
        .unwrap();
    let preview = publications::preview_discard(&pool, id, next.draft_id.unwrap())
        .await
        .unwrap();
    let restored = publications::discard_draft(
        &pool,
        id,
        school_academic_core::models::DiscardCurriculumDraftRequest {
            draft_id: preview.draft_id,
            row_version: preview.row_version,
            content_hash: preview.content_hash,
        },
    )
    .await
    .unwrap();
    assert_eq!(restored.publication_count, 2);
    assert_eq!(
        restored.current_publication_id,
        second.current_publication_id
    );
    let restored_workspace =
        publications::read_workspace(&pool, level_id, &CurriculumViewQuery::default())
            .await
            .unwrap();
    assert_eq!(
        serde_json::to_value(current).unwrap(),
        serde_json::to_value(restored_workspace).unwrap()
    );
    assert_eq!(actual, publication_actual_evidence(&pool).await);
}

#[tokio::test]
async fn curriculum_publication_discard_rolls_back_when_new_plan_has_external_reference() {
    use school_academic_core::models::{DiscardCurriculumDraftRequest, OpenCurriculumDraftRequest};
    use school_academic_core::services::curriculum_publications as publications;
    let pool = prepare_current_core_fixture("curriculum_publication_discard_referenced").await;
    let (id, level) = publication_fixture(&pool, "DISCARD-REFERENCED").await;
    let edition = curriculum::get(&pool, id).await.unwrap();
    let draft = publications::open_draft(
        &pool,
        id,
        OpenCurriculumDraftRequest {
            row_version: edition.row_version,
        },
    )
    .await
    .unwrap();
    let token = draft.draft_id.unwrap();
    let added = curriculum::create_program(
        &pool,
        level,
        CreateStudyProgramRequest {
            draft_id: token,
            name_th: "แผนมีห้องอ้างอิง".into(),
            is_default: false,
        },
    )
    .await
    .unwrap();
    let grade = curriculum_structure::get_workspace(&pool, level)
        .await
        .unwrap()
        .grade_levels[0]
        .id;
    sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,capacity,is_active) VALUES(gen_random_uuid(),'DISCARD-REF','ห้องอ้างอิง',$1,$2,$3,40,true)").bind(CURRENT_YEAR_ID).bind(grade).bind(added.id).execute(&pool).await.unwrap();
    let actual = publication_actual_evidence(&pool).await;
    let preview = publications::preview_discard(&pool, id, token)
        .await
        .unwrap();
    let error = publications::discard_draft(
        &pool,
        id,
        DiscardCurriculumDraftRequest {
            draft_id: token,
            row_version: preview.row_version,
            content_hash: preview.content_hash.clone(),
        },
    )
    .await
    .unwrap_err();
    assert!(error.public_message().contains("อ้างอิง"));
    let after = publications::preview_discard(&pool, id, token)
        .await
        .unwrap();
    assert_eq!(after.content_hash, preview.content_hash);
    assert_eq!(after.row_version, preview.row_version);
    assert_eq!(actual, publication_actual_evidence(&pool).await);
}

#[tokio::test]
async fn curriculum_publication_concurrent_open_reuses_one_draft_and_failed_publication_has_no_number(
) {
    use school_academic_core::models::OpenCurriculumDraftRequest;
    use school_academic_core::services::curriculum_publications as publications;
    let pool = prepare_current_core_fixture("curriculum_publication_single_draft").await;
    let (id, level) = publication_fixture(&pool, "CONCURRENT").await;
    let edition = curriculum::get(&pool, id).await.unwrap();
    let request = OpenCurriculumDraftRequest {
        row_version: edition.row_version,
    };
    let (a, b) = tokio::join!(
        publications::open_draft(&pool, id, request.clone()),
        publications::open_draft(&pool, id, request)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.draft_id, b.draft_id);
    assert_eq!(a.row_version, b.row_version);
    assert_eq!(a.row_version, edition.row_version + 1);
    let workspace = curriculum_structure::get_workspace(&pool, level)
        .await
        .unwrap();
    curriculum_structure::replace_program_structure(
        &pool,
        workspace.programs[0].id,
        ReplaceCurriculumStructureRequest {
            draft_id: a.draft_id.unwrap(),
            row_version: workspace.programs[0].row_version,
            requirements: vec![],
        },
    )
    .await
    .unwrap();
    let result = curriculum::publish(
        &pool,
        id,
        PublishCurriculumRequest {
            draft_id: a.draft_id.unwrap(),
            row_version: a.row_version,
            change_note: "ร่างไม่ครบ".into(),
        },
        fixture_actor(&pool).await,
    )
    .await;
    assert!(matches!(
        result,
        Err(school_errors::AppError::ValidationError(_))
    ));
    let unchanged = curriculum::get(&pool, id).await.unwrap();
    assert_eq!(unchanged.publication_count, 1);
    assert_eq!(
        unchanged.current_publication_id,
        edition.current_publication_id
    );
    assert_eq!(unchanged.draft_id, a.draft_id);
    assert_eq!(
        publications::list_publications(&pool, id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn curriculum_publication_database_seals_history_and_rejects_partial_capture() {
    let pool = prepare_current_core_fixture("curriculum_publication_immutable").await;
    let unpublished: Uuid = sqlx::query_scalar("INSERT INTO curriculum_editions(name,revision_year) VALUES('Unpublished fixture',2569) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    assert!(
        sqlx::query("UPDATE curriculum_editions SET status='published' WHERE id=$1")
            .bind(unpublished)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("INSERT INTO curriculum_editions(name,revision_year,status) VALUES('Invalid publication',2569,'published')")
        .execute(&pool).await.is_err());
    let unchanged = curriculum::get(&pool, unpublished).await.unwrap();
    assert_eq!(unchanged.status, VersionStatus::Draft);
    assert_eq!(unchanged.publication_count, 0);
    assert!(unchanged.current_publication_id.is_none());

    let (id, level) = publication_fixture(&pool, "IMMUTABLE").await;
    let edition = curriculum::get(&pool, id).await.unwrap();
    let publication = edition.current_publication_id.unwrap();
    assert!(
        sqlx::query("UPDATE curriculum_publications SET change_note='แก้ย้อนหลัง' WHERE id=$1")
            .bind(publication)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM curriculum_publications WHERE id=$1")
            .bind(publication)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query(
        "UPDATE curriculum_publication_programs SET name_th='แก้ย้อนหลัง' WHERE publication_id=$1"
    )
    .bind(publication)
    .execute(&pool)
    .await
    .is_err());
    assert!(sqlx::query("INSERT INTO curriculum_publication_levels SELECT (jsonb_populate_record(NULL::curriculum_publication_levels,to_jsonb(r)||jsonb_build_object('id',gen_random_uuid()))).* FROM curriculum_publication_levels r WHERE publication_id=$1 LIMIT 1").bind(publication).execute(&pool).await.is_err());
    assert!(
        sqlx::query("UPDATE curriculum_levels SET name_th='แก้โดยไม่มีร่าง' WHERE id=$1")
            .bind(level)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE curriculum_editions SET publication_count=100 WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .is_err()
    );
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO curriculum_publications(edition_id,publication_no,previous_publication_id,name,revision_year,change_note,level_count,program_count,slot_count,course_count,activity_count) VALUES($1,2,$2,'partial',2569,'ไม่ครบ',1,1,1,1,0)").bind(id).bind(publication).execute(&mut *tx).await.unwrap();
    assert!(tx.commit().await.is_err());
    assert_eq!(
        school_academic_core::services::curriculum_publications::list_publications(&pool, id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn curriculum_publication_draft_removal_keeps_actual_offering_requirement_identity() {
    use school_academic_core::models::OpenCurriculumDraftRequest;
    use school_academic_core::services::curriculum_publications as publications;
    let pool = prepare_current_core_fixture("curriculum_publication_used_requirement").await;
    let (requirement_id,program_id,level_id,edition_id):(Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT requirement.id,requirement.study_program_id,requirement.curriculum_level_id,l.edition_id FROM course_offering_details detail JOIN curriculum_course_requirements requirement ON requirement.id=detail.curriculum_course_requirement_id JOIN curriculum_levels l ON l.id=requirement.curriculum_level_id ORDER BY detail.learning_offering_id LIMIT 1").fetch_one(&pool).await.unwrap();
    let actual = publication_actual_evidence(&pool).await;
    let edition = curriculum::get(&pool, edition_id).await.unwrap();
    let draft = publications::open_draft(
        &pool,
        edition_id,
        OpenCurriculumDraftRequest {
            row_version: edition.row_version,
        },
    )
    .await
    .unwrap();
    let workspace = curriculum_structure::get_workspace(&pool, level_id)
        .await
        .unwrap();
    let requirements = workspace
        .requirements
        .iter()
        .filter(|r| r.study_program_id == program_id && r.id != requirement_id)
        .map(|r| CurriculumStructureRequirementInput {
            resource_kind: r.resource_kind,
            catalog_version_id: r.catalog_version_id,
            grade_level_id: r.grade_level.id,
            term_slot_id: r.term_slot_id,
            requirement_kind: r.requirement_kind,
            display_order: r.display_order,
        })
        .collect();
    let program = workspace
        .programs
        .iter()
        .find(|p| p.id == program_id)
        .unwrap();
    curriculum_structure::replace_program_structure(
        &pool,
        program_id,
        ReplaceCurriculumStructureRequest {
            draft_id: draft.draft_id.unwrap(),
            row_version: program.row_version,
            requirements,
        },
    )
    .await
    .unwrap();
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM curriculum_course_requirements WHERE id=$1)"
    )
    .bind(requirement_id)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM course_offering_details detail JOIN curriculum_requirement_sources source ON source.id=detail.curriculum_course_requirement_id WHERE source.id=$1)").bind(requirement_id).fetch_one(&pool).await.unwrap());
    let released = publications::read_workspace(
        &pool,
        level_id,
        &school_academic_core::models::CurriculumViewQuery::default(),
    )
    .await
    .unwrap();
    assert!(released.requirements.iter().any(|r| r.id == requirement_id));
    assert_eq!(actual, publication_actual_evidence(&pool).await);
}

#[tokio::test]
async fn curriculum_publication_discard_restores_all_levels_and_used_requirement_without_history() {
    use school_academic_core::models::{
        CurriculumViewQuery, DiscardCurriculumDraftRequest, OpenCurriculumDraftRequest,
    };
    use school_academic_core::services::curriculum_publications as publications;
    let pool = prepare_current_core_fixture("curriculum_publication_discard_graph").await;
    let (requirement, program, level, id):(Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT r.id,r.study_program_id,r.curriculum_level_id,l.edition_id FROM course_offering_details d JOIN curriculum_course_requirements r ON r.id=d.curriculum_course_requirement_id JOIN curriculum_levels l ON l.id=r.curriculum_level_id ORDER BY d.learning_offering_id LIMIT 1").fetch_one(&pool).await.unwrap();
    let actual = publication_actual_evidence(&pool).await;
    let original = curriculum::get(&pool, id).await.unwrap();
    let history =
        serde_json::to_value(publications::list_publications(&pool, id).await.unwrap()).unwrap();
    let draft = publications::open_draft(
        &pool,
        id,
        OpenCurriculumDraftRequest {
            row_version: original.row_version,
        },
    )
    .await
    .unwrap();
    let token = draft.draft_id.unwrap();
    // Exercise edits throughout the edition, not only the currently selected level.
    sqlx::query("UPDATE curriculum_levels SET name_th=name_th||' ร่าง',description='discard me' WHERE edition_id=$1").bind(id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE study_programs SET name_th=name_th||' ร่าง' WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)").bind(id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE curriculum_activity_requirements SET display_order=display_order+100 WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)").bind(id).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM curriculum_course_requirements WHERE id=$1")
        .bind(requirement)
        .execute(&pool)
        .await
        .unwrap();
    let grade:Uuid=sqlx::query_scalar("SELECT grade_level_id FROM curriculum_publication_courses WHERE publication_id=$1 AND id=$2").bind(original.current_publication_id).bind(requirement).fetch_one(&pool).await.unwrap();
    let added = curriculum::create_level(
        &pool,
        id,
        CreateCurriculumLevelRequest {
            draft_id: token,
            name_th: "ระดับทดลองที่จะลบ".into(),
            grade_level_ids: vec![grade],
            description: None,
        },
    )
    .await
    .unwrap();
    let added_program = curriculum::create_program(
        &pool,
        added.id,
        CreateStudyProgramRequest {
            draft_id: token,
            name_th: "แผนทดลองที่จะลบ".into(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    let added_slot: Uuid = sqlx::query_scalar(
        "SELECT id FROM curriculum_term_slots WHERE curriculum_level_id=$1 AND sequence=2",
    )
    .bind(added.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let added_requirement:Uuid=sqlx::query_scalar("INSERT INTO curriculum_course_requirements(id,curriculum_level_id,grade_level_id,subject_version_id,display_order,metadata,study_program_id,requirement_kind,term_slot_id) SELECT gen_random_uuid(),$1,grade_level_id,subject_version_id,1,metadata,$2,requirement_kind,$3 FROM curriculum_publication_courses WHERE publication_id=$4 AND id=$5 RETURNING id").bind(added.id).bind(added_program.id).bind(added_slot).bind(original.current_publication_id).bind(requirement).fetch_one(&pool).await.unwrap();
    // Preserve IDs of removed released requirements while removing new IDs physically.
    let preview = publications::preview_discard(&pool, id, token)
        .await
        .unwrap();
    let request = DiscardCurriculumDraftRequest {
        draft_id: token,
        row_version: preview.row_version,
        content_hash: preview.content_hash,
    };
    let restored = publications::discard_draft(&pool, id, request.clone())
        .await
        .unwrap();
    assert_eq!(
        restored.current_publication_id,
        original.current_publication_id
    );
    assert_eq!(restored.publication_count, original.publication_count);
    assert!(restored.draft_id.is_none());
    assert_eq!(
        history,
        serde_json::to_value(publications::list_publications(&pool, id).await.unwrap()).unwrap()
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT curriculum_workspace_matches_publication(id,current_publication_id) FROM curriculum_editions WHERE id=$1").bind(id).fetch_one(&pool).await.unwrap());
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM curriculum_course_requirements WHERE id=$1 AND study_program_id=$2 AND curriculum_level_id=$3)").bind(requirement).bind(program).bind(level).fetch_one(&pool).await.unwrap());
    assert!(!sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM curriculum_levels WHERE id=$1) OR EXISTS(SELECT 1 FROM study_programs WHERE id=$2)").bind(added.id).bind(added_program.id).fetch_one(&pool).await.unwrap());
    assert!(!sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM curriculum_requirement_sources WHERE id=$1) OR EXISTS(SELECT 1 FROM curriculum_course_requirements WHERE id=$1) OR EXISTS(SELECT 1 FROM curriculum_term_slots WHERE id=$2)").bind(added_requirement).bind(added_slot).fetch_one(&pool).await.unwrap());
    assert_eq!(actual, publication_actual_evidence(&pool).await);
    assert!(matches!(
        publications::discard_draft(&pool, id, request).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert!(publications::read_workspace(
        &pool,
        level,
        &CurriculumViewQuery {
            draft_id: Some(token),
            publication_id: None
        }
    )
    .await
    .is_err());
    let reopened = publications::open_draft(
        &pool,
        id,
        OpenCurriculumDraftRequest {
            row_version: restored.row_version,
        },
    )
    .await
    .unwrap();
    assert_ne!(reopened.draft_id, Some(token));
}

#[tokio::test]
async fn curriculum_publication_discard_refuses_changed_preview_and_unreleased_edition() {
    use school_academic_core::models::{DiscardCurriculumDraftRequest, OpenCurriculumDraftRequest};
    use school_academic_core::services::curriculum_publications as publications;
    let pool = prepare_current_core_fixture("curriculum_publication_discard_stale").await;
    let (id, level) = publication_fixture(&pool, "DISCARD-STALE").await;
    let edition = curriculum::get(&pool, id).await.unwrap();
    let draft = publications::open_draft(
        &pool,
        id,
        OpenCurriculumDraftRequest {
            row_version: edition.row_version,
        },
    )
    .await
    .unwrap();
    let token = draft.draft_id.unwrap();
    let preview = publications::preview_discard(&pool, id, token)
        .await
        .unwrap();
    sqlx::query("UPDATE curriculum_levels SET name_th='แก้หลังเปิดยืนยัน' WHERE id=$1")
        .bind(level)
        .execute(&pool)
        .await
        .unwrap();
    let request = DiscardCurriculumDraftRequest {
        draft_id: token,
        row_version: preview.row_version,
        content_hash: preview.content_hash,
    };
    assert!(matches!(
        publications::discard_draft(&pool, id, request).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert_eq!(
        curriculum::get(&pool, id).await.unwrap().draft_id,
        Some(token)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name_th FROM curriculum_levels WHERE id=$1")
            .bind(level)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "แก้หลังเปิดยืนยัน"
    );
    // Direct SQL cannot clear a token while leaving changed workspace rows behind.
    assert!(
        sqlx::query("UPDATE curriculum_editions SET draft_id=NULL WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .is_err()
    );
    let latest = publications::preview_discard(&pool, id, token)
        .await
        .unwrap();
    publications::discard_draft(
        &pool,
        id,
        DiscardCurriculumDraftRequest {
            draft_id: token,
            row_version: latest.row_version,
            content_hash: latest.content_hash,
        },
    )
    .await
    .unwrap();
    let never_published = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "ร่างใหม่ไม่มีข้อมูลคืน".into(),
            revision_year: 2570,
            description: None,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        publications::preview_discard(&pool, never_published.id, never_published.draft_id.unwrap())
            .await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn curriculum_publication_publish_and_discard_serialize_on_the_same_owner() {
    use school_academic_core::models::{DiscardCurriculumDraftRequest, OpenCurriculumDraftRequest};
    use school_academic_core::services::curriculum_publications as publications;
    let pool = prepare_current_core_fixture("curriculum_publication_discard_race").await;
    let (id, _) = publication_fixture(&pool, "DISCARD-RACE").await;
    let edition = curriculum::get(&pool, id).await.unwrap();
    let draft = publications::open_draft(
        &pool,
        id,
        OpenCurriculumDraftRequest {
            row_version: edition.row_version,
        },
    )
    .await
    .unwrap();
    let preview = publications::preview_discard(&pool, id, draft.draft_id.unwrap())
        .await
        .unwrap();
    let actor = fixture_actor(&pool).await;
    let (discard, publish) = tokio::join!(
        publications::discard_draft(
            &pool,
            id,
            DiscardCurriculumDraftRequest {
                draft_id: preview.draft_id,
                row_version: preview.row_version,
                content_hash: preview.content_hash
            }
        ),
        curriculum::publish(
            &pool,
            id,
            PublishCurriculumRequest {
                draft_id: preview.draft_id,
                row_version: preview.row_version,
                change_note: "เผยแพร่แข่งกับยกเลิก".into()
            },
            actor
        )
    );
    assert_ne!(discard.is_ok(), publish.is_ok());
    let expected_count = if publish.is_ok() { 2 } else { 1 };
    let failed = if discard.is_err() {
        discard.unwrap_err()
    } else {
        publish.unwrap_err()
    };
    assert!(matches!(failed, school_errors::AppError::Conflict(_)));
    let result = curriculum::get(&pool, id).await.unwrap();
    assert!(result.draft_id.is_none());
    assert_eq!(result.publication_count, expected_count);
    assert_eq!(
        publications::list_publications(&pool, id)
            .await
            .unwrap()
            .len(),
        expected_count as usize
    );
}

#[tokio::test]
async fn homeroom_roster_batches_preserve_history_and_reject_stale_or_invalid_writes() {
    use school_academic_core::models::*;
    use school_academic_core::services::homeroom_roster;
    let pool = prepare_current_core_fixture("homeroom_roster_management").await;
    apply_migrations_through(&pool, 99).await.unwrap();
    let actor = fixture_actor(&pool).await;
    let room: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut roster = homeroom_roster::get_roster(&pool, room).await.unwrap();
    assert!(!roster.students.is_empty());
    let other = Uuid::new_v4();
    sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity,is_active) SELECT $1,'ROSTER-TARGET','ทดสอบห้องใหม่',academic_year_id,grade_level_id,'ROSTER-TARGET',study_program_id,1,true FROM homerooms WHERE id=$2")
        .bind(other).bind(room).execute(&pool).await.unwrap();
    // Add two synthetic student-years whose names expose Thai leading-vowel ordering.
    let mut selections = Vec::new();
    for (name, gender) in [("เกียรติ", "male"), ("ขวัญ", "female")] {
        let user = Uuid::new_v4();
        let year = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status,gender) VALUES($1,$2,'synthetic-not-login',$3,'ทดสอบ','student','active',$4)")
            .bind(user).bind(format!("roster-{user}")).bind(name).bind(gender).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,$2,academic_year_id,grade_level_id,study_program_id,'active' FROM homerooms WHERE id=$3")
            .bind(year).bind(user).bind(room).execute(&pool).await.unwrap();
        selections.push(HomeroomRosterSelection {
            student_academic_year_id: year,
            student_year_row_version: 1,
            placement_id: None,
            placement_row_version: None,
        });
    }
    let date = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
    let before_count = roster.students.len();
    let add = MutateHomeroomRosterRequest {
        revision: roster.revision.clone(),
        action: HomeroomRosterAction::Add,
        selections: selections.clone(),
        effective_date: date,
        target_homeroom_id: None,
        reason: String::new(),
    };
    // An occupied student in the same batch rolls back the entire add.
    let mut invalid = add.clone();
    let existing = &roster.students[0];
    invalid.selections.push(HomeroomRosterSelection {
        student_academic_year_id: existing.student_academic_year_id,
        student_year_row_version: existing.student_year_row_version,
        placement_id: None,
        placement_row_version: None,
    });
    assert!(homeroom_roster::mutate_roster(&pool, actor, room, invalid)
        .await
        .is_err());
    assert_eq!(
        homeroom_roster::get_roster(&pool, room)
            .await
            .unwrap()
            .students
            .len(),
        before_count
    );
    roster = homeroom_roster::mutate_roster(&pool, actor, room, add.clone())
        .await
        .unwrap();
    assert_eq!(roster.students.len(), before_count + 2);
    assert!(homeroom_roster::mutate_roster(&pool, actor, room, add)
        .await
        .is_err());
    let preview = homeroom_roster::preview_numbers(
        &pool,
        room,
        HomeroomNumberingQuery {
            method: HomeroomNumberingMethod::Name,
            start_number: 1,
        },
    )
    .await
    .unwrap();
    let ordered: Vec<_> = preview
        .numbers
        .iter()
        .map(|n| {
            preview
                .roster
                .students
                .iter()
                .find(|s| s.placement_id == n.placement_id)
                .unwrap()
                .first_name
                .as_str()
        })
        .collect();
    assert!(
        ordered.iter().position(|name| *name == "เกียรติ").unwrap()
            < ordered.iter().position(|name| *name == "ขวัญ").unwrap()
    );
    let numbers = UpdateHomeroomNumbersRequest {
        revision: preview.roster.revision,
        numbers: preview.numbers,
    };
    roster = homeroom_roster::update_numbers(&pool, actor, room, numbers.clone())
        .await
        .unwrap();
    assert!(homeroom_roster::update_numbers(&pool, actor, room, numbers)
        .await
        .is_err());
    let people: Vec<_> = roster
        .students
        .iter()
        .filter(|s| {
            selections
                .iter()
                .any(|selection| selection.student_academic_year_id == s.student_academic_year_id)
        })
        .collect();
    let chosen: Vec<_> = people
        .iter()
        .map(|s| HomeroomRosterSelection {
            student_academic_year_id: s.student_academic_year_id,
            student_year_row_version: s.student_year_row_version,
            placement_id: Some(s.placement_id),
            placement_row_version: Some(s.row_version),
        })
        .collect();
    let move_request = MutateHomeroomRosterRequest {
        revision: roster.revision.clone(),
        action: HomeroomRosterAction::Transfer,
        selections: chosen.clone(),
        effective_date: NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
        target_homeroom_id: Some(other),
        reason: "ปรับการจัดห้องทดสอบ".into(),
    };
    assert!(
        homeroom_roster::mutate_roster(&pool, actor, room, move_request.clone())
            .await
            .is_err()
    );
    assert_eq!(
        homeroom_roster::get_roster(&pool, other)
            .await
            .unwrap()
            .students
            .len(),
        0
    );
    // Number collisions and wrong-room IDs fail before any write.
    assert!(homeroom_roster::update_numbers(
        &pool,
        actor,
        room,
        UpdateHomeroomNumbersRequest {
            revision: roster.revision.clone(),
            numbers: vec![HomeroomNumberInput {
                placement_id: people[0].placement_id,
                class_number: people[1].class_number.unwrap()
            }]
        }
    )
    .await
    .is_err());
    let untouched: Vec<_> = roster
        .students
        .iter()
        .filter(|s| {
            !chosen
                .iter()
                .any(|selection| selection.student_academic_year_id == s.student_academic_year_id)
        })
        .map(|s| (s.placement_id, s.class_number))
        .collect();
    sqlx::query("UPDATE homerooms SET capacity=40,row_version=row_version+1 WHERE id=$1")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    roster = homeroom_roster::mutate_roster(&pool, actor, room, move_request)
        .await
        .unwrap();
    assert_eq!(roster.students.len(), before_count);
    for (id, number) in untouched {
        assert_eq!(
            roster
                .students
                .iter()
                .find(|s| s.placement_id == id)
                .unwrap()
                .class_number,
            number
        );
    }
    let target = homeroom_roster::get_roster(&pool, other).await.unwrap();
    assert_eq!(target.students.len(), 2);
    assert_eq!(
        target
            .students
            .iter()
            .map(|s| s.class_number.unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let old: Vec<(String, NaiveDate)> =
        sqlx::query_as("SELECT status,end_date FROM homeroom_placements WHERE id=ANY($1)")
            .bind(
                chosen
                    .iter()
                    .filter_map(|s| s.placement_id)
                    .collect::<Vec<_>>(),
            )
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(old.iter().all(|(status, date)| status == "ended"
        && *date == NaiveDate::from_ymd_opt(2025, 6, 30).unwrap()));
    let remove = MutateHomeroomRosterRequest {
        revision: target.revision.clone(),
        action: HomeroomRosterAction::Remove,
        selections: target
            .students
            .iter()
            .map(|s| HomeroomRosterSelection {
                student_academic_year_id: s.student_academic_year_id,
                student_year_row_version: s.student_year_row_version,
                placement_id: Some(s.placement_id),
                placement_row_version: Some(s.row_version),
            })
            .collect(),
        effective_date: NaiveDate::from_ymd_opt(2025, 8, 1).unwrap(),
        target_homeroom_id: None,
        reason: "จัดห้องใหม่ภายหลัง".into(),
    };
    assert!(homeroom_roster::mutate_roster(&pool, actor, other, remove)
        .await
        .unwrap()
        .students
        .is_empty());
    let candidate_ids: Vec<_> = homeroom_roster::list_candidates(
        &pool,
        room,
        HomeroomCandidateQuery {
            search: Some("ทดสอบ".into()),
        },
    )
    .await
    .unwrap()
    .into_iter()
    .map(|c| c.student_academic_year_id)
    .collect();
    assert!(selections
        .iter()
        .all(|s| candidate_ids.contains(&s.student_academic_year_id)));
    // Cancelling a future planned placement keeps its identity without inventing attendance.
    let planned_year = selections[0].student_academic_year_id;
    sqlx::query(
        "UPDATE student_academic_years SET status='planned',row_version=row_version+1 WHERE id=$1",
    )
    .bind(planned_year)
    .execute(&pool)
    .await
    .unwrap();
    let version: i64 =
        sqlx::query_scalar("SELECT row_version FROM student_academic_years WHERE id=$1")
            .bind(planned_year)
            .fetch_one(&pool)
            .await
            .unwrap();
    let empty_target = homeroom_roster::get_roster(&pool, other).await.unwrap();
    let planned = homeroom_roster::mutate_roster(
        &pool,
        actor,
        other,
        MutateHomeroomRosterRequest {
            revision: empty_target.revision,
            action: HomeroomRosterAction::Add,
            selections: vec![HomeroomRosterSelection {
                student_academic_year_id: planned_year,
                student_year_row_version: version,
                placement_id: None,
                placement_row_version: None,
            }],
            effective_date: NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(),
            target_homeroom_id: None,
            reason: String::new(),
        },
    )
    .await
    .unwrap();
    let future = &planned.students[0];
    let cancelled = future.placement_id;
    homeroom_roster::mutate_roster(
        &pool,
        actor,
        other,
        MutateHomeroomRosterRequest {
            revision: planned.revision.clone(),
            action: HomeroomRosterAction::Remove,
            selections: vec![HomeroomRosterSelection {
                student_academic_year_id: planned_year,
                student_year_row_version: future.student_year_row_version,
                placement_id: Some(cancelled),
                placement_row_version: Some(future.row_version),
            }],
            effective_date: NaiveDate::from_ymd_opt(2025, 8, 15).unwrap(),
            target_homeroom_id: None,
            reason: "ยกเลิกแผนก่อนเริ่มเรียน".into(),
        },
    )
    .await
    .unwrap();
    let retained: (String, Option<NaiveDate>) =
        sqlx::query_as("SELECT status,end_date FROM homeroom_placements WHERE id=$1")
            .bind(cancelled)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retained, ("cancelled".into(), None));
    // Two staff saving the same snapshot cannot overwrite each other.
    let competing = |number| UpdateHomeroomNumbersRequest {
        revision: roster.revision.clone(),
        numbers: vec![HomeroomNumberInput {
            placement_id: roster.students[0].placement_id,
            class_number: number,
        }],
    };
    let (a, b) = tokio::join!(
        homeroom_roster::update_numbers(&pool, actor, room, competing(100)),
        homeroom_roster::update_numbers(&pool, actor, room, competing(101))
    );
    assert_ne!(a.is_ok(), b.is_ok());
    roster = homeroom_roster::get_roster(&pool, room).await.unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(CURRENT_YEAR_ID)
        .execute(&pool)
        .await
        .unwrap();
    assert!(homeroom_roster::get_roster(&pool, room).await.is_ok());
    assert!(homeroom_roster::update_numbers(
        &pool,
        actor,
        room,
        UpdateHomeroomNumbersRequest {
            revision: roster.revision,
            numbers: vec![HomeroomNumberInput {
                placement_id: roster.students[0].placement_id,
                class_number: 100
            }]
        }
    )
    .await
    .is_err());
}
