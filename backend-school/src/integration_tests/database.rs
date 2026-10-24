//! Disposable PostgreSQL execution tests for the connection-bound provider.
use school_database::DbSession;
use sqlx::{Connection, PgPool, Postgres, QueryBuilder, Row};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::instrument::WithSubscriber;

async fn fixture(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 3).await;
    sqlx::query("CREATE TABLE query_probe(id integer PRIMARY KEY, label text NOT NULL, changed integer NOT NULL DEFAULT 0)")
        .execute(&pool).await.unwrap();
    pool
}
#[derive(sqlx::FromRow, Debug, PartialEq)]
struct ProbeRow {
    id: i32,
    label: String,
}

#[tokio::test]
async fn database_typed_bulk_native_builder_and_borrowed_values_round_trip() {
    let pool = fixture("database_typed_bulk").await;
    let mut connection = pool.acquire().await.unwrap();
    let mut session = DbSession::new(&mut connection);
    let borrowed = String::from("synthetic");
    let mut builder = QueryBuilder::<Postgres>::new("INSERT INTO query_probe(id,label) ");
    builder.push_values(
        [(1_i32, borrowed.as_str()), (2_i32, "second")],
        |mut row, (id, label)| {
            row.push_bind(id).push_bind(label);
        },
    );
    let affected = session
        .prepared_query("test.bulk", builder.build())
        .execute()
        .await
        .unwrap();
    assert_eq!(affected.rows_affected(), 2);
    let rows: Vec<ProbeRow> = session
        .query_as("test.rows", "SELECT id,label FROM query_probe ORDER BY id")
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![
            ProbeRow {
                id: 1,
                label: borrowed.clone()
            },
            ProbeRow {
                id: 2,
                label: "second".into()
            }
        ]
    );
    let count: i64 = session
        .scalar("test.count", "SELECT count(*) FROM query_probe")
        .fetch_one()
        .await
        .unwrap();
    assert_eq!(count, 2);
    let absent: Option<i32> = session
        .scalar("test.optional", "SELECT id FROM query_probe WHERE id=$1")
        .bind(3_i32)
        .fetch_optional()
        .await
        .unwrap();
    assert_eq!(absent, None);
    let raw = session
        .query("test.raw", "SELECT label FROM query_probe WHERE id=$1")
        .bind(1_i32)
        .fetch_one()
        .await
        .unwrap();
    assert_eq!(raw.try_get::<String, _>("label").unwrap(), borrowed);
}

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<String>>>);
impl tracing::Subscriber for Recorder {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target() == "school_database" || metadata.target().starts_with("school_tenancy")
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        struct Fields(String);
        impl tracing::field::Visit for Fields {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0.push_str(&format!("{}={value:?};", field.name()));
            }
        }
        let mut fields = Fields(String::new());
        event.record(&mut fields);
        self.0.lock().unwrap().push(fields.0);
    }
}

#[tokio::test]
async fn database_real_error_retains_sqlx_error_but_trace_excludes_sql_and_bound_payload() {
    let pool = fixture("database_safe_error").await;
    let mut connection = pool.acquire().await.unwrap();
    let recorder = Recorder::default();
    let captured = recorder.clone();
    let error = async {
        DbSession::new(&mut connection)
            .scalar::<i32>("test.invalid_cast", "SELECT $1::text::integer")
            .bind("synthetic-secret-payload")
            .fetch_one()
            .await
            .unwrap_err()
    }
    .with_subscriber(recorder)
    .await;
    assert!(matches!(error, sqlx::Error::Database(_)));
    let events = captured.0.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert!(
        events[0].contains("status=\"error\"") && events[0].contains("error_class=\"database\"")
    );
    assert!(!events[0].contains("synthetic-secret-payload") && !events[0].contains("SELECT"));
}

#[tokio::test]
async fn database_cancelled_query_observes_cancellation_and_domain_rolls_back_prior_write() {
    let pool = fixture("database_cancelled_rollback").await;
    sqlx::query("INSERT INTO query_probe(id,label) VALUES(1,'locked')")
        .execute(&pool)
        .await
        .unwrap();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM query_probe WHERE id=1 FOR UPDATE")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    let mut transaction = connection.begin().await.unwrap();
    DbSession::new(&mut transaction)
        .query(
            "test.before_cancel",
            "INSERT INTO query_probe(id,label) VALUES(2,'rollback')",
        )
        .execute()
        .await
        .unwrap();
    let recorder = Recorder::default();
    let captured = recorder.clone();
    let cancelled = async {
        tokio::time::timeout(
            Duration::from_millis(30),
            DbSession::new(&mut transaction)
                .query(
                    "test.cancel",
                    "UPDATE query_probe SET changed=changed+1 WHERE id=1",
                )
                .execute(),
        )
        .await
    }
    .with_subscriber(recorder)
    .await;
    assert!(cancelled.is_err());
    blocker.rollback().await.unwrap();
    transaction.rollback().await.unwrap();
    let count: i64 = DbSession::new(&mut connection)
        .scalar(
            "test.rollback_result",
            "SELECT count(*) FROM query_probe WHERE id=2 OR changed<>0",
        )
        .fetch_one()
        .await
        .unwrap();
    assert_eq!(count, 0);
    let events = captured.0.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert!(events[0].contains("status=\"cancelled\""));
    assert!(!events[0].contains("UPDATE") && !events[0].contains("rollback"));
}

#[tokio::test]
async fn database_owned_observed_transaction_releases_single_connection_on_commit_and_rollback() {
    use school_tenancy::TenantPool;
    let pool = school_test_db::create_named_test_pool("database_owned_transaction").await;
    sqlx::query("CREATE TABLE query_probe(id integer PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    let mut transaction = TenantPool::begin_observed(&pool, "test.transaction.commit")
        .await
        .unwrap();
    DbSession::new(&mut transaction)
        .query(
            "test.transaction.insert",
            "INSERT INTO query_probe VALUES(1)",
        )
        .execute()
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let mut transaction = TenantPool::begin_observed(&pool, "test.transaction.rollback")
        .await
        .unwrap();
    DbSession::new(&mut transaction)
        .query(
            "test.transaction.rollback_insert",
            "INSERT INTO query_probe VALUES(2)",
        )
        .execute()
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    let mut connection = tokio::time::timeout(
        Duration::from_secs(1),
        TenantPool::observed_checkout_pool(&pool, "test.after_transaction"),
    )
    .await
    .unwrap()
    .unwrap();
    let rows: Vec<i32> = DbSession::new(&mut connection)
        .scalar(
            "test.after_transaction_rows",
            "SELECT id FROM query_probe ORDER BY id",
        )
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows, vec![1]);
}

#[tokio::test]
async fn database_owned_observed_transaction_retains_managed_lease_and_physical_reservation_until_release(
) {
    let recorder = Recorder::default();
    let captured = recorder.clone();
    async {
        for end in ["commit", "rollback", "drop", "cancel"] {
            observed_transaction_end_case(end).await;
        }
    }
    .with_subscriber(recorder)
    .await;
    let events = captured.0.lock().unwrap().join("\n");
    assert!(
        events.contains("begin_wait_us=")
            && events.contains("hold_us=")
            && events.contains("admission_us=")
    );
    assert!(events.contains("status=\"cancelled\""));
    for forbidden in [
        "postgres://",
        "postgresql://",
        "INSERT INTO",
        "synthetic-secret",
    ] {
        assert!(!events.contains(forbidden));
    }
}

async fn observed_transaction_end_case(end: &str) {
    use school_tenancy::{PoolManager, PoolManagerConfig, TenantPool};
    let name = format!("database_owned_transaction_{end}");
    let fixture = school_test_db::create_named_test_pool_with_max_connections(&name, 3).await;
    school_test_db::run_test_migrations(&fixture).await;
    sqlx::query("CREATE TABLE query_probe(id integer PRIMARY KEY)")
        .execute(&fixture)
        .await
        .unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(&fixture)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema},public"));
    let manager = PoolManager::with_config(PoolManagerConfig::for_tests(2, 1, 1));
    let origin = manager
        .get_pool(url.as_str(), "transaction-origin")
        .await
        .unwrap();
    let raw = origin.as_ref().clone();
    let mut transaction = TenantPool::begin_observed(&origin, "test.managed_transaction")
        .await
        .unwrap();
    drop(origin);
    assert!(
        TenantPool::retain_for_pool(&raw).unwrap().is_some(),
        "owned transaction must retain the originating logical lease"
    );
    DbSession::new(&mut transaction)
        .query("test.managed_insert", "INSERT INTO query_probe VALUES(1)")
        .execute()
        .await
        .unwrap();
    // Waiting for a second transaction on the one-connection pool is cancellable;
    // it must not retain an orphaned logical lease after its admission is dropped.
    assert!(tokio::time::timeout(
        Duration::from_millis(30),
        TenantPool::begin_observed(&raw, "test.cancelled_begin")
    )
    .await
    .is_err());
    let mut second_url = url.clone();
    second_url
        .query_pairs_mut()
        .append_pair("application_name", "transaction-second");
    let second = manager
        .get_pool(second_url.as_str(), "transaction-second")
        .await
        .unwrap();
    let mut third_url = url.clone();
    third_url
        .query_pairs_mut()
        .append_pair("application_name", "transaction-third");
    assert!(manager
        .get_pool(third_url.as_str(), "transaction-third")
        .await
        .is_err());
    assert!(
        !raw.is_closed(),
        "active transaction must remain ineligible for eviction"
    );
    match end {
        "commit" => transaction.commit().await.unwrap(),
        "rollback" => transaction.rollback().await.unwrap(),
        "drop" => drop(transaction),
        "cancel" => {
            let (started, observed) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(async move {
                let _transaction = transaction;
                started.send(()).unwrap();
                std::future::pending::<()>().await;
            });
            observed.await.unwrap();
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        }
        _ => unreachable!(),
    }
    assert!(
        TenantPool::retain_for_pool(&raw).is_err(),
        "every completion must release its last logical operation lease"
    );
    let third = manager
        .get_pool(third_url.as_str(), "transaction-third")
        .await
        .unwrap();
    assert!(raw.is_closed() && !third.is_closed() && !second.is_closed());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM query_probe")
        .fetch_one(&fixture)
        .await
        .unwrap();
    assert_eq!(
        count,
        i64::from(end == "commit"),
        "cancelled/dropped work rolls back without an automatic write replay"
    );
}
