use backend_admin::{build_app, db::migrate_admin, AppState};
use reqwest::{Client, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
async fn public_directory_is_bounded_credential_free_and_preserves_schools() {
    let fixture = std::env::var("TEST_DATABASE_URL").expect("owned fixture required");
    let control = PgPool::connect(&fixture).await.unwrap();
    let database = format!("so_discovery_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
        .execute(&control)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&fixture).unwrap();
    url.set_path(&database);
    let pool = PgPool::connect(url.as_str()).await.unwrap();
    migrate_admin(&pool).await.unwrap();
    for index in 0..105 {
        sqlx::query("INSERT INTO schools(name,subdomain,db_name,status,db_runtime_password,db_migration_password,config) VALUES($1,$2,$2,'active',$3,$4,'{\"dns_record_id\":\"private-provider-id\"}')")
            .bind(format!("Public fixture {index}"))
            .bind(format!("school-{index:03}"))
            .bind(vec![1u8,2,3]).bind(vec![4u8,5,6]).execute(&pool).await.unwrap();
    }
    for (subdomain, status) in [
        ("sandbox", "active"),
        ("www", "active"),
        ("inactive", "inactive"),
        ("pending", "provisioning"),
        ("bad.host", "active"),
    ] {
        sqlx::query("INSERT INTO schools(name,subdomain,db_name,status) VALUES($1,$1,$1,$2)")
            .bind(subdomain)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
    }
    let snapshot: serde_json::Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM schools s")
            .fetch_one(&pool)
            .await
            .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = build_app(AppState::new(pool.clone()));
    let server = tokio::spawn(async { axum::serve(listener, app).await.unwrap() });
    let client = Client::new();
    let response = client
        .get(format!("{origin}/api/v1/public/schools"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let first: serde_json::Value = response.json().await.unwrap();
    let schools = first["data"]["schools"].as_array().unwrap();
    assert_eq!(schools.len(), 100);
    assert_eq!(first["data"]["nextCursor"], "school-099");
    for (index, school) in schools.iter().enumerate() {
        assert_eq!(school.as_object().unwrap().len(), 2);
        assert_eq!(school["name"], format!("Public fixture {index}"));
        assert_eq!(school["subdomain"], format!("school-{index:03}"));
    }
    assert!(!first.to_string().contains("private-provider-id"));
    assert!(!first.to_string().contains("db_"));
    let second: serde_json::Value = client
        .get(format!("{origin}/api/v1/public/schools?after=school-099"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["data"]["schools"].as_array().unwrap().len(), 5);
    assert!(second["data"]["nextCursor"].is_null());
    assert_eq!(second["data"]["schools"][0]["subdomain"], "school-100");
    for cursor in ["../bad", "sandbox", "UPPER", "x';DROP TABLE schools;--"] {
        let response = client
            .get(format!("{origin}/api/v1/public/schools"))
            .query(&[("after", cursor)])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    for path in ["/api/v1/portal/schools", "/api/v1/schools"] {
        assert_eq!(
            client
                .get(format!("{origin}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        client
            .post(format!("{origin}/api/v1/public/schools"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM schools s")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(snapshot, after);
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
