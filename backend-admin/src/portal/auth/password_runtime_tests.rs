use super::*;
use sqlx::postgres::PgPoolOptions;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

const HASH: &str = "$2b$12$rri4KlbA4XkEM.JADCiBdu3/cjxESITAXMOYiKoklPN9HW6vgjEP2";

struct Fixture {
    control: PgPool,
    pool: PgPool,
    database: String,
    id: Uuid,
}

impl Fixture {
    async fn new() -> Self {
        let fixture = std::env::var("TEST_DATABASE_URL").expect("owned fixture required");
        let control = PgPool::connect(&fixture).await.unwrap();
        let database = format!("so_password_{}", Uuid::new_v4().simple());
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
            .execute(&control)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&fixture).unwrap();
        url.set_path(&database);
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(url.as_str())
            .await
            .unwrap();
        crate::db::migrate_admin(&pool).await.unwrap();
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO platform_accounts(id,email,password_hash,name,verified_at) VALUES($1,'password-probe@example.test',$2,'Synthetic password probe',now())")
            .bind(id).bind(HASH).execute(&pool).await.unwrap();
        Self {
            control,
            pool,
            database,
            id,
        }
    }

    async fn independent_pool(&self) -> PgPool {
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        url.set_path(&self.database);
        PgPool::connect(url.as_str()).await.unwrap()
    }

    async fn close(self) {
        self.pool.close().await;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE {}",
            self.database
        )))
        .execute(&self.control)
        .await
        .unwrap();
        self.control.close().await;
    }
}

fn credentials() -> Credentials {
    Credentials {
        email: "password-probe@example.test".into(),
        password: "synthetic-password-for-wave-four".into(),
    }
}

#[tokio::test]
async fn real_bcrypt_login_resource_probe() {
    let fixture = Fixture::new().await;
    for repetition in 0..4 {
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(4));
        let mut requests = Vec::new();
        for _ in 0..3 {
            let barrier = barrier.clone();
            let pool = fixture.pool.clone();
            requests.push(tokio::spawn(async move {
                barrier.wait().await;
                let start = Instant::now();
                let result = login(&pool, credentials()).await;
                (start.elapsed(), result)
            }));
        }
        barrier.wait().await;
        let batch = Instant::now();
        let mut connection_wait = Duration::ZERO;
        let mut connection_max = Duration::ZERO;
        let mut samples = 0;
        while requests.iter().any(|request| !request.is_finished()) {
            let start = Instant::now();
            drop(fixture.pool.acquire().await.unwrap());
            let elapsed = start.elapsed();
            connection_wait += elapsed;
            connection_max = connection_max.max(elapsed);
            samples += 1;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let mut request_time = Duration::ZERO;
        let mut successes = 0;
        let mut overloads = 0;
        for request in requests {
            let (elapsed, result) = request.await.unwrap();
            request_time += elapsed;
            match result {
                Ok(_) => successes += 1,
                Err(PortalError(StatusCode::SERVICE_UNAVAILABLE, _)) => overloads += 1,
                Err(error) => panic!("unexpected synthetic login failure: {error:?}"),
            }
        }
        eprintln!("real_bcrypt_probe repetition={repetition} warmup={} concurrent=3 successes={successes} overloads={overloads} batch_us={} request_total_us={} connection_samples={samples} connection_total_us={} connection_max_us={}", repetition == 0, batch.elapsed().as_micros(), request_time.as_micros(), connection_wait.as_micros(), connection_max.as_micros());
        assert_eq!(successes, 3);
        assert_eq!(overloads, 0);
    }
    fixture.close().await;
}

#[tokio::test]
async fn login_releases_pool_and_account_lock_while_verification_is_blocked() {
    let fixture = Fixture::new().await;
    let independent = fixture.independent_pool().await;
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let pool = fixture.pool.clone();
    let login = tokio::spawn(async move {
        login_with_verifier(&pool, credentials(), |_, _| async move {
            entered_tx.send(()).unwrap();
            release_rx.await.unwrap();
            Ok(true)
        })
        .await
    });
    entered_rx.await.unwrap();
    let start = Instant::now();
    let available = tokio::time::timeout(Duration::from_millis(150), fixture.pool.acquire()).await;
    let pool_wait = start.elapsed();
    let pool_available = available.is_ok();
    drop(available);
    let mut tx = independent.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout = '150ms'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let start = Instant::now();
    let update = sqlx::query("UPDATE platform_accounts SET name=name WHERE id=$1")
        .bind(fixture.id)
        .execute(&mut *tx)
        .await;
    let lock_wait = start.elapsed();
    let account_available = update.is_ok();
    tx.rollback().await.unwrap();
    release_tx.send(()).unwrap();
    login.await.unwrap().unwrap();
    independent.close().await;
    fixture.close().await;
    eprintln!("password_probe pool_available={pool_available} pool_wait_us={} account_available={account_available} account_wait_us={}", pool_wait.as_micros(), lock_wait.as_micros());
    assert!(
        pool_available,
        "login held the single pool slot during verification"
    );
    assert!(
        account_available,
        "login held the account row lock during verification"
    );
}

#[tokio::test]
async fn changed_login_snapshots_cannot_issue_sessions() {
    for change in ["reset", "unverify", "delete", "email", "replacement"] {
        let fixture = Fixture::new().await;
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let pool = fixture.pool.clone();
        let pending = tokio::spawn(async move {
            login_with_verifier(&pool, credentials(), |_, stored| async move {
                assert_eq!(stored, HASH);
                entered_tx.send(()).unwrap();
                release_rx.await.unwrap();
                Ok(true)
            })
            .await
        });
        entered_rx.await.unwrap();
        // Every mutation uses the same one-slot pool while verification is paused.
        // The reset case exercises the real token consumer and its revocation path.
        match change {
            "reset" => {
                let token = "ab".repeat(32);
                sqlx::query("INSERT INTO platform_email_tokens(token_hash,account_id,purpose,expires_at) VALUES($1,$2,'reset',now()+interval '1 hour')")
                    .bind(digest(&token)).bind(fixture.id).execute(&fixture.pool).await.unwrap();
                consume_token(
                    &fixture.pool,
                    &token,
                    Some("replacement-fixture-password".into()),
                )
                .await
                .unwrap();
            }
            "unverify" => {
                sqlx::query("UPDATE platform_accounts SET verified_at=NULL WHERE id=$1")
                    .bind(fixture.id)
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
            }
            "email" => {
                sqlx::query(
                    "UPDATE platform_accounts SET email='changed@example.test' WHERE id=$1",
                )
                .bind(fixture.id)
                .execute(&fixture.pool)
                .await
                .unwrap();
            }
            "delete" | "replacement" => {
                sqlx::query("DELETE FROM platform_accounts WHERE id=$1")
                    .bind(fixture.id)
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
                if change == "replacement" {
                    sqlx::query("INSERT INTO platform_accounts(email,password_hash,name,verified_at) VALUES('password-probe@example.test',$1,'Replacement fixture',now())")
                        .bind(HASH).execute(&fixture.pool).await.unwrap();
                }
            }
            _ => unreachable!(),
        }
        release_tx.send(()).unwrap();
        assert_eq!(
            pending.await.unwrap().unwrap_err().0,
            StatusCode::UNAUTHORIZED,
            "{change}"
        );
        let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM platform_sessions")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
        assert_eq!(sessions, 0, "{change}");
        fixture.close().await;
    }
}

#[tokio::test]
async fn missing_unverified_and_incorrect_passwords_preserve_dummy_work_and_denial() {
    let fixture = Fixture::new().await;
    for candidate in ["missing", "unverified", "wrong_password"] {
        let mut input = credentials();
        if candidate == "missing" {
            input.email = "missing@example.test".into();
        }
        sqlx::query("UPDATE platform_accounts SET verified_at=CASE WHEN $1 THEN NULL ELSE now() END WHERE id=$2")
            .bind(candidate == "unverified").bind(fixture.id).execute(&fixture.pool).await.unwrap();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let invocation = called.clone();
        let result = login_with_verifier(&fixture.pool, input, |password, stored| async move {
            invocation.store(true, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(password, "synthetic-password-for-wave-four");
            assert_eq!(stored, HASH);
            // Even a successful dummy comparison must not authenticate a missing
            // or unverified account; an actual mismatch also remains denied.
            Ok(candidate != "wrong_password")
        })
        .await;
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
        let error = result.unwrap_err();
        assert_eq!(error.0, StatusCode::UNAUTHORIZED);
        assert_eq!(error.1, "อีเมลหรือรหัสผ่านไม่ถูกต้อง หรือยังไม่ได้ยืนยันอีเมล");
    }
    // The real bcrypt verifier also refuses a wrong password.
    let mut invalid = credentials();
    invalid.password = "incorrect-fixture-password".into();
    assert_eq!(
        login(&fixture.pool, invalid).await.unwrap_err().0,
        StatusCode::UNAUTHORIZED
    );
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM platform_sessions")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
    fixture.close().await;
}

#[tokio::test]
async fn concurrent_logins_preserve_session_cap_and_current_account_fields() {
    let fixture = Fixture::new().await;
    let concurrent_pool = fixture.independent_pool().await;
    sqlx::query(
        "UPDATE platform_accounts SET role='operator',name='Current operator fixture' WHERE id=$1",
    )
    .bind(fixture.id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    for index in 0..9i32 {
        sqlx::query("INSERT INTO platform_sessions(token_hash,account_id,created_at,expires_at) VALUES($1,$2,now()-interval '1 hour'-make_interval(secs=>$3),now()+interval '1 day')")
            .bind(digest(&format!("old-synthetic-session-{index}"))).bind(fixture.id).bind(index as f64)
            .execute(&fixture.pool).await.unwrap();
    }
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    let mut requests = Vec::new();
    for _ in 0..8 {
        let pool = concurrent_pool.clone();
        let barrier = barrier.clone();
        requests.push(tokio::spawn(async move {
            login_with_verifier(&pool, credentials(), |_, _| async move {
                barrier.wait().await;
                Ok(true)
            })
            .await
        }));
    }
    for request in requests {
        let (account, token) = request.await.unwrap().unwrap();
        assert_eq!(account.id, fixture.id);
        assert_eq!(account.role, "operator");
        assert_eq!(account.name, "Current operator fixture");
        assert!(valid_token(&token));
        assert_eq!(
            authenticate(&fixture.pool, &token).await.unwrap().id,
            fixture.id
        );
    }
    let sessions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM platform_sessions WHERE account_id=$1")
            .bind(fixture.id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(sessions, 10);
    let old_sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM platform_sessions WHERE account_id=$1 AND created_at<now()-interval '30 minutes'")
        .bind(fixture.id).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(old_sessions, 2);
    concurrent_pool.close().await;
    fixture.close().await;
}
