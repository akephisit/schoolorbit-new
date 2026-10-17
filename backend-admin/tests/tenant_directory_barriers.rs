use backend_admin::{build_app, db::migrate_admin, AppState};
use reqwest::{Client, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
async fn durable_barrier_survives_school_restart_and_recovery_prevents_late_writer() {
    let fixture = std::env::var("TEST_DATABASE_URL").expect("owned fixture required");
    let control = PgPool::connect(&fixture).await.unwrap();
    let database = format!("so_directory_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
        .execute(&control)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&fixture).unwrap();
    url.set_path(&database);
    let pool = PgPool::connect(url.as_str()).await.unwrap();
    migrate_admin(&pool).await.unwrap();
    let id:Uuid = sqlx::query_scalar("INSERT INTO schools(name,subdomain,db_name,status) VALUES('Barrier fixture','barrier-fixture','barrier_fixture','active') RETURNING id").fetch_one(&pool).await.unwrap();
    let token = Uuid::new_v4();
    sqlx::query("INSERT INTO tenant_directory_mutations(school_id,token) VALUES($1,$2)")
        .bind(id)
        .bind(token)
        .execute(&pool)
        .await
        .unwrap();
    // The Admin barrier is durable: a completely new School client (simulated restart)
    // cannot resolve credentials, and the all-tenant deployment list fails as a whole.
    std::env::set_var("INTERNAL_API_SECRET", "directory-fixture-secret");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = build_app(AppState::new(pool.clone()));
    let server = tokio::spawn(async { axum::serve(listener, app).await.unwrap() });
    let client = Client::new();
    for path in [
        "/internal/schools/barrier-fixture",
        "/internal/schools?status=active",
        "/internal/schools/barrier-fixture/migration-connection",
    ] {
        let response = client
            .get(format!("{origin}{path}"))
            .header("X-Internal-Caller", "backend-school")
            .header("X-Internal-Secret", "directory-fixture-secret")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = response.text().await.unwrap();
        assert!(!body.contains("db_connection_string"));
        assert!(!body.contains("password"));
    }
    // A cancelled/rolled-back writer cannot accidentally remove the barrier.
    let mut transaction = pool.begin().await.unwrap();
    let claimed: Option<Uuid> = sqlx::query_scalar(
        "DELETE FROM tenant_directory_mutations WHERE school_id=$1 AND token=$2 RETURNING token",
    )
    .bind(id)
    .bind(token)
    .fetch_optional(&mut *transaction)
    .await
    .unwrap();
    assert_eq!(claimed, Some(token));
    sqlx::query("UPDATE schools SET status='inactive' WHERE id=$1")
        .bind(id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM schools WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "active"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM tenant_directory_mutations WHERE school_id=$1"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    // Exact-token recovery serializes with the old writer. Its later claim returns no row,
    // so production's claim guard refuses to perform the school update.
    sqlx::query("DELETE FROM tenant_directory_mutations WHERE school_id=$1 AND token=$2")
        .bind(id)
        .bind(token)
        .execute(&pool)
        .await
        .unwrap();
    let late: Option<Uuid> = sqlx::query_scalar(
        "DELETE FROM tenant_directory_mutations WHERE school_id=$1 AND token=$2 RETURNING token",
    )
    .bind(id)
    .bind(token)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(late.is_none());
    let next = Uuid::new_v4();
    sqlx::query("INSERT INTO tenant_directory_mutations(school_id,token) VALUES($1,$2)")
        .bind(id)
        .bind(next)
        .execute(&pool)
        .await
        .unwrap();
    let stale: Option<Uuid> = sqlx::query_scalar(
        "DELETE FROM tenant_directory_mutations WHERE school_id=$1 AND token=$2 RETURNING token",
    )
    .bind(id)
    .bind(token)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(stale.is_none());
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM tenant_directory_mutations WHERE school_id=$1 AND token=$2")
        .bind(id)
        .bind(next)
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE schools SET status='inactive' WHERE id=$1")
        .bind(id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let response = client
        .get(format!("{origin}/internal/schools?status=active"))
        .header("X-Internal-Secret", "directory-fixture-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["total"],
        0
    );
    server.abort();
    pool.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE {database} WITH (FORCE)"
    )))
    .execute(&control)
    .await
    .unwrap();
    control.close().await;
}
