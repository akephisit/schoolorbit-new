# Backend School Lazy Test Pools Design

## Goal

Keep the complete `backend-school` test suite repeatable when many shared- and named-schema tests start concurrently against a connection-limited PostgreSQL service.

## Root cause

Each test reset or ensured its schema with a short-lived administrative connection, then eagerly opened its application pool before entering the process-wide migration lock. Concurrent tests therefore held idle database connections during a potentially long migration wait. The accumulated eager connections exhausted the remote database limit and surfaced an unrelated `PoolTimedOut` in whichever test tried to reconnect or acquire next.

## Design

- Keep named-schema reset eager so failures to drop or create the isolated schema remain immediate and explicit.
- Construct every shared- and named-schema test application pool with SQLx `connect_lazy` after schema setup.
- Preserve its configured maximum connection count and `after_connect` search-path initialization.
- Let the first migration query establish the first application connection while `run_test_migrations` holds the existing migration lock.
- Keep each pool local to its test's Tokio runtime; never reuse an SQLx pool across independently created `#[tokio::test]` runtimes.
- Leave runtime pools, database URL policy, timeouts, schema isolation, and migration implementation unchanged.

## Regression proof

A unit test constructs the common application-pool helper with a syntactically valid URL pointing to an unreachable local port. Construction must succeed without network access. Existing database-backed shared and named tests then prove that the first real acquisition still applies the isolated search path and runs migrations normally.

## Non-goals

- Increasing connection or acquisition timeouts.
- Adding retries, sleeps, ignored failures, or serializing the entire suite.
- Changing any applied migration, production pool, schema naming rule, or admin/frontend application.
