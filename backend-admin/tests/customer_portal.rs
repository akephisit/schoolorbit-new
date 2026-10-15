use backend_admin::{
    build_app,
    db::migrate_admin,
    portal::{auth, models::*, service},
    AppState,
};
use reqwest::{Client, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

async fn account(pool: &PgPool, email: &str, role: &str) -> Account {
    let hash = auth::password_hash("fixture-password-123".into())
        .await
        .unwrap();
    sqlx::query_as("INSERT INTO platform_accounts(email,password_hash,name,role,verified_at) VALUES($1,$2,'Fixture',$3,now()) RETURNING id,email,name,role")
        .bind(email).bind(hash).bind(role).fetch_one(pool).await.unwrap()
}

fn readiness(database_url: &str) -> bool {
    std::process::Command::new(env!("CARGO_BIN_EXE_create_admin"))
        .arg("--check-ready")
        .env("ADMIN_MIGRATION_DATABASE_URL", database_url)
        .env("BASE_DOMAIN", "example.test")
        .env("RESEND_API_KEY", "re_synthetic-test-key")
        .env("RESEND_FROM", "noreply@example.test")
        .output()
        .unwrap()
        .status
        .success()
}

#[tokio::test]
async fn portal_preserves_schools_and_enforces_account_service_boundaries() {
    std::env::set_var("BASE_DOMAIN", "example.test");
    std::env::remove_var("RESEND_API_KEY");
    std::env::remove_var("RESEND_FROM");
    let fixture = std::env::var("TEST_DATABASE_URL").expect("owned fixture required");
    let control = PgPool::connect(&fixture).await.unwrap();
    let database = format!("so_portal_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
        .execute(&control)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&fixture).unwrap();
    url.set_path(&database);
    let pool = PgPool::connect(url.as_str()).await.unwrap();
    let mut old = sqlx::migrate!("./migrations");
    old.migrations =
        std::borrow::Cow::Owned(old.iter().filter(|m| m.version <= 9).cloned().collect());
    old.run(&pool).await.unwrap();
    assert!(!readiness(url.as_str()));
    let school = Uuid::now_v7();
    let other_school = Uuid::now_v7();
    for (id, subdomain) in [(school, "fixture"), (other_school, "other")] {
        sqlx::query("INSERT INTO schools(id,name,subdomain,db_name,db_runtime_password,db_migration_password,config) VALUES($1,'Preserved school',$2,$2,$3,$4,'{\"dns_record_id\":\"preserved\",\"deployment_url\":\"https://fixture.example.test\"}')")
            .bind(id).bind(subdomain).bind(vec![1u8,2,3]).bind(vec![4u8,5,6]).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO deployment_history(school_id,status,message) VALUES($1,'success','Preserved deployment')").bind(school).execute(&pool).await.unwrap();
    let before: serde_json::Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM schools s")
            .fetch_one(&pool)
            .await
            .unwrap();
    let history: serde_json::Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(d) ORDER BY id) FROM deployment_history d")
            .fetch_one(&pool)
            .await
            .unwrap();
    // Unexpected account dependencies must prevent cleanup, with the whole migration rolled back.
    sqlx::query(
        "CREATE TABLE unexpected_account_dependency (account_id UUID REFERENCES admin_users(id))",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(migrate_admin(&pool).await.is_err());
    // A failed SQLx migration process exits in production. Close its pooled
    // connections here too, releasing that process's connection-scoped advisory lock.
    pool.close().await;
    let pool = PgPool::connect(url.as_str()).await.unwrap();
    let old_exists: bool = sqlx::query_scalar("SELECT to_regclass('admin_users') IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(old_exists);
    sqlx::query("DROP TABLE unexpected_account_dependency")
        .execute(&pool)
        .await
        .unwrap();
    migrate_admin(&pool).await.unwrap();
    migrate_admin(&pool).await.unwrap();
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM schools s")
            .fetch_one(&pool)
            .await
            .unwrap();
    let after_history: serde_json::Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(d) ORDER BY id) FROM deployment_history d")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    assert_eq!(history, after_history);
    let old_exists: bool = sqlx::query_scalar("SELECT to_regclass('admin_users') IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!old_exists);
    assert!(!readiness(url.as_str()));
    let operator = account(&pool, "operator@example.test", "operator").await;
    assert!(readiness(url.as_str()));
    let customer = account(&pool, "owner@example.test", "customer").await;
    let stranger = account(&pool, "stranger@example.test", "customer").await;
    service::assign(&pool, &operator, school, &customer.email)
        .await
        .unwrap();
    assert!(
        service::assign(&pool, &customer, other_school, &stranger.email)
            .await
            .is_err()
    );
    assert!(service::schools(&pool, &stranger).await.unwrap().is_empty());
    assert_eq!(service::schools(&pool, &customer).await.unwrap().len(), 1);
    sqlx::query("INSERT INTO school_service_periods(school_id,cadence,starts_at,ends_at,granted_by) VALUES($1,'monthly',now()-interval '2 months',now()-interval '1 month',$2)")
        .bind(school).bind(operator.id).execute(&pool).await.unwrap();
    let own = service::schools(&pool, &customer).await.unwrap();
    assert!(own[0].expired);
    assert_eq!(own[0].id, school);
    let request = ServiceRequestInput {
        id: Uuid::now_v7(),
        school_id: Some(school),
        kind: "renew_school".into(),
        school_name: "ignored".into(),
        subdomain: "ignored".into(),
        cadence: "yearly".into(),
    };
    assert!(service::create_request(&pool, &stranger, request.clone())
        .await
        .is_err());
    service::create_request(&pool, &customer, request.clone())
        .await
        .unwrap();
    service::create_request(&pool, &customer, request.clone())
        .await
        .unwrap();
    assert_eq!(service::requests(&pool, &customer).await.unwrap().len(), 1);
    assert!(service::requests(&pool, &stranger)
        .await
        .unwrap()
        .is_empty());
    let decision = || DecisionInput {
        approve: true,
        school_id: Some(school),
        reason: "Manual service approval".into(),
    };
    assert!(service::decide(&pool, &customer, request.id, decision())
        .await
        .is_err());
    let (first, second) = tokio::join!(
        service::decide(&pool, &operator, request.id, decision()),
        service::decide(&pool, &operator, request.id, decision())
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let periods: i64 =
        sqlx::query_scalar("SELECT count(*) FROM school_service_periods WHERE school_id=$1")
            .bind(school)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(periods, 2);
    assert!(!service::schools(&pool, &customer).await.unwrap()[0].expired);
    let (_, raw) = auth::login(
        &pool,
        Credentials {
            email: customer.email.clone(),
            password: "fixture-password-123".into(),
        },
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = build_app(AppState::new(pool.clone()));
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = Client::new();
    let login_response = client
        .post(format!("{base}/api/v1/auth/login"))
        .header("origin", "https://example.test")
        .json(&serde_json::json!({"email":operator.email,"password":"fixture-password-123"}))
        .send()
        .await
        .unwrap();
    assert_eq!(login_response.status(), StatusCode::OK);
    let cookies: Vec<_> = login_response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|header| header.to_str().unwrap())
        .collect();
    let opaque = cookies
        .iter()
        .find(|cookie| cookie.starts_with(auth::COOKIE))
        .unwrap();
    for attribute in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"] {
        assert!(opaque.contains(attribute));
    }
    assert!(!opaque.contains("Domain="));
    assert_eq!(
        login_response.headers().get("cache-control").unwrap(),
        "no-store"
    );
    let operator_cookie = opaque.split(';').next().unwrap().to_string();
    let deleted = client
        .delete(format!("{base}/api/v1/schools/{school}"))
        .header("cookie", &operator_cookie)
        .header(
            "x-csrf-token",
            auth::csrf(operator_cookie.split_once('=').unwrap().1),
        )
        .header("origin", "https://example.test")
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(auth::bootstrap_operator(
        &pool,
        &customer.email,
        "fixture-password-123".into(),
        "Cannot promote"
    )
    .await
    .is_err());

    let cookie = format!("{}={raw}", auth::COOKIE);
    let unauthorized = client
        .get(format!("{base}/api/v1/portal/schools"))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let denied = client
        .get(format!("{base}/api/v1/schools"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let denied = client
        .get(format!("{base}/api/v1/operator/migration/status"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let own = client
        .get(format!("{base}/api/v1/portal/schools"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(own["data"].as_array().unwrap().len(), 1);
    assert!(own["data"][0].get("dbRuntimePassword").is_none());
    let logout = || {
        client
            .post(format!("{base}/api/v1/auth/logout"))
            .header("cookie", &cookie)
    };
    assert_eq!(
        logout().send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        logout()
            .header("origin", "https://fixture.example.test")
            .header("x-csrf-token", auth::csrf(&raw))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        logout()
            .header("origin", "https://example.test")
            .header("x-csrf-token", auth::csrf(&raw))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert!(auth::authenticate(&pool, &raw).await.is_err());
    let legacy = client
        .get(format!("{base}/api/v1/auth/me"))
        .header("cookie", "auth_token=synthetic.legacy.jwt")
        .send()
        .await
        .unwrap();
    assert_eq!(legacy.status(), StatusCode::UNAUTHORIZED);
    // Registration must never grant operator access, even with email delivery unavailable.
    let registration=client.post(format!("{base}/api/v1/auth/register")).header("origin","https://example.test")
        .json(&serde_json::json!({"email":"pending@example.test","password":"fixture-password-123","name":"Pending"})).send().await.unwrap();
    assert_eq!(registration.status(), StatusCode::SERVICE_UNAVAILABLE);
    let pending: (String, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT role,verified_at FROM platform_accounts WHERE email='pending@example.test'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending.0, "customer");
    assert!(pending.1.is_none());
    assert!(auth::login(
        &pool,
        Credentials {
            email: "pending@example.test".into(),
            password: "fixture-password-123".into()
        }
    )
    .await
    .is_err());
    // Single use, purpose and expiry. Reset revokes all existing sessions.
    let reset = auth::random_token().unwrap();
    sqlx::query("INSERT INTO platform_email_tokens(token_hash,account_id,purpose,expires_at) VALUES($1,$2,'reset',now()+interval '1 hour')")
        .bind(auth::digest(&reset)).bind(customer.id).execute(&pool).await.unwrap();
    assert!(auth::consume_token(&pool, &reset, None).await.is_err());
    let (_, session) = auth::login(
        &pool,
        Credentials {
            email: customer.email.clone(),
            password: "fixture-password-123".into(),
        },
    )
    .await
    .unwrap();
    auth::consume_token(&pool, &reset, Some("changed-password-123".into()))
        .await
        .unwrap();
    assert!(auth::authenticate(&pool, &session).await.is_err());
    assert!(
        auth::consume_token(&pool, &reset, Some("changed-password-123".into()))
            .await
            .is_err()
    );
    let expired = auth::random_token().unwrap();
    sqlx::query("INSERT INTO platform_email_tokens(token_hash,account_id,purpose,expires_at) SELECT $1,id,'verify',now()-interval '1 hour' FROM platform_accounts WHERE email='pending@example.test'")
        .bind(auth::digest(&expired)).execute(&pool).await.unwrap();
    assert!(auth::consume_token(&pool, &expired, None).await.is_err());
    sqlx::query(
        "UPDATE platform_email_tokens SET expires_at=now()+interval '1 hour' WHERE token_hash=$1",
    )
    .bind(auth::digest(&expired))
    .execute(&pool)
    .await
    .unwrap();
    auth::consume_token(&pool, &expired, None).await.unwrap();
    assert!(auth::consume_token(&pool, &expired, None).await.is_err());
    for _ in 0..2 {
        auth::throttle(&pool, "fixture-budget", 2).await.unwrap();
    }
    assert!(auth::throttle(&pool, "fixture-budget", 2).await.is_err());
    let preserved: serde_json::Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM schools s")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, preserved);
    server.abort();
    let _ = server.await;
    pool.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP DATABASE {database}")))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
