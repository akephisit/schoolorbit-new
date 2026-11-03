use chrono::{DateTime, Duration, Utc};
use school_academic_lifecycle::{
    models::*,
    services::retention::{self, AcademicRetentionPort, RetentionJob},
};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::{PgConnection, PgPool};
use std::sync::Mutex;
use uuid::Uuid;

const REQUIRED: [&str; 4] = [
    codes::ACADEMIC_LIFECYCLE_READ_SCHOOL,
    codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
    codes::ATTENDANCE_DELETE_SCHOOL,
    codes::BEHAVIOR_CLEANUP_EXECUTE_SCHOOL,
];

#[derive(Default)]
struct RecordingPort {
    advanced: Mutex<Vec<RetentionJob>>,
}
#[async_trait::async_trait]
impl AcademicRetentionPort for RecordingPort {
    async fn impact(
        &self,
        _: &mut PgConnection,
        _: Uuid,
    ) -> Result<AcademicRetentionImpact, AppError> {
        Ok(AcademicRetentionImpact::default())
    }
    async fn advance(&self, _: &PgPool, job: &RetentionJob) -> Result<(), AppError> {
        self.advanced.lock().unwrap().push(job.clone());
        Ok(())
    }
}

struct Fixture {
    pool: PgPool,
    actor: ActorContext,
    role: Uuid,
}
async fn fixture(name: &str) -> Fixture {
    let pool = school_test_db::create_named_test_pool(name).await;
    school_test_db::run_test_migrations(&pool).await;
    let actor = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'!','Retention','Fixture','staff','active')")
        .bind(actor).bind(actor.to_string()).execute(&pool).await.unwrap();
    let role: Uuid = sqlx::query_scalar("INSERT INTO roles(code,name,user_type,is_active) VALUES('RETENTION_FIXTURE','Retention fixture','staff',true) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=ANY($2)")
        .bind(role).bind(REQUIRED.to_vec()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(actor)
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    Fixture {
        pool,
        actor: ActorContext {
            user_id: actor,
            permissions: REQUIRED.iter().map(|p| p.to_string()).collect(),
        },
        role,
    }
}
fn update(version: i64, enabled: bool, days: i32) -> UpdateAcademicRetentionPolicy {
    UpdateAcademicRetentionPolicy {
        expected_row_version: version,
        enabled,
        primary_boundary: AcademicRetentionBoundary::Year,
        secondary_boundary: AcademicRetentionBoundary::Term,
        grace_days: days,
    }
}
async fn enable_historical_fixture(f: &Fixture) {
    // Synthetic history only: production policy saves always set their own enable timestamp.
    sqlx::query("UPDATE academic_retention_policy SET enabled=true,effective_from=now()-interval '200 days',updated_by=$1")
        .bind(f.actor.user_id).execute(&f.pool).await.unwrap();
}
async fn academic(f: &Fixture, number: i32, kind: &str) -> (Uuid, Uuid, Uuid) {
    let year: Uuid = sqlx::query_scalar("INSERT INTO academic_years(year,name,status) VALUES($1,'Retention fixture','planning') RETURNING id")
        .bind(number).fetch_one(&f.pool).await.unwrap();
    let bell: Uuid = sqlx::query_scalar("INSERT INTO bell_schedules(id,academic_year_id,code,name,is_default) VALUES(uuidv7(),$1,'RETENTION','Retention',true) RETURNING id")
        .bind(year).fetch_one(&f.pool).await.unwrap();
    let grade: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE level_type=$1 ORDER BY year,id LIMIT 1",
    )
    .bind(kind)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    let edition: Uuid = sqlx::query_scalar("INSERT INTO curriculum_editions(id,name,revision_year) VALUES(uuidv7(),'Retention',2999) RETURNING id")
        .fetch_one(&f.pool).await.unwrap();
    let level: Uuid = sqlx::query_scalar("INSERT INTO curriculum_levels(id,edition_id,code,name_th,grade_level_ids,is_active) VALUES(uuidv7(),$1,'RETENTION','Retention',$2,true) RETURNING id")
        .bind(edition).bind(sqlx::types::Json(vec![grade])).fetch_one(&f.pool).await.unwrap();
    let program: Uuid = sqlx::query_scalar("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status) VALUES(uuidv7(),$1,'RETENTION','Retention','draft') RETURNING id")
        .bind(level).fetch_one(&f.pool).await.unwrap();
    let student = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'!','Student','Fixture','student','active')")
        .bind(student).bind(student.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES(uuidv7(),$1,$2,$3,$4,'completed')")
        .bind(student).bind(year).bind(grade).bind(program).execute(&f.pool).await.unwrap();
    let term = add_term(f, year, bell, 1, Utc::now() - Duration::days(100)).await;
    (year, bell, term)
}
async fn add_term(
    f: &Fixture,
    year: Uuid,
    bell: Uuid,
    sequence: i32,
    closed: DateTime<Utc>,
) -> Uuid {
    let term: Uuid = sqlx::query_scalar("INSERT INTO academic_terms(school_days, academic_year_id,sequence_no,code,name,term_type,bell_schedule_id,status,included_in_year_result,blocks_year_closure) VALUES( 'MON,TUE,WED,THU,FRI',$1,$2,$3,'Retention term','regular',$4,'closed',true,true) RETURNING id")
        .bind(year).bind(sequence).bind(format!("T{sequence}")).bind(bell).fetch_one(&f.pool).await.unwrap();
    close_receipt(f, year, term, closed).await;
    term
}
async fn close_receipt(f: &Fixture, year: Uuid, term: Uuid, closed: DateTime<Utc>) {
    sqlx::query("INSERT INTO academic_term_transition_receipts(request_id,academic_year_id,academic_term_id,actor_user_id,action,request_checksum,accepted_readiness,outcome,canonical_request,created_at) VALUES(uuidv7(),$1,$2,$3,'close',repeat('a',64),'{}','{}','{}',$4)")
        .bind(year).bind(term).bind(f.actor.user_id).bind(closed).execute(&f.pool).await.unwrap();
}

#[tokio::test]
async fn retention_policy_enforces_exact_permissions_revisions_limits_and_enable_epoch() {
    let f = fixture("retention_policy_contract").await;
    let mut connection = f.pool.acquire().await.unwrap();
    let original = retention::policy(&mut connection).await.unwrap();
    assert!(!original.enabled);
    assert_eq!(original.grace_days, 90);
    assert_eq!(original.effective_from, None);
    drop(connection);
    for missing in REQUIRED {
        let denied = ActorContext {
            user_id: f.actor.user_id,
            permissions: REQUIRED
                .iter()
                .filter(|p| **p != missing)
                .map(|p| p.to_string())
                .collect(),
        };
        assert!(matches!(
            retention::update_policy(&f.pool, &denied, update(1, true, 90)).await,
            Err(AppError::Forbidden(_))
        ));
    }
    for days in [0, 3651] {
        assert!(matches!(
            retention::update_policy(&f.pool, &f.actor, update(1, true, days)).await,
            Err(AppError::ValidationError(_))
        ));
    }
    let first = retention::update_policy(&f.pool, &f.actor, update(1, true, 1))
        .await
        .unwrap();
    assert_eq!(first.row_version, 2);
    assert!(first.effective_from.is_some());
    assert!(matches!(
        retention::update_policy(&f.pool, &f.actor, update(1, true, 90)).await,
        Err(AppError::Conflict(_))
    ));
    let edited = retention::update_policy(&f.pool, &f.actor, update(2, true, 3650))
        .await
        .unwrap();
    assert_eq!(first.effective_from, edited.effective_from);
    let disabled = retention::update_policy(&f.pool, &f.actor, update(3, false, 90))
        .await
        .unwrap();
    assert_eq!(disabled.effective_from, first.effective_from);
    let enabled = retention::update_policy(&f.pool, &f.actor, update(4, true, 90))
        .await
        .unwrap();
    assert!(enabled.effective_from.unwrap() > first.effective_from.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_retention_jobs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn retention_scans_past_sixty_four_blocked_periods_and_checks_live_authorizer() {
    let f = fixture("retention_cursor_authority").await;
    enable_historical_fixture(&f).await;
    let (primary, bell, _) = academic(&f, 2997, "primary").await;
    for sequence in 2..=64 {
        add_term(
            &f,
            primary,
            bell,
            sequence,
            Utc::now() - Duration::days(100),
        )
        .await;
    }
    let (_, _, eligible) = academic(&f, 2998, "secondary").await;
    let port = RecordingPort::default();
    sqlx::query("DELETE FROM role_permissions WHERE role_id=$1 AND permission_id=(SELECT id FROM permissions WHERE code=$2)")
        .bind(f.role).bind(codes::ATTENDANCE_DELETE_SCHOOL).execute(&f.pool).await.unwrap();
    retention::run_due(&port, &f.pool, Utc::now())
        .await
        .unwrap();
    assert!(port.advanced.lock().unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_retention_jobs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2")
        .bind(f.role).bind(codes::ATTENDANCE_DELETE_SCHOOL).execute(&f.pool).await.unwrap();
    retention::run_due(&port, &f.pool, Utc::now())
        .await
        .unwrap();
    assert!(
        port.advanced.lock().unwrap().is_empty(),
        "first 64 primary periods must wait for year closure"
    );
    let cursor: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT after_closed_at FROM academic_retention_scan_state")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert!(cursor.is_some());
    retention::run_due(&port, &f.pool, Utc::now())
        .await
        .unwrap();
    let jobs = port.advanced.lock().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].academic_term_id, eligible);
}

#[tokio::test]
async fn retention_reopen_reclose_invalidates_unstarted_job_and_restarts_grace() {
    let f = fixture("retention_reclose_deadline").await;
    enable_historical_fixture(&f).await;
    let (year, _, term) = academic(&f, 2997, "secondary").await;
    let port = RecordingPort::default();
    retention::run_due(&port, &f.pool, Utc::now())
        .await
        .unwrap();
    let old = port.advanced.lock().unwrap()[0].id;
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(term)
        .execute(&f.pool)
        .await
        .unwrap();
    retention::run_due(&port, &f.pool, Utc::now())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM academic_retention_jobs WHERE id=$1")
            .bind(old)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "cancelled"
    );
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(term)
        .execute(&f.pool)
        .await
        .unwrap();
    let reclosed = Utc::now() - Duration::days(10);
    close_receipt(&f, year, term, reclosed).await;
    retention::run_due(&port, &f.pool, Utc::now())
        .await
        .unwrap();
    assert_eq!(port.advanced.lock().unwrap().len(), 1);
    let workspace = retention::get_workspace(&port, &f.pool, &f.actor, year)
        .await
        .unwrap();
    assert_eq!(
        workspace.terms[0].closed_at.map(|v| v.timestamp_micros()),
        Some(reclosed.timestamp_micros())
    );
    assert!(workspace.terms[0]
        .blocked_reasons
        .contains(&"grace_period".into()));
    assert!(!workspace.terms[0].eligible);
}

#[tokio::test]
async fn retention_mixed_and_unknown_rosters_are_conservative_and_preview_never_mutates() {
    let f = fixture("retention_scope_preview").await;
    enable_historical_fixture(&f).await;
    let (year, _, _) = academic(&f, 2997, "primary").await;
    let extra_student = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) VALUES($1,'!','Extra student','Fixture','student','active')").bind(extra_student).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT uuidv7(),$1,$2,(SELECT id FROM grade_levels WHERE level_type='secondary' ORDER BY year LIMIT 1),study_program_id,'graduated' FROM student_academic_years WHERE academic_year_id=$2 LIMIT 1")
        .bind(extra_student).bind(year).execute(&f.pool).await.unwrap();
    let port = RecordingPort::default();
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_retention_jobs")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let mixed = retention::get_workspace(&port, &f.pool, &f.actor, year)
        .await
        .unwrap();
    assert_eq!(mixed.terms[0].level_types, vec!["primary", "secondary"]);
    assert_eq!(
        mixed.terms[0].required_boundary,
        AcademicRetentionBoundary::Year
    );
    assert!(mixed.terms[0]
        .blocked_reasons
        .contains(&"year_not_closed".into()));
    sqlx::query("UPDATE student_academic_years SET grade_level_id=(SELECT id FROM grade_levels WHERE level_type='kindergarten' ORDER BY year LIMIT 1) WHERE student_id=$1")
        .bind(extra_student).execute(&f.pool).await.unwrap();
    let unknown = retention::get_workspace(&port, &f.pool, &f.actor, year)
        .await
        .unwrap();
    assert!(unknown.terms[0]
        .blocked_reasons
        .contains(&"education_level_unknown".into()));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_retention_jobs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        before
    );
}
