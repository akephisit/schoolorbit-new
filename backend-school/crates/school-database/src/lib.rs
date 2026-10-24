//! Connection-bound PostgreSQL execution. Domains own SQL and transaction policy.
use sqlx::{
    postgres::{PgArguments, PgQueryResult, PgRow},
    query::{Query, QueryAs, QueryScalar},
    Encode, Error, FromRow, PgConnection, Postgres, SqlSafeStr, Type,
};
use std::{future::Future, time::Instant};

/// A borrowed connection, without acquisition, transaction or retry behavior.
///
/// Queries exclusively borrow this session until completion or cancellation.
/// ```compile_fail
/// use school_database::DbSession;
/// async fn overlap(connection: &mut sqlx::PgConnection) {
///     let mut session = DbSession::new(connection);
///     let first = session.scalar::<i32>("first", "SELECT 1");
///     let second = session.scalar::<i32>("second", "SELECT 2");
///     first.fetch_one().await.unwrap();
///     second.fetch_one().await.unwrap();
/// }
/// # let _ = overlap;
/// ```
/// Execution cannot substitute a pool or another connection.
/// ```compile_fail
/// use school_database::DbSession;
/// async fn replace(connection: &mut sqlx::PgConnection, pool: &sqlx::PgPool) {
///     DbSession::new(connection).query("query", "SELECT 1").execute(pool).await.unwrap();
/// }
/// # let _ = replace;
/// ```
/// SQLx safe-string requirements are retained.
/// ```compile_fail
/// use school_database::DbSession;
/// fn unchecked(connection: &mut sqlx::PgConnection, sql: String) {
///     DbSession::new(connection).query("query", sql);
/// }
/// # let _ = unchecked;
/// ```
pub struct DbSession<'c> {
    connection: &'c mut PgConnection,
}
impl<'c> DbSession<'c> {
    pub fn new(connection: &'c mut PgConnection) -> Self {
        Self { connection }
    }
    pub fn query<'s, 'q>(
        &'s mut self,
        operation: &'static str,
        sql: impl SqlSafeStr,
    ) -> DbQuery<'s, 'q> {
        self.prepared_query(operation, sqlx::query(sql))
    }
    pub fn query_as<'s, 'q, R>(
        &'s mut self,
        operation: &'static str,
        sql: impl SqlSafeStr,
    ) -> DbQueryAs<'s, 'q, R>
    where
        R: for<'r> FromRow<'r, PgRow>,
    {
        self.prepared_query_as(operation, sqlx::query_as(sql))
    }
    pub fn scalar<'s, 'q, T>(
        &'s mut self,
        operation: &'static str,
        sql: impl SqlSafeStr,
    ) -> DbScalar<'s, 'q, T>
    where
        (T,): for<'r> FromRow<'r, PgRow>,
    {
        self.prepared_scalar(operation, sqlx::query_scalar(sql))
    }
    /// Bind an already-built native query, retaining its SQL safety and arguments.
    pub fn prepared_query<'s, 'q>(
        &'s mut self,
        operation: &'static str,
        query: Query<'q, Postgres, PgArguments>,
    ) -> DbQuery<'s, 'q> {
        DbQuery {
            connection: &mut *self.connection,
            query,
            operation,
        }
    }
    pub fn prepared_query_as<'s, 'q, R>(
        &'s mut self,
        operation: &'static str,
        query: QueryAs<'q, Postgres, R, PgArguments>,
    ) -> DbQueryAs<'s, 'q, R> {
        DbQueryAs {
            connection: &mut *self.connection,
            query,
            operation,
        }
    }
    pub fn prepared_scalar<'s, 'q, T>(
        &'s mut self,
        operation: &'static str,
        query: QueryScalar<'q, Postgres, T, PgArguments>,
    ) -> DbScalar<'s, 'q, T> {
        DbScalar {
            connection: &mut *self.connection,
            query,
            operation,
        }
    }
}

#[must_use = "a database query must be executed"]
pub struct DbQuery<'s, 'q> {
    connection: &'s mut PgConnection,
    query: Query<'q, Postgres, PgArguments>,
    operation: &'static str,
}
impl<'s, 'q> DbQuery<'s, 'q> {
    pub fn bind<'t, T: Encode<'t, Postgres> + Type<Postgres>>(mut self, value: T) -> Self {
        self.query = self.query.bind(value);
        self
    }
    pub async fn execute(self) -> Result<PgQueryResult, Error> {
        observed(self.operation, self.query.execute(self.connection)).await
    }
    pub async fn fetch_one(self) -> Result<PgRow, Error> {
        observed(self.operation, self.query.fetch_one(self.connection)).await
    }
    pub async fn fetch_optional(self) -> Result<Option<PgRow>, Error> {
        observed(self.operation, self.query.fetch_optional(self.connection)).await
    }
    pub async fn fetch_all(self) -> Result<Vec<PgRow>, Error> {
        observed(self.operation, self.query.fetch_all(self.connection)).await
    }
}

#[must_use = "a database query must be executed"]
pub struct DbQueryAs<'s, 'q, R> {
    connection: &'s mut PgConnection,
    query: QueryAs<'q, Postgres, R, PgArguments>,
    operation: &'static str,
}
impl<'s, 'q, R> DbQueryAs<'s, 'q, R> {
    pub fn bind<T: 'q + Encode<'q, Postgres> + Type<Postgres>>(mut self, value: T) -> Self {
        self.query = self.query.bind(value);
        self
    }
}
impl<'s, 'q, R> DbQueryAs<'s, 'q, R>
where
    R: Send + Unpin + for<'r> FromRow<'r, PgRow>,
{
    pub async fn fetch_one(self) -> Result<R, Error> {
        observed(self.operation, self.query.fetch_one(self.connection)).await
    }
    pub async fn fetch_optional(self) -> Result<Option<R>, Error> {
        observed(self.operation, self.query.fetch_optional(self.connection)).await
    }
    pub async fn fetch_all(self) -> Result<Vec<R>, Error> {
        observed(self.operation, self.query.fetch_all(self.connection)).await
    }
}

#[must_use = "a database query must be executed"]
pub struct DbScalar<'s, 'q, T> {
    connection: &'s mut PgConnection,
    query: QueryScalar<'q, Postgres, T, PgArguments>,
    operation: &'static str,
}
impl<'s, 'q, T> DbScalar<'s, 'q, T> {
    pub fn bind<V: 'q + Encode<'q, Postgres> + Type<Postgres>>(mut self, value: V) -> Self {
        self.query = self.query.bind(value);
        self
    }
}
impl<'s, 'q, T> DbScalar<'s, 'q, T>
where
    T: Send + Unpin,
    (T,): for<'r> FromRow<'r, PgRow>,
{
    pub async fn fetch_one(self) -> Result<T, Error> {
        observed(self.operation, self.query.fetch_one(self.connection)).await
    }
    pub async fn fetch_optional(self) -> Result<Option<T>, Error> {
        observed(self.operation, self.query.fetch_optional(self.connection)).await
    }
    pub async fn fetch_all(self) -> Result<Vec<T>, Error> {
        observed(self.operation, self.query.fetch_all(self.connection)).await
    }
}

struct QueryObservation {
    operation: &'static str,
    started: Instant,
    status: &'static str,
    error_class: &'static str,
}
impl QueryObservation {
    fn new(operation: &'static str) -> Self {
        Self {
            operation,
            started: Instant::now(),
            status: "cancelled",
            error_class: "none",
        }
    }
    fn complete<T>(&mut self, result: &Result<T, Error>) {
        match result {
            Ok(_) => {
                self.status = "ok";
            }
            Err(error) => {
                self.status = "error";
                self.error_class = error_class(error);
            }
        }
    }
}
impl Drop for QueryObservation {
    fn drop(&mut self) {
        tracing::debug!(
            operation = self.operation,
            duration_us = self.started.elapsed().as_micros() as u64,
            status = self.status,
            error_class = self.error_class,
            "Database query completed"
        );
    }
}
async fn observed<T>(
    operation: &'static str,
    future: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let mut observation = QueryObservation::new(operation);
    let result = future.await;
    observation.complete(&result);
    result
}
/// A fixed safe category for domain context logs; never formats SQLx payloads.
pub fn error_class(error: &Error) -> &'static str {
    match error {
        Error::Database(_) => "database",
        Error::RowNotFound => "row_not_found",
        Error::PoolTimedOut => "pool_timeout",
        Error::PoolClosed => "pool_closed",
        Error::Io(_) | Error::Tls(_) => "connection",
        Error::Decode(_)
        | Error::ColumnDecode { .. }
        | Error::ColumnNotFound(_)
        | Error::ColumnIndexOutOfBounds { .. } => "decode",
        Error::Encode(_) => "encode",
        Error::Protocol(_) => "protocol",
        Error::Configuration(_) => "configuration",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<String>>>);
    impl tracing::Subscriber for Recorder {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
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
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    self.0.push_str(&format!("{}={value:?};", field.name()));
                }
            }
            let mut fields = Fields(String::new());
            event.record(&mut fields);
            self.0.lock().unwrap().push(fields.0);
        }
    }
    #[test]
    fn actual_execution_guard_observes_completion_error_and_cancellation_without_payloads() {
        let recorder = Recorder::default();
        let captured = recorder.clone();
        tracing::subscriber::with_default(recorder, || {
            let waker = std::task::Waker::noop();
            let mut context = std::task::Context::from_waker(waker);
            let mut success = std::pin::pin!(observed(
                "test.query",
                std::future::ready(Ok::<_, Error>(17))
            ));
            assert!(matches!(
                success.as_mut().poll(&mut context),
                std::task::Poll::Ready(Ok(17))
            ));
            let mut error = std::pin::pin!(observed::<()>(
                "test.error",
                std::future::ready(Err(Error::Protocol("synthetic-secret-SQL-and-bind".into())))
            ));
            assert!(matches!(
                error.as_mut().poll(&mut context),
                std::task::Poll::Ready(Err(Error::Protocol(_)))
            ));
            {
                let mut cancelled =
                    std::pin::pin!(observed::<()>("test.cancel", std::future::pending()));
                assert!(cancelled.as_mut().poll(&mut context).is_pending());
            }
        });
        let events = captured.0.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert!(events[0].contains("status=\"ok\""));
        assert!(
            events[1].contains("status=\"error\"")
                && events[1].contains("error_class=\"protocol\"")
        );
        assert!(events[2].contains("status=\"cancelled\""));
        for event in events.iter() {
            assert!(event.contains("duration_us="));
            assert!(
                !event.contains("synthetic-secret")
                    && !event.contains("SELECT")
                    && !event.contains("bind=")
            );
        }
    }

    #[test]
    fn error_categories_do_not_expose_error_messages() {
        assert_eq!(
            error_class(&Error::Protocol("synthetic-secret".into())),
            "protocol"
        );
        assert_eq!(error_class(&Error::RowNotFound), "row_not_found");
    }
}
