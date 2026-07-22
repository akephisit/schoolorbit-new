# Backend School Reliability Design

**Date:** 2026-07-23
**Status:** Approved for implementation

## Goal

Improve backend-school reliability in three related areas without changing tenant data or API business behavior:

1. prevent duplicate tenant connection-pool creation and make pool TTL truly sliding;
2. bound backend-school calls to backend-admin with timeout and safe retries;
3. expose strict readiness for deployment and container health checks.

This change is backend and deployment infrastructure only. It does not add a database migration, modify frontend behavior, reintroduce automatic scheduling, or add multi-replica realtime infrastructure.

## Current behavior and problems

### Tenant pool cache

`PoolManager::get_pool()` reads the cache and creates a new `PgPool` after a miss. Concurrent misses for the same tenant can each create a pool because there is no creation lock around the miss-create-insert sequence. A cache hit also clones the pool without updating `PoolEntry::last_used`, so the documented 30-minute TTL is measured from creation rather than last use.

### Internal admin client

`AdminClient` uses `reqwest::Client::new()` and direct `.send()` calls. Requests have no application-defined total timeout and transient failures are returned immediately. A stalled control-plane request can therefore delay tenant resolution, cleanup jobs, calendar jobs, and migration operations for an unbounded library/default duration.

### Backend-school readiness

Backend-school exposes only `/health`, which always returns healthy. It cannot tell deployment tooling whether the backend-admin control plane and its admin database are usable. Backend-admin already exposes `/ready`, but production and development health checks still use liveness routes.

## Chosen approach

Use a targeted reliability foundation implemented with existing Tokio, reqwest, Axum, and sqlx dependencies. Do not add a general resilience framework or circuit breaker in this iteration.

The rejected alternatives are:

- one global pool-creation mutex, because a cold tenant would block unrelated tenants;
- a full Tower resilience/circuit-breaker stack, because it adds policy and operational complexity that is not yet justified for a single backend-school replica.

## Tenant pool design

### Per-tenant single flight

`PoolManager` will keep a per-subdomain asynchronous creation lock. The lock map may retain one lightweight lock per tenant for the process lifetime; its size is bounded by tenants seen by the process.

`get_pool()` will use this flow:

1. inspect the pool cache;
2. if a fresh entry exists, update `last_used` and return its cloned handle;
3. after a miss, acquire only that tenant's creation lock;
4. inspect the cache again because another waiter may have populated it;
5. create and insert one pool only when the second inspection still misses;
6. run the existing migration and permission synchronization gates.

Different tenants can create pools concurrently. Concurrent requests for one tenant share the winner's pool. Migration and permission synchronization retain their current `MigrationTracker` single-flight behavior.

### Sliding TTL

Every successful cache hit sets `last_used` to the current instant. An expired entry is removed before a replacement is created. Periodic cleanup removes entries whose time since last use is at least the configured TTL.

The existing defaults remain unchanged:

- maximum five database connections per school;
- 30-minute pool-cache TTL;
- zero minimum connections and five-minute sqlx idle timeout.

No database URL, credential, or plaintext sensitive value may be added to logs.

### Test boundary

Pool selection and creation will be factored into a private async helper that accepts a pool factory. Production supplies the current `PgPoolOptions` factory; tests supply lazy pools and an atomic creation counter. This enables deterministic tests for concurrent single-flight behavior without requiring a live database for every focused unit test.

Pure entry freshness/touch behavior will accept an explicit `Instant` in tests so sliding TTL and expiration do not depend on sleeps.

## AdminClient design

### Configuration

The client will use these optional environment variables:

- `BACKEND_ADMIN_REQUEST_TIMEOUT_MS`, default `5000`;
- `BACKEND_ADMIN_RETRY_MAX_ATTEMPTS`, default `3`, counting the first request;
- `BACKEND_ADMIN_RETRY_BASE_DELAY_MS`, default `100`.

Production parsing accepts a request timeout from 100 through 30,000 milliseconds, one through five total attempts, and a base delay from 1 through 5,000 milliseconds. Zero, malformed, and out-of-range values produce a startup configuration error. The example environment file documents the same bounds. Tests may construct a client directly with shorter durations.

### Safe retry policy

All requests receive the configured total per-attempt timeout.

GET requests may retry when:

- the request fails because of connection or timeout transport errors;
- the response is `429`;
- the response is any `5xx` status.

GET requests do not retry `400`, `401`, `403`, `404`, or other non-transient `4xx` statuses. Retry delays use deterministic exponential backoff from the configured base delay and stop after the configured maximum attempts.

`PUT /internal/schools/{subdomain}/migration-status` receives the timeout but is not retried because repeating a state-changing request automatically could duplicate or reorder side effects. JSON decoding errors are not retried because a successful HTTP response with an invalid contract is not a transport failure.

The client will expose a readiness method that performs a retryable GET of backend-admin `/ready` and treats only a successful status as ready.

### Error and logging policy

Callers receive concise contextual errors. Retry attempts are logged with structured fields such as operation, attempt, maximum attempts, and status/error category. Logs and HTTP responses must not contain the internal secret, database connection strings, raw response bodies, or request headers.

## Readiness design

### Endpoint semantics

Backend-school keeps `/health` as lightweight process liveness that does not call dependencies.

The new public `GET /ready` endpoint checks backend-admin `/ready` through `AdminClient`:

- backend-admin ready: return HTTP `200` with `status: "ready"` and `controlPlane: "connected"`;
- backend-admin unavailable, timed out, or not ready: log the internal failure and return HTTP `503` with `status: "not_ready"` and `controlPlane: "unavailable"`.

The failure response does not expose the internal error. Both responses include a timestamp. Readiness does not resolve a school, open a tenant pool, run migrations, or ping every tenant database.

Health and readiness remain intentionally outside the generated school OpenAPI contract, matching the existing contract policy for operational endpoints.

### Module placement

Health handlers and response mapping move from `main.rs` into `modules/system/handlers/health.rs`. `main.rs` only registers `/health` and `/ready`. This keeps operational behavior testable without expanding the already large router file.

## Deployment integration

Development and production Compose definitions will use readiness where dependency readiness matters:

- backend-admin container health check calls `/ready`;
- backend-school container health check calls `/ready`;
- backend-school continues to depend on a healthy backend-admin.

Compose readiness probes keep the existing 30-second interval, three retries, and 40-second startup allowance, while increasing the single-probe timeout to 20 seconds so the bounded internal retry budget can finish. The backend deployment workflows will wait up to 60 seconds after startup, polling readiness every five seconds, and fail the deployment job when the new container never becomes ready. This iteration does not redesign deployment around immutable release tags or automatic rollback; those remain separate deployment work.

`docs/TESTING.md`, environment examples, and improvement-status documentation will describe the liveness/readiness distinction and the new AdminClient settings. The existing L-1 circuit-breaker item remains open, with timeout and retry recorded as completed groundwork rather than falsely marking a circuit breaker complete.

## Test strategy

Implementation follows test-driven development.

### PoolManager focused tests

- concurrent misses for one tenant invoke the pool factory once;
- concurrent misses for different tenants are not serialized by one global lock;
- a cache hit refreshes `last_used`;
- an entry remains fresh relative to its most recent touch;
- an expired entry is removed and recreated;
- pool creation failure is not cached and a later request can retry.

### AdminClient focused tests

A local Axum test server will provide deterministic HTTP behavior:

- a transient `503` followed by success is retried and succeeds;
- `404` returns immediately without retry;
- a timed-out GET is retried only to the configured attempt bound;
- repeated transient failures stop at the configured bound;
- migration-status PUT is attempted once even when it returns `503`;
- readiness accepts only a successful backend-admin `/ready` response.

### Health/readiness tests

- liveness returns `200` without dependency state;
- ready control plane maps to `200` and the public response fields;
- failed control plane maps to `503` without an internal error field;
- static/router guards confirm both operational routes remain registered and outside OpenAPI.

### Final verification

- backend-school formatting;
- strict Clippy for all targets and features;
- complete backend-school test suite against the supported disposable PostgreSQL version with required extensions;
- focused Compose/configuration checks;
- repository static guards and documentation reference checks;
- clean Git diff and worktree status.

## Success criteria

The work is complete when:

1. one tenant cannot receive duplicate pools from concurrent cold requests within a process;
2. successful reuse extends the pool cache lifetime from the latest access;
3. backend-admin calls have bounded duration and only safe GET operations retry transient failures;
4. backend-school readiness fails closed when backend-admin is not ready without touching tenant databases;
5. deployment/container checks consume readiness instead of mistaking liveness for dependency readiness;
6. all focused and full verification suites pass without changing tenant schema or frontend behavior.
