use school_errors::AppError;
use school_test_db::{create_test_pool, run_test_migrations};
use uuid::Uuid;

use school_students::models::{CreateParentRequest, CreateStudentRequest, UpdateStudentRequest};
use school_students::services;

#[tokio::test]
async fn update_missing_student_returns_not_found() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let result = services::update_student(
        &pool,
        Uuid::new_v4(),
        Uuid::new_v4(),
        UpdateStudentRequest {
            email: None,
            first_name: Some("Missing".to_string()),
            last_name: None,
            phone: None,
            address: None,
            ..Default::default()
        },
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn delete_missing_student_returns_not_found() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let result = services::delete_student(
        &pool,
        Uuid::new_v4(),
        Uuid::new_v4(),
        &crate::http_state::PLACEMENT_ROSTERS,
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn add_parent_to_missing_student_returns_not_found_without_creating_parent() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let phone = format!("09{:08}", Uuid::new_v4().as_u128() % 100_000_000);
    let result = services::add_parent_to_student(
        &pool,
        Uuid::new_v4(),
        CreateParentRequest {
            title: None,
            first_name: "Missing".to_string(),
            last_name: "Parent".to_string(),
            phone: phone.clone(),
            relationship: "parent".to_string(),
            national_id: None,
            email: None,
        },
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
    let created: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
            .bind(phone)
            .fetch_one(&pool)
            .await
            .expect("parent existence should load");
    assert!(!created);
}

async fn parent_fixture(name: &str) -> (sqlx::PgPool, Uuid) {
    let pool = school_test_db::create_named_test_pool(name).await;
    run_test_migrations(&pool).await;
    let student = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'not-a-login','Student','Fixture','student','active')").bind(student).bind(format!("student-{student}")).execute(&pool).await.unwrap();
    (pool, student)
}
fn parent_payload() -> CreateParentRequest {
    CreateParentRequest {
        title: None,
        first_name: "Parent".into(),
        last_name: "Fixture".into(),
        phone: format!("09{:08}", Uuid::new_v4().as_u128() % 100_000_000),
        relationship: "parent".into(),
        national_id: None,
        email: None,
    }
}
#[tokio::test]
async fn parent_creation_requires_an_active_matching_role_and_rolls_back_if_missing() {
    let (pool, student) = parent_fixture("parent_required_role").await;
    sqlx::query("UPDATE roles SET is_active=false WHERE code='PARENT'")
        .execute(&pool)
        .await
        .unwrap();
    let payload = parent_payload();
    assert!(matches!(
        services::add_parent_to_student(&pool, student, payload.clone()).await,
        Err(AppError::Conflict(_))
    ));
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE username=$1)")
            .bind(payload.phone)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM student_parents WHERE student_user_id=$1"
        )
        .bind(student)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
#[tokio::test]
async fn parent_reuse_repairs_only_missing_role_history_and_rejects_nonparent_identity() {
    let (pool, student) = parent_fixture("parent_reuse_roles").await;
    let payload = parent_payload();
    let parent = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'not-a-login','Parent','Fixture','parent','active')").bind(parent).bind(&payload.phone).execute(&pool).await.unwrap();
    assert_eq!(
        services::add_parent_to_student(&pool, student, payload.clone())
            .await
            .unwrap(),
        parent
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=$1 AND r.code='PARENT'").bind(parent).fetch_one(&pool).await.unwrap(), 1);
    services::remove_parent_from_student(&pool, student, parent)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE user_roles SET ended_at=CURRENT_DATE-1,started_at=CURRENT_DATE-2 WHERE user_id=$1",
    )
    .bind(parent)
    .execute(&pool)
    .await
    .unwrap();
    services::add_parent_to_student(&pool, student, payload.clone())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM user_roles WHERE user_id=$1 AND ended_at IS NULL"
        )
        .bind(parent)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let wrong = parent_payload();
    sqlx::query("UPDATE users SET username=$2 WHERE id=$1")
        .bind(student)
        .bind(&wrong.phone)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        services::add_parent_to_student(&pool, student, wrong).await,
        Err(AppError::Conflict(_))
    ));
}
#[tokio::test]
async fn newly_created_parent_is_provisioned_and_linked_atomically() {
    let (pool, student) = parent_fixture("parent_new_roles").await;
    let parent = services::add_parent_to_student(&pool, student, parent_payload())
        .await
        .unwrap();
    assert!(sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id JOIN role_permissions rp ON rp.role_id=r.id JOIN permissions p ON p.id=rp.permission_id WHERE ur.user_id=$1 AND r.code='PARENT' AND p.code='attendance.read.own')").bind(parent).fetch_one(&pool).await.unwrap());
    assert!(sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM student_parents WHERE parent_user_id=$1 AND student_user_id=$2)").bind(parent).bind(student).fetch_one(&pool).await.unwrap());
}

#[tokio::test]
async fn create_student_with_new_parent_persists_working_passwords_and_required_roles() {
    let pool = school_test_db::create_named_test_pool("student_parent_password_creation").await;
    run_test_migrations(&pool).await;
    let actor = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'not-a-login','Registrar','Fixture','staff','active')")
        .bind(actor).bind(format!("registrar-{actor}")).execute(&pool).await.unwrap();
    let password = "Student-fixture-password";
    let username = format!("student-{}", Uuid::new_v4());
    let mut parent = parent_payload();
    parent.relationship = "guardian".into();
    let parent_phone = parent.phone.clone();
    let created = services::create_student(
        &pool,
        actor,
        CreateStudentRequest {
            national_id: None,
            username: Some(username.clone()),
            email: None,
            password: password.into(),
            first_name: "Student".into(),
            last_name: "Fixture".into(),
            title: None,
            student_id: "STUDENT-WITH-PARENT".into(),
            registration_status: None,
            enrollment_date: None,
            phone: None,
            address: None,
            date_of_birth: None,
            gender: None,
            parents: Some(vec![parent]),
        },
    )
    .await
    .expect("student and new parent creation should succeed");
    assert_eq!(created.student.username, username);
    assert_eq!(created.parent_ids.len(), 1);
    let parent_id = created.parent_ids[0];
    assert_ne!(created.student.id, parent_id);

    let registration: (String, String, Uuid) = sqlx::query_as(
        "SELECT student_id,registration_status,registration_status_updated_by FROM student_info WHERE user_id=$1",
    )
    .bind(created.student.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        registration,
        ("STUDENT-WITH-PARENT".into(), "pending".into(), actor)
    );

    for (user_id, expected_username, credential, user_type, role) in [
        (
            created.student.id,
            username.as_str(),
            password,
            "student",
            "STUDENT",
        ),
        (
            parent_id,
            parent_phone.as_str(),
            parent_phone.as_str(),
            "parent",
            "PARENT",
        ),
    ] {
        let persisted: (String, String, String, String) =
            sqlx::query_as("SELECT username,password_hash,user_type,status FROM users WHERE id=$1")
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(persisted.0, expected_username);
        assert_eq!(persisted.2, user_type);
        assert_eq!(persisted.3, "active");
        let hash = persisted.1;
        assert_eq!(
            hash.parse::<bcrypt::HashParts>().unwrap().get_cost(),
            bcrypt::DEFAULT_COST
        );
        assert!(bcrypt::verify(credential, &hash).unwrap());
        assert!(!bcrypt::verify("incorrect-fixture-password", &hash).unwrap());
        assert!(sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=$1 AND r.code=$2 AND r.user_type=$3 AND r.is_active=true AND ur.is_primary=true AND ur.ended_at IS NULL)")
            .bind(user_id).bind(role).bind(user_type).fetch_one(&pool).await.unwrap());
    }
    let relationship: (String, bool) = sqlx::query_as(
        "SELECT relationship,is_primary FROM student_parents WHERE student_user_id=$1 AND parent_user_id=$2",
    )
    .bind(created.student.id)
    .bind(parent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(relationship, ("guardian".into(), true));
}
