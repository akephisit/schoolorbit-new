use school_errors::AppError;
use school_test_db::{create_test_pool, run_test_migrations};
use uuid::Uuid;

use school_students::models::{CreateParentRequest, UpdateStudentRequest};
use school_students::services;

#[tokio::test]
async fn update_missing_student_returns_not_found() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let result = services::update_student(
        &pool,
        Uuid::new_v4(),
        UpdateStudentRequest {
            email: None,
            first_name: Some("Missing".to_string()),
            last_name: None,
            phone: None,
            address: None,
            student_number: None,
        },
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn delete_missing_student_returns_not_found() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let result = services::delete_student(&pool, Uuid::new_v4(), Uuid::new_v4()).await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn add_parent_to_missing_student_returns_not_found_without_creating_parent() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let phone = format!("09{}", &Uuid::new_v4().simple().to_string()[..8]);
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
        phone: format!("09{}", &Uuid::new_v4().simple().to_string()[..8]),
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
