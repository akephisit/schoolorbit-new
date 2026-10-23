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
