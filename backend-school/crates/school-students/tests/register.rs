use chrono::{DateTime, NaiveDate, Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_students::access_policy::{authorize_student_profile_read, resolve_student_list_access};
use school_students::models::{
    CreateStudentRequest, ListStudentsQuery, StudentRegistrationStatus as Status,
    UpdateStudentRequest,
};
use school_students::services::{self, StudentListAccess};
use school_test_db::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn empty_fixture(name: &str) -> (PgPool, Uuid) {
    let pool = school_test_db::create_named_test_pool(name).await;
    school_test_db::run_test_migrations(&pool).await;
    let actor = insert_user(&pool, "Registrar", "staff", "active").await;
    (pool, actor)
}

async fn insert_user(pool: &PgPool, name: &str, kind: &str, status: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'not-a-login',$3,'Fixture',$4,$5)")
        .bind(id).bind(format!("fixture-{id}")).bind(name).bind(kind).bind(status)
        .execute(pool).await.unwrap();
    id
}

async fn insert_student(pool: &PgPool, name: &str, status: &str) -> Uuid {
    let id = insert_user(pool, name, "student", status).await;
    sqlx::query("INSERT INTO student_info(user_id,student_id) VALUES($1,$2)")
        .bind(id)
        .bind(format!("S-{id}"))
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn academic_fixture(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 117).await.unwrap();
    pool
}

fn actor(id: Uuid, permissions: &[&str]) -> ActorContext {
    ActorContext {
        user_id: id,
        permissions: permissions.iter().map(|code| code.to_string()).collect(),
    }
}

#[tokio::test]
async fn migration_preserves_unknown_history_and_defaults_all_future_registrations_pending() {
    let pool = school_test_db::create_named_test_pool("student_register_migration").await;
    apply_migrations_through(&pool, 116).await.unwrap();
    let existing = insert_student(&pool, "Historical", "inactive").await;
    sqlx::query(
        "UPDATE student_info SET enrollment_date='2020-05-01',student_number=9 WHERE user_id=$1",
    )
    .bind(existing)
    .execute(&pool)
    .await
    .unwrap();
    let before: (Uuid, String, NaiveDate, i32) = sqlx::query_as(
        "SELECT id,student_id,enrollment_date,student_number FROM student_info WHERE user_id=$1",
    )
    .bind(existing)
    .fetch_one(&pool)
    .await
    .unwrap();
    apply_migrations_through(&pool, 117).await.unwrap();
    let after: (Uuid, String, NaiveDate, i32) = sqlx::query_as(
        "SELECT id,student_id,enrollment_date,student_number FROM student_info WHERE user_id=$1",
    )
    .bind(existing)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    let historical: (String, Option<DateTime<Utc>>, Option<Uuid>) = sqlx::query_as("SELECT registration_status,registration_status_updated_at,registration_status_updated_by FROM student_info WHERE user_id=$1")
        .bind(existing).fetch_one(&pool).await.unwrap();
    assert_eq!(historical, ("unrecorded".into(), None, None));
    // Admission's existing insert omits registration_status, so the database owns this default.
    let future = insert_student(&pool, "New", "active").await;
    let status: String =
        sqlx::query_scalar("SELECT registration_status FROM student_info WHERE user_id=$1")
            .bind(future)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
    assert!(
        sqlx::query("UPDATE student_info SET registration_status='invented' WHERE user_id=$1")
            .bind(existing)
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn permanent_register_includes_unenrolled_inactive_students_and_sorts_before_pagination() {
    let (pool, _) = empty_fixture("student_register_list").await;
    let a = insert_student(&pool, "REG-A", "inactive").await;
    let b = insert_student(&pool, "REG-B", "active").await;
    let c = insert_student(&pool, "REG-C", "suspended").await;
    let query = |page| ListStudentsQuery {
        search: Some("REG-".into()),
        page: Some(page),
        page_size: Some(2),
        ..Default::default()
    };
    let first = services::list_students(&pool, query(1), StudentListAccess::School)
        .await
        .unwrap();
    let second = services::list_students(&pool, query(2), StudentListAccess::School)
        .await
        .unwrap();
    assert_eq!(
        first.items.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![a, b]
    );
    assert_eq!(
        second.items.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![c]
    );
    assert_eq!(first.items[0].account_status, "inactive");
    assert_eq!(first.items[0].registration_status, Status::Pending);
    assert!(first.items[0].enrollment_date.is_none());
    let filtered = services::list_students(
        &pool,
        ListStudentsQuery {
            search: Some("REG-".into()),
            account_status: Some("inactive".into()),
            registration_status: Some(Status::Pending),
            ..Default::default()
        },
        StudentListAccess::School,
    )
    .await
    .unwrap();
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].id, a);
    let own = services::list_students(
        &pool,
        ListStudentsQuery::default(),
        StudentListAccess::Own(b),
    )
    .await
    .unwrap();
    assert_eq!(
        own.items.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![b]
    );
    let partial = insert_user(&pool, "PartialRegistry", "student", "active").await;
    sqlx::query("UPDATE users SET username=NULL WHERE id=$1")
        .bind(partial)
        .execute(&pool)
        .await
        .unwrap();
    let partial_rows = services::list_students(
        &pool,
        ListStudentsQuery {
            search: Some("PartialRegistry".into()),
            ..Default::default()
        },
        StudentListAccess::School,
    )
    .await
    .unwrap();
    assert_eq!(partial_rows.items.len(), 1);
    assert_eq!(partial_rows.items[0].id, partial);
    assert!(partial_rows.items[0].username.is_none());
    assert!(partial_rows.items[0].student_id.is_none());
    assert_eq!(
        partial_rows.items[0].registration_status,
        Status::Unrecorded
    );
    let partial_profile = services::get_student(&pool, partial, None, false)
        .await
        .unwrap();
    assert!(partial_profile.info.username.is_none());
    let partial_update = serde_json::from_value(
        json!({"first_name":"must-roll-back", "registration_status":"studying"}),
    )
    .unwrap();
    assert!(matches!(
        services::update_student(&pool, partial, partial, partial_update).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        services::get_student(&pool, partial, None, false)
            .await
            .unwrap()
            .info
            .first_name,
        "PartialRegistry"
    );

    let first_tie = insert_student(&pool, "TIE", "active").await;
    let second_tie = insert_student(&pool, "TIE", "active").await;
    let mut expected_ties = vec![first_tie, second_tie];
    expected_ties.sort();
    let mut actual_ties = Vec::new();
    for page in [1, 2] {
        let rows = services::list_students(
            &pool,
            ListStudentsQuery {
                search: Some("TIE".into()),
                page: Some(page),
                page_size: Some(1),
                ..Default::default()
            },
            StudentListAccess::School,
        )
        .await
        .unwrap();
        actual_ties.push(rows.items[0].id);
    }
    assert_eq!(actual_ties, expected_ties);
    for query in [
        ListStudentsQuery {
            page: Some(0),
            ..Default::default()
        },
        ListStudentsQuery {
            page_size: Some(-1),
            ..Default::default()
        },
        ListStudentsQuery {
            page: Some(i64::MAX),
            ..Default::default()
        },
        ListStudentsQuery {
            account_status: Some("invented".into()),
            ..Default::default()
        },
    ] {
        assert!(matches!(
            services::list_students(&pool, query, StudentListAccess::School).await,
            Err(AppError::BadRequest(_))
        ));
    }
}

#[tokio::test]
async fn assigned_register_matches_profile_scope_and_unions_own_access() {
    let pool = academic_fixture("student_register_scopes").await;
    let assigned_student = Uuid::from_u128(0x50000000000000000000000000000001);
    let advisor = insert_student(&pool, "AdvisorOwn", "active").await;
    let unrelated = insert_student(&pool, "Unrelated", "active").await;
    sqlx::query("INSERT INTO homeroom_advisors(homeroom_id,user_id,role) SELECT homeroom_id,$1,'primary' FROM homeroom_placements WHERE status='current' LIMIT 1")
        .bind(advisor).execute(&pool).await.unwrap();
    let advisor_actor = actor(
        advisor,
        &[codes::STUDENT_READ_ASSIGNED, codes::STUDENT_READ_OWN],
    );
    let access = resolve_student_list_access(&advisor_actor).unwrap();
    let listed = services::list_students(&pool, ListStudentsQuery::default(), access)
        .await
        .unwrap();
    let ids: Vec<_> = listed.items.iter().map(|row| row.id).collect();
    assert!(ids.contains(&assigned_student));
    assert!(ids.contains(&advisor));
    assert!(!ids.contains(&unrelated));
    assert!(
        !authorize_student_profile_read(&pool, &advisor_actor, assigned_student)
            .await
            .unwrap()
    );
    assert!(authorize_student_profile_read(
        &pool,
        &actor(
            advisor,
            &[
                codes::STUDENT_READ_ASSIGNED,
                codes::STUDENT_PII_READ_ASSIGNED
            ]
        ),
        assigned_student
    )
    .await
    .unwrap());
    assert!(authorize_student_profile_read(
        &pool,
        &actor(unrelated, &[codes::STUDENT_READ_ASSIGNED]),
        assigned_student
    )
    .await
    .is_err());
    assert!(
        resolve_student_list_access(&actor(unrelated, &[codes::STUDENT_PII_READ_SCHOOL])).is_err()
    );
    // Ended placement and planned student-year must not grant an advisor profile or list scope.
    sqlx::query(
        "UPDATE homeroom_placements SET status='ended',end_date=start_date WHERE status='current'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let ended = services::list_students(
        &pool,
        ListStudentsQuery::default(),
        StudentListAccess::Assigned(advisor),
    )
    .await
    .unwrap();
    assert!(!ended.items.iter().any(|row| row.id == assigned_student));
    assert!(
        authorize_student_profile_read(&pool, &advisor_actor, assigned_student)
            .await
            .is_err()
    );
    sqlx::query("UPDATE homeroom_placements SET status='current',end_date=NULL WHERE student_academic_year_id IN (SELECT id FROM student_academic_years WHERE student_id=$1 AND status='active')")
        .bind(assigned_student).execute(&pool).await.unwrap();
    sqlx::query("UPDATE student_academic_years SET status='planned' WHERE student_id=$1")
        .bind(assigned_student)
        .execute(&pool)
        .await
        .unwrap();
    assert!(services::list_students(
        &pool,
        ListStudentsQuery::default(),
        StudentListAccess::Assigned(advisor)
    )
    .await
    .unwrap()
    .items
    .is_empty());
}

#[tokio::test]
async fn staff_profile_projects_only_requested_year_and_redacts_failed_protected_fields() {
    let pool = academic_fixture("student_register_profile").await;
    let student = Uuid::from_u128(0x50000000000000000000000000000001);
    let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE year=2025")
        .fetch_one(&pool)
        .await
        .unwrap();
    let next_year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE year=2026")
        .fetch_one(&pool)
        .await
        .unwrap();
    let permanent = services::get_student(&pool, student, None, false)
        .await
        .unwrap();
    assert_eq!(permanent.info.registration_status, Status::Unrecorded);
    assert!(permanent.info.grade_level.is_none());
    assert!(permanent.info.homeroom.is_none());
    assert!(permanent.info.student_number.is_none());
    assert_eq!(
        services::get_student(&pool, student, Some(year), false)
            .await
            .unwrap()
            .info
            .homeroom
            .as_deref(),
        Some("ม.1/1 ปี 2025")
    );
    assert_eq!(
        services::get_student(&pool, student, Some(next_year), false)
            .await
            .unwrap()
            .info
            .homeroom
            .as_deref(),
        Some("ม.1/1 ปี 2026")
    );
    let absent = services::get_student(&pool, student, Some(Uuid::new_v4()), false)
        .await
        .unwrap();
    assert!(absent.info.grade_level.is_none());
    assert!(absent.info.homeroom.is_none());
    let unenrolled = insert_student(&pool, "Unenrolled", "inactive").await;
    sqlx::query("UPDATE users SET national_id='invalid-encrypted-value' WHERE id=$1")
        .bind(unenrolled)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE student_info SET medical_conditions='invalid-encrypted-value',blood_type='O',allergies='synthetic' WHERE user_id=$1").bind(unenrolled).execute(&pool).await.unwrap();
    let hidden = services::get_student(&pool, unenrolled, None, false)
        .await
        .unwrap();
    assert!(hidden.info.national_id.is_none() && hidden.info.medical_conditions.is_none());
    assert!(hidden.info.blood_type.is_none() && hidden.info.allergies.is_none());
    let failed = services::get_student(&pool, unenrolled, None, true)
        .await
        .unwrap();
    assert!(failed.info.national_id.is_none() && failed.info.medical_conditions.is_none());
    assert_eq!(failed.info.account_status, "inactive");
    assert!(services::get_student(&pool, Uuid::new_v4(), None, false)
        .await
        .is_err());
}

#[tokio::test]
async fn administrative_status_edits_preserve_accounts_annual_history_and_omitted_values() {
    let pool = academic_fixture("student_register_update").await;
    let student = Uuid::from_u128(0x50000000000000000000000000000001);
    let registrar = insert_user(&pool, "Registrar", "staff", "active").await;
    let before: String = sqlx::query_scalar("SELECT jsonb_build_object('years',(SELECT jsonb_agg(to_jsonb(y) ORDER BY id) FROM student_academic_years y WHERE student_id=$1),'placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM homeroom_placements p JOIN student_academic_years y ON y.id=p.student_academic_year_id WHERE y.student_id=$1),'memberships',(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM learning_group_students m WHERE m.student_id=$1))::text")
        .bind(student).fetch_one(&pool).await.unwrap();
    let mut input: UpdateStudentRequest = serde_json::from_value(json!({
        "first_name":"  Revised  ","title":"เด็กหญิง","email":"test@example.invalid", "phone":"0812345678", "address":"Updated address",
        "date_of_birth":"2012-02-29", "gender":"female", "enrollment_date":"2020-05-01", "registration_status":"transferred_out"
    })).unwrap();
    services::update_student(&pool, student, registrar, input)
        .await
        .unwrap();
    let profile = services::get_student(&pool, student, None, false)
        .await
        .unwrap();
    assert_eq!(profile.info.first_name, "Revised");
    assert_eq!(profile.info.account_status, "active");
    assert_eq!(profile.info.registration_status, Status::TransferredOut);
    assert_eq!(profile.info.registration_status_updated_by, Some(registrar));
    let audit = profile.info.registration_status_updated_at;
    assert!(audit.is_some());
    assert_eq!(
        profile.info.date_of_birth,
        NaiveDate::from_ymd_opt(2012, 2, 29)
    );
    assert_eq!(
        profile.info.enrollment_date,
        NaiveDate::from_ymd_opt(2020, 5, 1)
    );
    input = serde_json::from_value(json!({"registration_status":"transferred_out"})).unwrap();
    services::update_student(&pool, student, registrar, input)
        .await
        .unwrap();
    services::update_student(&pool, student, registrar, UpdateStudentRequest::default())
        .await
        .unwrap();
    let unchanged = services::get_student(&pool, student, None, false)
        .await
        .unwrap();
    assert_eq!(unchanged.info.registration_status_updated_at, audit);
    assert_eq!(unchanged.info.phone.as_deref(), Some("0812345678"));
    let clear = serde_json::from_value(json!({"title":null,"email":null,"phone":null,"address":null,"date_of_birth":null,"gender":null,"enrollment_date":null})).unwrap();
    services::update_student(&pool, student, registrar, clear)
        .await
        .unwrap();
    let cleared = services::get_student(&pool, student, None, false)
        .await
        .unwrap();
    assert!(
        cleared.info.title.is_none()
            && cleared.info.email.is_none()
            && cleared.info.phone.is_none()
    );
    assert!(
        cleared.info.address.is_none()
            && cleared.info.date_of_birth.is_none()
            && cleared.info.gender.is_none()
            && cleared.info.enrollment_date.is_none()
    );
    let reentry = serde_json::from_value(json!({"registration_status":"studying"})).unwrap();
    services::update_student(&pool, student, registrar, reentry)
        .await
        .unwrap();
    let after: String = sqlx::query_scalar("SELECT jsonb_build_object('years',(SELECT jsonb_agg(to_jsonb(y) ORDER BY id) FROM student_academic_years y WHERE student_id=$1),'placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM homeroom_placements p JOIN student_academic_years y ON y.id=p.student_academic_year_id WHERE y.student_id=$1),'memberships',(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM learning_group_students m WHERE m.student_id=$1))::text")
        .bind(student).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let invalid =
        serde_json::from_value(json!({"first_name":"must-roll-back","email":"invalid"})).unwrap();
    assert!(services::update_student(&pool, student, registrar, invalid)
        .await
        .is_err());
    let invalid_actor_update = serde_json::from_value(
        json!({"first_name":"must-roll-back", "registration_status":"withdrawn"}),
    )
    .unwrap();
    assert!(
        services::update_student(&pool, student, Uuid::new_v4(), invalid_actor_update)
            .await
            .is_err()
    );
    assert_eq!(
        services::get_student(&pool, student, None, false)
            .await
            .unwrap()
            .info
            .first_name,
        "Revised"
    );
}

#[tokio::test]
async fn manual_create_is_immediately_visible_and_duplicate_creation_rolls_back() {
    let (pool, registrar) = empty_fixture("student_register_create").await;
    let input = || {
        serde_json::from_value::<CreateStudentRequest>(json!({
        "first_name":"  New  ","last_name":"Student", "student_id":"REG-NEW", "password":"Valid-password",
        "phone":"0812345678", "address":"New address", "enrollment_date":"2026-05-01"
    })).unwrap()
    };
    let created = services::create_student(&pool, registrar, input())
        .await
        .unwrap();
    let profile = services::get_student(&pool, created.student.id, None, false)
        .await
        .unwrap();
    assert_eq!(profile.info.first_name, "New");
    assert_eq!(profile.info.registration_status, Status::Pending);
    assert_eq!(profile.info.registration_status_updated_by, Some(registrar));
    assert!(profile.info.homeroom.is_none());
    assert_eq!(profile.info.phone.as_deref(), Some("0812345678"));
    let listed = services::list_students(
        &pool,
        ListStudentsQuery {
            search: Some("REG-NEW".into()),
            ..Default::default()
        },
        StudentListAccess::School,
    )
    .await
    .unwrap();
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].id, created.student.id);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(services::create_student(&pool, registrar, input())
        .await
        .is_err());
    let mut duplicate_code = input();
    duplicate_code.username = Some(format!("different-{}", Uuid::new_v4()));
    assert!(services::create_student(&pool, registrar, duplicate_code)
        .await
        .is_err());

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users")
            .fetch_one(&pool)
            .await
            .unwrap(),
        count
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM student_academic_years WHERE student_id=$1"
        )
        .bind(created.student.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
