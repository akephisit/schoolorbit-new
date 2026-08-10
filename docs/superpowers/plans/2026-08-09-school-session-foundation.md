# School App Server-Side Session Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace browser JWT authentication in the school applications with revocable, tenant-bound opaque sessions that provide one durable authentication foundation for staff, students, and parents.

**Architecture:** Store only SHA-256 session-token digests in each tenant database and validate the owning active user at one router-level Axum boundary. Keep the backend-host-only cookie and session-derived CSRF token out of browser storage, migrate feature handlers from header reparsing to a typed `AuthenticatedSession`, and make SSE/WebSocket revalidation use the same session service. Cut over once after all backend, frontend, realtime, proxy, deployment, and staging gates are ready; do not run JWT and opaque browser authentication concurrently.

**Tech Stack:** Rust 2021, Axum 0.8, SQLx/PostgreSQL, Tokio broadcast, HMAC/SHA-256, bcrypt, SvelteKit 5/Svelte 5, TypeScript, Node test runner, Playwright, Podman Compose, Nginx, Bash/Bats.

## Global Constraints

- Application code scope is only `backend-school/` and `frontend-school/`; root contracts, deployment configuration, scripts, canonical documentation, and this plan may change only where the school-session rollout requires them.
- Tasks produce reviewable commits but are not independently deployable; keep maintenance/cutover blocked until Tasks 1–17 and the final evidence gate are green, then deploy backend-school and frontend-school as the one coordinated rollout described in Operations.
- Do not modify any file under `backend-admin/` or `frontend-admin/`. End every task with `git diff --name-only | rg '^(backend-admin|frontend-admin)/'` and require no output.
- Never edit migrations `001` through `033`; add exactly `backend-school/migrations/034_auth_sessions.sql`.
- Never store or log a plaintext national ID, username throttle key, source address, raw User-Agent, password, cookie, session token, CSRF token, HMAC key, or request body.
- Generate at least 256 random token bits, encode with base64url without padding, store only a 32-byte SHA-256 digest, and compare application-held digests/tokens in constant time.
- Use cookie `__Host-schoolorbit_session` with `Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`, and no `Domain` attribute. A normal session cookie has no `Max-Age`; a new remembered cookie starts at `Max-Age=2592000`, while rotations use only the remaining whole seconds to absolute expiry and never extend it.
- Normal sessions enforce a two-hour idle timeout and twelve-hour absolute timeout. Remembered sessions enforce a seven-day idle timeout and thirty-day absolute timeout. Rotate after fifteen minutes, allow the previous token for sixty seconds, and touch activity at most once every five minutes.
- Protect every authenticated `POST`, `PUT`, `PATCH`, and `DELETE` with the exact tenant Origin and `X-CSRF-Token`; login validates the exact Origin without CSRF, and logout permits a missing/invalid session only for its idempotent cookie-expiry path.
- Derive one stable CSRF token from `tenant_id + session_id` with a dedicated HMAC domain; it remains unchanged across credential rotation so independent browser tabs stay coherent without browser storage or a database CSRF secret.
- Realtime SSE/WebSocket handshakes validate and touch the same authoritative session but defer credential rotation to the next ordinary HTTP request, where credential-maintenance response behavior is observable and testable through the shared fetch transport.
- Login throttles use a fifteen-minute window. Identifier delay starts at failure five and source delay at failure twenty, using `1, 2, 4, 8, 16, 30` seconds and a thirty-second cap.
- Keep multi-device sessions. Support current, selected, and all-session revocation. Password change atomically rotates the current session and revokes every other session.
- New passwords use bcrypt's non-truncating API and must contain 8–128 Unicode scalar values without exceeding 71 UTF-8 bytes; existing bcrypt hashes remain login-compatible.
- Retain expired/revoked session metadata for thirty days, stale throttle rows for one day, and clean both opportunistically in bounded batches; do not add another scheduler.
- `/api/auth/me` contains only id, username, first name, last name, user type, active status, primary role name, profile image file ID, and effective permissions. Keep `/api/auth/me/profile` as the explicit existing profile contract in this project.
- Preserve `ApiResponse<T>`, generated OpenAPI/TypeScript ownership, permission behavior (`401` authentication, `403` authorization/Origin/CSRF, `429` throttle, `503` session-store availability), and existing resource policies.
- Keep the frontend and backend on different origins. Bootstrap remains client-side, the session cookie remains host-only on the backend origin, and no BFF or parent-domain cookie is introduced.
- Do not redesign the public admission portal, account activation, password recovery, MFA, notification authorization, or shared multi-replica event delivery.
- At cutover, keep backend-admin on existing `JWT_SECRET`; map a separately rotated `SCHOOL_ROLLBACK_JWT_SECRET` only to backend-school for emergency rollback. The session-enabled backend does not read either JWT key, and old school JWTs are never accepted.
- Run the exact change-type verification matrix in `.rules`; runtime smoke/E2E credentials come only from environment variables.

---

## File Responsibility Map

- `backend-school/migrations/034_auth_sessions.sql` owns the immutable tenant schema for sessions and login throttle buckets.
- `backend-school/src/modules/auth/config.rs` owns validated session/origin/proxy configuration without printable secrets.
- `backend-school/src/modules/auth/session_crypto.rs` owns raw credential generation/parsing, hashing, and domain-separated CSRF/throttle HMAC derivation.
- `backend-school/src/modules/auth/session_policy.rs` owns durations, rotation/touch decisions, throttle delays, password bounds, and coarse device labels.
- `backend-school/src/modules/auth/session_repository.rs` and `throttle_repository.rs` own SQL and transactions only.
- `backend-school/src/modules/auth/session_service.rs` owns login, validation, cleanup, revocation, current-user loading, and password-change orchestration.
- `backend-school/src/modules/auth/http.rs` owns cookie/header construction and typed HTTP auth outcomes; handlers remain thin.
- `backend-school/src/middleware/session.rs` owns the protected HTTP boundary and inserts `AuthenticatedSession`.
- `backend-school/src/utils/request_context.rs` maps `AuthenticatedSession` to tenant/current-user/actor contexts; its header compatibility functions exist only during Tasks 6–10.
- `frontend-school/src/lib/api/session-security.ts` owns the module-memory CSRF value and transport-header helpers.
- `frontend-school/src/lib/api/client.ts` owns transport status, retry metadata, CSRF capture/injection, and confirmed-`401` handling for every JSON/blob/multipart path.
- `frontend-school/src/lib/api/auth.ts` owns generated auth/session DTO consumption and explicit auth refresh states.
- `frontend-school/src/lib/features/session-security/SessionSecurityPanel.svelte` owns the shared session/password UI used by `/account/security`.
- `backend-school/src/modules/notification/handlers.rs`, `backend-school/src/modules/academic/websockets.rs`, `frontend-school/src/lib/stores/notification.ts`, and timetable socket files own bounded realtime revalidation and reconnect behavior.
- `podman-compose.yml`, `docker-compose.yml`, `nginx-configs/school-api*.template`, installer files, smoke tests, `.rules`, `docs/TESTING.md`, and `docs/OPERATIONS.md` own rollout/runtime truth.

### Task 1: Add the forward-only session and throttle schema

**Files:**
- Create: `backend-school/migrations/034_auth_sessions.sql`
- Create: `backend-school/src/modules/auth/session_schema_tests.rs`
- Modify: `backend-school/src/modules/auth.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: existing `users(id)` in each tenant database and the centralized `run_tenant_migrations` path.
- Produces: `auth_sessions` and `auth_login_throttles` with the columns, indexes, and constraints used by Tasks 3–6.

- [ ] **Step 1: Write failing static and database schema tests**

Add a static test that reads only migration `034`, plus a database test that applies all active migrations to a named isolated schema:

```rust
#[test]
fn auth_session_migration_is_forward_only_and_hash_only() {
    let migration = read_source(manifest_dir().join("migrations/034_auth_sessions.sql"));
    for required in [
        "CREATE TABLE auth_sessions",
        "current_token_hash BYTEA NOT NULL",
        "previous_token_hash BYTEA",
        "CREATE UNIQUE INDEX auth_sessions_current_token_hash_key",
        "CREATE UNIQUE INDEX auth_sessions_previous_token_hash_key",
        "CREATE TABLE auth_login_throttles",
        "PRIMARY KEY (bucket_kind, bucket_hash)",
    ] {
        assert!(migration.contains(required), "missing `{required}`");
    }
    for forbidden in ["raw_token", "csrf_token", "username TEXT", "ip_address", "user_agent"] {
        assert!(!migration.contains(forbidden), "forbidden persisted field `{forbidden}`");
    }
}
```

```rust
use crate::test_helpers::{create_named_test_pool, create_test_user, run_test_migrations};

#[tokio::test]
async fn migration_034_enforces_hash_and_expiry_constraints() {
    let pool = create_named_test_pool("auth_session_schema").await;
    run_test_migrations(&pool).await;

    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'auth_sessions' \
         ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(columns.contains(&"current_token_hash".to_string()));
    assert!(!columns.iter().any(|column| column.contains("token") && column.ends_with("value")));

    let user_id = create_test_user(&pool, "auth-schema@example.test", "test-password")
        .await
        .unwrap();
    let invalid_expiry = sqlx::query(
        "INSERT INTO auth_sessions \
         (user_id, current_token_hash, remember_me, device_label, created_at, last_seen_at, \
          idle_expires_at, absolute_expires_at, rotated_at) \
         VALUES ($1, decode(repeat('00', 32), 'hex'), false, 'Test device', now(), now(), \
                 now() - interval '1 second', now() + interval '1 hour', now())",
    )
    .bind(user_id)
    .execute(&pool)
    .await;
    assert!(invalid_expiry.is_err(), "idle expiry before creation must violate the check");

    let invalid_hash = sqlx::query(
        "INSERT INTO auth_login_throttles \
         (bucket_kind, bucket_hash, failure_count, window_started_at, updated_at) \
         VALUES ('identifier', decode('00', 'hex'), 1, now(), now())",
    )
    .execute(&pool)
    .await;
    assert!(invalid_hash.is_err(), "one-byte throttle hashes must violate the check");
}
```

Register `#[cfg(test)] mod session_schema_tests;` from `auth.rs`.

- [ ] **Step 2: Run the focused tests and verify the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture auth_session_migration_is_forward_only_and_hash_only -- --exact
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_schema_tests::migration_034_enforces_hash_and_expiry_constraints --bin backend-school -- --exact --nocapture
```

Expected: the static test fails because `034_auth_sessions.sql` is absent; the database test fails because `auth_sessions` does not exist.

- [ ] **Step 3: Add the immutable migration**

Create the migration with this complete schema contract:

```sql
CREATE TABLE auth_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    current_token_hash BYTEA NOT NULL CHECK (octet_length(current_token_hash) = 32),
    previous_token_hash BYTEA,
    previous_token_valid_until TIMESTAMPTZ,
    remember_me BOOLEAN NOT NULL,
    device_label TEXT NOT NULL CHECK (btrim(device_label) <> ''),
    created_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    idle_expires_at TIMESTAMPTZ NOT NULL,
    absolute_expires_at TIMESTAMPTZ NOT NULL,
    rotated_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revocation_reason TEXT,
    CHECK (idle_expires_at > created_at),
    CHECK (absolute_expires_at > created_at),
    CHECK (last_seen_at >= created_at),
    CHECK (rotated_at >= created_at),
    CHECK (idle_expires_at <= absolute_expires_at),
    CHECK ((previous_token_hash IS NULL) = (previous_token_valid_until IS NULL)),
    CHECK (previous_token_hash IS NULL OR octet_length(previous_token_hash) = 32),
    CHECK (previous_token_valid_until IS NULL OR previous_token_valid_until > rotated_at),
    CHECK (revocation_reason IS NULL OR revoked_at IS NOT NULL)
);

CREATE UNIQUE INDEX auth_sessions_current_token_hash_key
    ON auth_sessions (current_token_hash);
CREATE UNIQUE INDEX auth_sessions_previous_token_hash_key
    ON auth_sessions (previous_token_hash)
    WHERE previous_token_hash IS NOT NULL;
CREATE INDEX auth_sessions_active_user_expiry_idx
    ON auth_sessions (user_id, absolute_expires_at, id)
    WHERE revoked_at IS NULL;
CREATE INDEX auth_sessions_cleanup_idx
    ON auth_sessions (
        (COALESCE(revoked_at, LEAST(idle_expires_at, absolute_expires_at))),
        id
    );

CREATE TABLE auth_login_throttles (
    bucket_kind TEXT NOT NULL CHECK (bucket_kind IN ('identifier', 'source')),
    bucket_hash BYTEA NOT NULL CHECK (octet_length(bucket_hash) = 32),
    failure_count INTEGER NOT NULL CHECK (failure_count >= 0),
    window_started_at TIMESTAMPTZ NOT NULL,
    blocked_until TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (bucket_kind, bucket_hash),
    CHECK (updated_at >= window_started_at),
    CHECK (blocked_until IS NULL OR blocked_until >= window_started_at)
);

CREATE INDEX auth_login_throttles_cleanup_idx
    ON auth_login_throttles (updated_at, bucket_kind, bucket_hash);
```

- [ ] **Step 4: Run schema tests and migration timeline checks**

Run:

```bash
cd backend-school
cargo test --test static_architecture auth_session_migration_is_forward_only_and_hash_only -- --exact --nocapture
cargo test --test static_architecture active_migrations_are_clean_sequential_timeline -- --exact --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_schema_tests --bin backend-school -- --nocapture
```

Expected: all selected tests pass; the named test schema contains migration versions `001` through `034` and rejects invalid hashes/expiries.

- [ ] **Step 5: Commit the schema slice**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/migrations/034_auth_sessions.sql backend-school/src/modules/auth.rs backend-school/src/modules/auth/session_schema_tests.rs backend-school/tests/static_architecture.rs
git commit -m "feat(auth): add tenant session schema"
```

### Task 2: Add validated auth configuration, credential crypto, lifecycle policy, and source handling

**Files:**
- Create: `backend-school/src/modules/auth/config.rs`
- Create: `backend-school/src/modules/auth/session_crypto.rs`
- Create: `backend-school/src/modules/auth/session_policy.rs`
- Create: `backend-school/src/utils/client_address.rs`
- Modify: `backend-school/src/modules/auth.rs`
- Modify: `backend-school/src/utils.rs`
- Modify: `backend-school/src/error.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`

**Interfaces:**
- Consumes: `SESSION_HMAC_KEY`, `BASE_DOMAIN`, optional comma-separated `TRUSTED_PROXY_CIDRS`, and optional comma-separated `SCHOOL_ALLOWED_DEV_ORIGINS`.
- Produces: `SessionConfig::from_env()`, `RawSessionToken`, `EncodedSessionToken`, `TokenHash`, `ThrottleBucketHash`, `SessionLifetime`, `SessionTimes`, `ThrottlePolicy`, `client_address`, `device_label`, `AppError::RateLimited`, and `AppError::PayloadTooLarge`.

- [ ] **Step 1: Write failing pure tests for every security boundary**

Place focused `#[cfg(test)]` modules beside their implementation targets with these assertions:

```rust
#[test]
fn token_round_trip_is_32_bytes_and_debug_is_redacted() {
    let token = RawSessionToken::generate().unwrap();
    let encoded = token.encode();
    assert_eq!(RawSessionToken::parse(encoded.expose_for_cookie()).unwrap().as_bytes().len(), 32);
    assert_eq!(format!("{token:?}"), "RawSessionToken([REDACTED])");
    assert!(!format!("{encoded:?}").contains(encoded.expose_for_cookie()));
}

#[test]
fn hmac_domains_do_not_overlap() {
    let key = SessionHmacKey::for_tests([7_u8; 32]);
    let tenant = Uuid::new_v4();
    let session = Uuid::new_v4();
    let csrf = session_csrf_token(&key, tenant, session);
    assert_eq!(csrf, session_csrf_token(&key, tenant, session));
    assert_ne!(
        csrf.expose_for_header(),
        session_csrf_token(&key, Uuid::new_v4(), session).expose_for_header()
    );
    assert_ne!(
        csrf.expose_for_header(),
        session_csrf_token(&key, tenant, Uuid::new_v4()).expose_for_header()
    );
    assert_ne!(
        csrf.expose_for_header(),
        URL_SAFE_NO_PAD.encode(identifier_bucket(&key, tenant, "teacher.one").as_bytes())
    );
    assert_ne!(
        identifier_bucket(&key, Uuid::nil(), "teacher.one"),
        source_bucket(&key, Uuid::nil(), "203.0.113.9".parse().unwrap())
    );
}

#[test]
fn normal_and_remembered_lifetimes_match_contract() {
    assert_eq!(SessionLifetime::normal().idle, Duration::hours(2));
    assert_eq!(SessionLifetime::normal().absolute, Duration::hours(12));
    assert_eq!(SessionLifetime::remembered().idle, Duration::days(7));
    assert_eq!(SessionLifetime::remembered().absolute, Duration::days(30));
}

#[test]
fn throttle_delay_starts_at_owned_thresholds_and_caps() {
    let policy = ThrottlePolicy::default();
    assert_eq!(policy.delay(BucketKind::Identifier, 4), None);
    assert_eq!(policy.delay(BucketKind::Identifier, 5), Some(Duration::seconds(1)));
    assert_eq!(policy.delay(BucketKind::Identifier, 10), Some(Duration::seconds(30)));
    assert_eq!(policy.delay(BucketKind::Source, 19), None);
    assert_eq!(policy.delay(BucketKind::Source, 20), Some(Duration::seconds(1)));
}

#[test]
fn forwarded_address_is_used_only_for_a_trusted_peer() {
    let headers = HeaderMap::from_iter([(
        HeaderName::from_static("x-real-ip"),
        HeaderValue::from_static("203.0.113.9"),
    )]);
    let trusted: Vec<IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
    assert_eq!(
        client_address("10.88.0.4:41234".parse().unwrap(), &headers, &trusted),
        "203.0.113.9".parse::<IpAddr>().unwrap()
    );
    assert_eq!(
        client_address("198.51.100.7:41234".parse().unwrap(), &headers, &trusted),
        "198.51.100.7".parse::<IpAddr>().unwrap()
    );
}
```

Add config tests for a key shorter than 32 bytes, invalid CIDR, non-origin development URL, and valid production values. Add device-label cases for Chrome/Windows, Safari/iOS, Firefox/Linux, and unknown values; assert the raw User-Agent never appears in the result.

Add token/CSRF parser cases for padding, non-base64url characters, decoded lengths 31/33, quoted/whitespace values, and duplicate headers/cookies. Valid encoded credentials are exactly 43 base64url characters; invalid inputs return fixed auth/CSRF errors and never enter a log.

Add address cases proving duplicate/malformed `X-Real-IP` falls back to the direct peer, `X-Forwarded-For` is never trusted by this helper, IPv4/IPv6 are accepted only as one bare address, IPv4-mapped IPv6 normalizes to the same IPv4 bucket, and no header value appears in Debug/tracing output.

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
cd backend-school
cargo test modules::auth::session_crypto::tests --bin backend-school -- --nocapture
cargo test modules::auth::session_policy::tests --bin backend-school -- --nocapture
cargo test modules::auth::config::tests --bin backend-school -- --nocapture
cargo test utils::client_address::tests --bin backend-school -- --nocapture
```

Expected: compilation fails because the four modules and the two new `AppError` variants do not exist.

- [ ] **Step 3: Implement the exact domain types and constants**

Keep the existing direct `cookie`, `url`, `subtle`, `rand`, `hmac`, `sha2`, and `base64` dependencies; add only `ipnet = "2"` and `zeroize = { version = "1", features = ["derive"] }`. Implement these signatures and constants:

```rust
pub const SESSION_COOKIE_NAME: &str = "__Host-schoolorbit_session";
pub const LEGACY_COOKIE_NAME: &str = "auth_token";
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";

pub struct SessionConfig {
    hmac_key: SessionHmacKey,
    pub base_domain: String,
    pub trusted_proxy_cidrs: Vec<IpNet>,
    pub allowed_dev_origins: HashSet<String>,
}

impl SessionConfig {
    pub fn from_env() -> Result<Self, AppError>;
    pub fn hmac_key(&self) -> &SessionHmacKey;
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct RawSessionToken([u8; 32]);

impl RawSessionToken {
    pub fn generate() -> Result<Self, AppError>;
    pub fn parse(value: &str) -> Result<Self, AppError>;
    pub fn encode(&self) -> EncodedSessionToken;
    pub fn token_hash(&self) -> TokenHash;
    pub fn as_bytes(&self) -> &[u8; 32];
}

#[derive(Clone, Copy, Eq)]
pub struct TokenHash([u8; 32]);

impl TokenHash {
    pub fn as_bytes(&self) -> &[u8; 32];
}

impl PartialEq for TokenHash {
    fn eq(&self, other: &Self) -> bool;
}

#[derive(Eq, Zeroize, ZeroizeOnDrop)]
pub struct CsrfToken([u8; 32]);

impl CsrfToken {
    pub fn expose_for_header(&self) -> String;
}

impl PartialEq for CsrfToken {
    fn eq(&self, other: &Self) -> bool;
}

#[derive(Clone, Copy, Eq)]
pub struct ThrottleBucketHash([u8; 32]);

impl ThrottleBucketHash {
    pub fn as_bytes(&self) -> &[u8; 32];
}

impl PartialEq for ThrottleBucketHash {
    fn eq(&self, other: &Self) -> bool;
}

pub fn session_csrf_token(
    key: &SessionHmacKey,
    tenant_id: Uuid,
    session_id: Uuid,
) -> CsrfToken;

pub fn identifier_bucket(
    key: &SessionHmacKey,
    tenant_id: Uuid,
    normalized_username: &str,
) -> ThrottleBucketHash;

pub fn source_bucket(
    key: &SessionHmacKey,
    tenant_id: Uuid,
    source: IpAddr,
) -> ThrottleBucketHash;
```

Use `rand::rngs::OsRng.try_fill_bytes`, `URL_SAFE_NO_PAD`, `Sha256`, `Hmac<Sha256>`, separate labels `schoolorbit/session/csrf/v1`, `schoolorbit/login/identifier/v1`, and `schoolorbit/login/source/v1`, and `subtle::ConstantTimeEq`. Feed the CSRF HMAC the fixed 16-byte tenant UUID followed by the fixed 16-byte session UUID; never use printable concatenation. Implement equality for `TokenHash`, `CsrfToken`, and `ThrottleBucketHash` with `ct_eq`, not derived byte equality. Make `SessionHmacKey`, `RawSessionToken`, `EncodedSessionToken`, and `CsrfToken` zeroize on drop. Implement custom `Debug` that prints only `[REDACTED]`; expose encoded values only through `EncodedSessionToken::expose_for_cookie()` and `CsrfToken::expose_for_header()`.

```rust
pub const ROTATION_INTERVAL: Duration = Duration::minutes(15);
pub const PREVIOUS_TOKEN_GRACE: Duration = Duration::seconds(60);
pub const TOUCH_INTERVAL: Duration = Duration::minutes(5);
pub const THROTTLE_WINDOW: Duration = Duration::minutes(15);
pub const SESSION_RETENTION: Duration = Duration::days(30);
pub const THROTTLE_RETENTION: Duration = Duration::days(1);
pub const CLEANUP_BATCH_SIZE: i64 = 100;

pub struct SessionLifetime { pub idle: Duration, pub absolute: Duration }
pub struct SessionTimes { pub created_at: DateTime<Utc>, pub last_seen_at: DateTime<Utc>, pub idle_expires_at: DateTime<Utc>, pub absolute_expires_at: DateTime<Utc>, pub rotated_at: DateTime<Utc> }
pub enum BucketKind { Identifier, Source }
pub struct ThrottlePolicy;

pub fn normalize_login_identifier(value: &str) -> String;
pub fn device_label(user_agent: Option<&str>) -> String;
pub fn cookie_max_age_seconds(
    remember_me: bool,
    absolute_expires_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<u64>;
pub fn validate_login_input(username: &str, password: &str) -> Result<(), AppError>;
pub fn validate_new_password(value: &str) -> Result<(), AppError>;
```

`cookie_max_age_seconds` returns `None` for a normal session and the floored positive remaining seconds capped at `2592000` for a remembered session; it never rounds up beyond absolute expiry. Add login/day-29/expired boundary tests.

Login input accepts a non-empty identifier of at most 100 Unicode scalars/400 UTF-8 bytes and a non-empty password of at most 1,024 bytes; structurally invalid credentials still receive the generic public login error and never reach a tenant query with an unbounded value. Password validation accepts 8 through 128 Unicode scalar values and at most 71 UTF-8 bytes, the non-truncating input limit for the project's bcrypt version. Add tests proving 71 ASCII bytes pass, 72 fail, 23 three-byte Thai scalars pass the byte boundary, and 24 fail; password-change hashing must use `bcrypt::non_truncating_hash` so no two accepted passwords can collapse through bcrypt truncation. Existing login verification continues to use `bcrypt::verify` for compatibility with already-stored hashes. Add `AppError::RateLimited { retry_after_seconds: u64 }` and `AppError::PayloadTooLarge`; their `IntoResponse` branches return `429` plus validated `Retry-After`, or `413`, respectively, using the standard fixed error envelope without logging identifiers or bodies. Compute retry seconds by ceiling the positive `blocked_until - now` duration to at least one and at most thirty. Centralize testable error semantics in `AppError::status_code() -> StatusCode`, `AppError::public_message() -> &str`, and `AppError::retry_after_seconds() -> Option<u64>`; `IntoResponse` must use those same methods.

- [ ] **Step 4: Run focused tests and security scans**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test modules::auth::session_crypto::tests --bin backend-school -- --nocapture
cargo test modules::auth::session_policy::tests --bin backend-school -- --nocapture
cargo test modules::auth::config::tests --bin backend-school -- --nocapture
cargo test utils::client_address::tests --bin backend-school -- --nocapture
cargo test error::tests --bin backend-school -- --nocapture
rg -n "derive\([^)]*Debug[^)]*\)" src/modules/auth/config.rs src/modules/auth/session_crypto.rs
```

Expected: tests pass; the final `rg` prints no secret-holding type with derived `Debug`.

- [ ] **Step 5: Commit the pure auth foundation**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/Cargo.toml backend-school/Cargo.lock backend-school/src/error.rs backend-school/src/modules/auth.rs backend-school/src/modules/auth/config.rs backend-school/src/modules/auth/session_crypto.rs backend-school/src/modules/auth/session_policy.rs backend-school/src/utils.rs backend-school/src/utils/client_address.rs
git commit -m "feat(auth): add session security primitives"
```

### Task 3: Implement transactional session and throttle repositories

**Files:**
- Create: `backend-school/src/modules/auth/session_repository.rs`
- Create: `backend-school/src/modules/auth/throttle_repository.rs`
- Create: `backend-school/src/modules/auth/session_repository_tests.rs`
- Modify: `backend-school/src/modules/auth.rs`
- Modify: `backend-school/src/test_helpers.rs`

**Interfaces:**
- Consumes: migration `034`, `TokenHash`, `ThrottleBucketHash`, `SessionTimes`, `ThrottlePolicy`, `CLEANUP_BATCH_SIZE`, and a tenant `PgPool`.
- Produces: `SessionRow`, `NewSession`, `MaintainedSession`, `SessionRevocationTarget`, `ThrottleState`, `check_login_throttles`, `record_login_failure`, `create_login_session`, `authenticate_and_maintain`, `revalidate_session`, `list_user_sessions`, `revoke_sessions`, and `cleanup_auth_state`.

- [ ] **Step 1: Write failing database tests for ownership, expiry, concurrency, throttle, and cleanup**

Add `create_named_test_pool_with_max_connections(test_name, max_connections)` to `test_helpers.rs`, preserving the same direct `TEST_DATABASE_URL`, schema reset, and search-path isolation behavior. Reject zero connections in the helper. Create ordinary fixtures with one connection and rotation/collision fixtures with at least four so their barriers reach database locks concurrently.

Create named-schema tests with this matrix and exact observable assertions:

```rust
#[tokio::test]
async fn concurrent_rotation_keeps_one_current_hash_and_one_grace_hash() {
    let fixture = SessionFixture::concurrent("concurrent_rotation", 4).await;
    let old = fixture.raw_token.token_hash();
    fixture.set_rotated_at(Utc::now() - Duration::minutes(16)).await;
    let now = Utc::now();
    let barrier = Arc::new(Barrier::new(2));

    let (left, right) = tokio::join!(
        async {
            barrier.wait().await;
            authenticate_and_maintain(
                &fixture.pool, old, now, SessionMaintenanceMode::RotateAndTouch,
                || RawSessionToken::from_bytes([1; 32]),
            ).await
        },
        async {
            barrier.wait().await;
            authenticate_and_maintain(
                &fixture.pool, old, now, SessionMaintenanceMode::RotateAndTouch,
                || RawSessionToken::from_bytes([2; 32]),
            ).await
        },
    );

    assert!(left.is_ok());
    assert!(right.is_ok());
    let left = left.unwrap().expect("left request must authenticate");
    let right = right.unwrap().expect("right request must authenticate");
    assert_eq!(
        left.replacement.is_some() as usize + right.replacement.is_some() as usize,
        1
    );
    let row = fixture.reload().await;
    assert_eq!(row.previous_token_hash, Some(old));
    assert_ne!(row.current_token_hash, old);
    assert_eq!(row.rotated_at, left.rotated_at.max(right.rotated_at));
}

#[tokio::test]
async fn selected_revocation_cannot_cross_user_ownership() {
    let owner = SessionFixture::active("owner_session").await;
    let other = owner.create_other_user_session().await;
    let revoked = revoke_sessions(
        &owner.pool,
        owner.user_id,
        SessionRevocationTarget::Session(other.session_id),
        SessionRevocationReason::UserSelected,
        Utc::now(),
    )
    .await
    .unwrap();
    assert!(revoked.is_empty());
    assert!(other.reload().await.revoked_at.is_none());
}

#[tokio::test]
async fn failure_buckets_update_atomically_and_success_clears_only_identifier() {
    let fixture = SessionFixture::active("throttle_atomicity").await;
    record_login_failure(&fixture.pool, fixture.identifier_hash, fixture.source_hash, Utc::now())
        .await
        .unwrap();
    assert_eq!(fixture.failure_count(BucketKind::Identifier).await, 1);
    assert_eq!(fixture.failure_count(BucketKind::Source).await, 1);

    fixture.create_login_session_and_clear_identifier().await;
    assert_eq!(fixture.failure_count(BucketKind::Identifier).await, 0);
    assert_eq!(fixture.failure_count(BucketKind::Source).await, 1);
}
```

Add separate cases for current hash, previous hash inside/outside sixty seconds, revoked row, inactive owning user, idle expiry, absolute expiry, five-minute touch, `TouchOnly` activity without generator invocation or hash change, active-session-only listing, current/all revocation, thirty-day session cleanup, one-day throttle cleanup, and a cleanup limit of exactly 100 rows.

- [ ] **Step 2: Run repository tests and verify the red state**

Run:

```bash
cd backend-school
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_repository_tests --bin backend-school -- --nocapture
```

Expected: compilation fails because repository functions and `SessionFixture` are not defined.

- [ ] **Step 3: Implement rows, transactions, and bounded cleanup**

Use these public types and signatures:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentedTokenKind { Current, Previous }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMaintenanceMode { RotateAndTouch, TouchOnly }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRevocationTarget {
    Session(Uuid),
    User { except_session_id: Option<Uuid> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRevocationReason {
    Logout,
    UserSelected,
    LogoutAll,
    PasswordChanged,
}

pub struct NewSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub current_token_hash: TokenHash,
    pub remember_me: bool,
    pub device_label: String,
    pub times: SessionTimes,
}

pub struct MaintainedSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub user_type: String,
    pub presented_as: PresentedTokenKind,
    pub remember_me: bool,
    pub rotated_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
    pub replacement: Option<RawSessionToken>,
}

pub async fn authenticate_and_maintain<F>(
    pool: &PgPool,
    presented_hash: TokenHash,
    now: DateTime<Utc>,
    maintenance: SessionMaintenanceMode,
    generate: F,
) -> Result<Option<MaintainedSession>, AppError>
where F: FnOnce() -> RawSessionToken;

pub async fn revalidate_session(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<bool, AppError>;

pub async fn revoke_sessions(
    pool: &PgPool,
    user_id: Uuid,
    target: SessionRevocationTarget,
    reason: SessionRevocationReason,
    now: DateTime<Utc>,
) -> Result<Vec<Uuid>, AppError>;
```

First perform an indexed active read. Only enter `SELECT ... FOR UPDATE` when permitted rotation, touch, or expired previous-token cleanup is due; re-read by `current_token_hash = $1 OR previous_token_hash = $1` after acquiring the lock. A previous token authenticates only while `previous_token_valid_until > now()` and never rotates again. `TouchOnly` may update activity and clear expired previous-token fields but must never invoke the credential generator or change either token hash. Set idle expiry to `LEAST(now + idle_duration, absolute_expires_at)`.

Before insert or rotation, acquire a transaction-scoped PostgreSQL advisory lock derived from a fixed domain tag plus the first eight digest bytes, then reject a generated digest already present in either token-hash column and reject a replacement equal to the current digest. This preserves concurrency for unrelated tokens while closing the cross-column race that two separate unique indexes cannot cover. Any collision aborts the operation with a fixed `503` reason and never exposes a digest; its probability for the production CSPRNG is negligible, while deterministic tests can prove rollback. Cover current/current, current/previous, previous/previous, same-row replacement, and concurrent same-digest attempts in repository tests.

Implement throttle operations as one transaction for both failure buckets. Reset a bucket when `window_started_at <= now - 15 minutes`; set `blocked_until` from `ThrottlePolicy`; never store its input. `create_login_session` inserts the session and deletes only the matching identifier row before one commit.

Persist revocation text only through `SessionRevocationReason::as_str()` (`logout`, `user_selected`, `logout_all`, `password_changed`); no handler/service accepts a free-form reason. Use delete CTEs with ordered `LIMIT 100` selections for sessions and throttle rows, then delete by primary key. Map auth-table SQL failures to `AppError::ServiceUnavailable("session_store".to_string())` so callers return `503` without exposing SQL text.

- [ ] **Step 4: Run database tests and inspect SQL boundaries**

Run:

```bash
cd backend-school
cargo fmt --all
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_repository_tests --bin backend-school -- --nocapture
rg -n "username|ip_address|user_agent|raw_token|csrf_token" src/modules/auth/session_repository.rs src/modules/auth/throttle_repository.rs migrations/034_auth_sessions.sql
```

Expected: all repository tests pass; the scan prints no persisted raw throttle/session field (references in type/function names must be reviewed and contain no SQL column or log).

- [ ] **Step 5: Commit the persistence slice**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/src/modules/auth.rs backend-school/src/modules/auth/session_repository.rs backend-school/src/modules/auth/throttle_repository.rs backend-school/src/modules/auth/session_repository_tests.rs backend-school/src/test_helpers.rs
git commit -m "feat(auth): persist revocable sessions"
```

### Task 4: Orchestrate login, validation, revocation, cleanup, and atomic password change

**Files:**
- Create: `backend-school/src/modules/auth/audit.rs`
- Create: `backend-school/src/modules/auth/events.rs`
- Create: `backend-school/src/modules/auth/session_service.rs`
- Create: `backend-school/src/modules/auth/session_service_tests.rs`
- Modify: `backend-school/src/modules/auth/session_repository.rs`
- Modify: `backend-school/src/modules/auth/services.rs`
- Modify: `backend-school/src/modules/auth.rs`
- Delete: `backend-school/src/modules/auth/tests.rs`

**Interfaces:**
- Consumes: repository functions from Task 3, `get_cached_user_permissions`, active `users`, `roles`, bcrypt, and the session/throttle policy from Task 2.
- Produces: fixed-field redacted auth audit helpers, `AuthenticatedSession`, `SessionAuthentication`, `LoginCommand`, `LoginResult`, `PasswordChangeResult`, `SessionRevocationEvent`, `login`, `authenticate(..., SessionMaintenanceMode)`, `load_current_user`, `logout`, `list_sessions`, `revoke_selected`, `logout_all`, `change_password`, and `revalidate`.

- [ ] **Step 1: Write failing service tests for enumeration resistance and atomic behavior**

Create database-backed tests that use a deterministic credential generator only inside tests:

```rust
#[tokio::test]
async fn unknown_wrong_and_inactive_logins_share_one_public_error() {
    let fixture = AuthServiceFixture::new("generic_login_error").await;
    fixture.insert_user("inactive.user", "correct-password", "inactive").await;
    fixture.insert_user("active.user", "correct-password", "active").await;

    let errors = [
        fixture.login("missing.user", "wrong").await.unwrap_err(),
        fixture.login("inactive.user", "correct-password").await.unwrap_err(),
        fixture.login("active.user", "wrong").await.unwrap_err(),
    ];
    for error in errors {
        assert_eq!(error.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(error.public_message(), "ชื่อผู้ใช้หรือรหัสผ่านไม่ถูกต้อง");
    }
    assert_eq!(fixture.session_count().await, 0);
}

#[tokio::test]
async fn fifth_identifier_failure_and_twentieth_source_failure_return_retry_after() {
    let fixture = AuthServiceFixture::new("login_throttle_thresholds").await;
    fixture.insert_user("teacher.one", "correct-password", "active").await;

    for expected in 1..=5 {
        let result = fixture.login_from("teacher.one", "wrong", "203.0.113.9").await;
        if expected < 5 {
            assert_eq!(result.unwrap_err().status_code(), StatusCode::UNAUTHORIZED);
        } else {
            assert_eq!(result.unwrap_err().retry_after_seconds(), Some(1));
        }
    }

    fixture.clear_identifier_bucket("teacher.one").await;
    for expected in 1..=20 {
        let result = fixture.login_from(
            &format!("unknown-{expected}"),
            "wrong",
            "198.51.100.4",
        ).await;
        if expected == 20 {
            assert_eq!(result.unwrap_err().retry_after_seconds(), Some(1));
        }
    }
}

#[tokio::test]
async fn password_change_commits_hash_rotation_and_other_revocations_together() {
    let fixture = AuthServiceFixture::new("password_change_atomicity").await;
    let current = fixture.login("teacher.one", "old-password").await.unwrap();
    let other = fixture.login("teacher.one", "old-password").await.unwrap();

    let result = change_password(
        &fixture.context,
        &current.authenticated,
        "old-password",
        "new-password-123",
        Utc::now(),
        fixture.credentials([8; 32]),
    )
    .await
    .unwrap();

    assert_eq!(result.revoked_session_ids, vec![other.authenticated.session_id]);
    assert!(fixture.password_verifies("new-password-123").await);
    assert!(fixture.session_is_active(current.authenticated.session_id).await);
    assert!(!fixture.session_is_active(other.authenticated.session_id).await);
    assert_ne!(result.credential.token_hash(), current.credential.token_hash());
}
```

Add a rollback test that forces the replacement hash to collide with another row and asserts the old password plus both sessions remain unchanged. Add tests for successful identifier reset, blocked correct credentials, primary role/permissions loaded before insert, session-list ownership, selected/current/all revocation, cleanup failure being logged only as a fixed reason code, and audit capture proving rejected login fields never contain username/source/bucket hashes while committed creation/revocation events contain only tenant/user/session IDs plus an allowlisted reason code.

Add password error-semantics cases: an incorrect current password or invalid new password returns `AppError::BadRequest`/`400` and leaves the authenticated session/user state untouched; a same-session concurrent password-hash change returns `409` without overwriting it; only an inactive user or no-longer-active current session returns `401`. This prevents the frontend's confirmed-`401` handler from logging out a still-valid user on form mistakes.

Delete the old setup-only `modules/auth/tests.rs` (it never called a handler) and remove its module declaration; the schema/repository/service/HTTP tests in Tasks 1–6 replace it with executable assertions.

- [ ] **Step 2: Run service tests and verify the red state**

Run:

```bash
cd backend-school
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_service_tests --bin backend-school -- --nocapture
```

Expected: compilation fails because `session_service`, its result types, and event types do not exist.

- [ ] **Step 3: Implement the service boundary and post-commit events**

Define the identity and result contracts exactly once:

```rust
#[derive(Clone)]
pub struct AuthenticatedSession {
    pub tenant: TenantContext,
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub user_type: String,
}

pub struct SessionAuthentication {
    pub authenticated: AuthenticatedSession,
    pub csrf_token: CsrfToken,
    pub replacement: Option<SessionCredential>,
}

pub struct SessionCredential {
    raw: RawSessionToken,
    pub cookie_max_age_seconds: Option<u64>,
}

impl SessionCredential {
    pub fn encoded(&self) -> EncodedSessionToken;
    pub fn token_hash(&self) -> TokenHash;
}

pub struct LoginCommand<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub remember_me: bool,
    pub source: IpAddr,
    pub user_agent: Option<&'a str>,
    pub now: DateTime<Utc>,
}

pub struct LoginUserSnapshot {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    pub status: String,
    pub primary_role_name: Option<String>,
    pub profile_image_file_id: Option<Uuid>,
    pub permissions: Vec<String>,
}

pub struct LoginResult {
    pub user: LoginUserSnapshot,
    pub authenticated: AuthenticatedSession,
    pub credential: SessionCredential,
}

pub struct SessionRevocationResult {
    pub revoked_session_ids: Vec<Uuid>,
    pub current_revoked: bool,
}

pub struct PasswordChangeResult {
    pub credential: SessionCredential,
    pub revoked_session_ids: Vec<Uuid>,
}
```

Use an optional user lookup that does not filter status. Select the real hash or fixed non-secret dummy bcrypt hash `$2b$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy`, then call `bcrypt::verify` through `tokio::task::spawn_blocking`. Check both throttle buckets before verification and again immediately before session creation. Unknown, inactive, and wrong-password paths record both bucket failures in one transaction, then return the same message; a newly active delay returns `RateLimited`.

Load the primary role and effective permissions before `create_login_session`; clear the identifier bucket in the insert transaction. Run bounded cleanup after a committed login and session-list read. Cleanup errors emit only `tracing::warn!(reason = "auth_cleanup_failed")` and do not undo the user operation.

Every login/authenticate/password result obtains its CSRF value only through `session_csrf_token(config.hmac_key(), tenant.tenant_id, session_id)`. Rotation changes the raw credential/hash but not this logical-session value; add a test that login, current-token authentication, previous-token authentication, and password-change rotation all yield the same CSRF header for one session, while another session or tenant differs.

Every `SessionCredential` computes `cookie_max_age_seconds` from the session's fixed `absolute_expires_at` at the moment the credential is issued. Normal credentials use `None`; remembered login starts at most thirty days, and later authenticate/password rotations shrink the value. Add a day-29 rotation test proving a replacement cookie cannot extend the server's absolute lifetime.

`load_current_user` re-reads the active user's approved shell fields by `session.user_id`, loads the primary active role plus `get_cached_user_permissions`, and returns `LoginUserSnapshot`; it never calls the full profile/decryption path. Add a test that status changing to inactive between middleware authentication and this read returns `401` and no PII field is selected.

Extend the repository with this password transaction contract so `session_service.rs` contains no SQL:

```rust
pub struct PasswordChangeSnapshot {
    pub password_hash: String,
}

pub struct LockedPasswordChange {
    pub password_hash: String,
    pub remember_me: bool,
    pub absolute_expires_at: DateTime<Utc>,
}

pub async fn load_password_change_snapshot(
    pool: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<PasswordChangeSnapshot, AppError>;

pub async fn lock_password_change<'a>(
    tx: &mut Transaction<'a, Postgres>,
    user_id: Uuid,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<LockedPasswordChange, AppError>;

pub async fn apply_password_change<'a>(
    tx: &mut Transaction<'a, Postgres>,
    user_id: Uuid,
    session_id: Uuid,
    new_password_hash: &str,
    new_token_hash: TokenHash,
    now: DateTime<Utc>,
) -> Result<Vec<Uuid>, AppError>;
```

Validate 8–128 scalars plus the 71-byte non-truncating bound, load `PasswordChangeSnapshot`, verify the current password, hash the new password with `bcrypt::non_truncating_hash`, and generate the replacement credential through `spawn_blocking`/CSPRNG before opening the write transaction. Then lock the user and current-session rows, require the locked hash to equal the verified snapshot and the session to remain active, and abort without mutation if either changed concurrently. Update the password, revoke other sessions with `password_changed`, rotate current with a sixty-second previous-token grace, and commit before returning the replacement credential. No bcrypt work may run while a database row lock is held.

Define redacted process-local revocation signals:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRevocationEvent {
    pub tenant: String,
    pub user_id: Uuid,
    pub target: SessionRevocationTarget,
}

impl SessionRevocationEvent {
    pub fn session(tenant: &str, user_id: Uuid, session_id: Uuid) -> Self;
    pub fn user(tenant: &str, user_id: Uuid, except_session_id: Option<Uuid>) -> Self;
    pub fn applies_to(&self, tenant: &str, user_id: Uuid, session_id: Uuid) -> bool;
}
```

Send events only after committed selected/current/all/password-change revocations. Treat a broadcast channel with no receivers as an expected condition and log no credential data.

Create `audit.rs` with typed helpers rather than ad hoc tracing fields. Allow only event names `login_rejected`, `login_succeeded`, `session_created`, `session_revoked`, `password_sessions_revoked`, `session_rotation_failed`, `origin_rejected`, `csrf_rejected`, and `session_realtime_disconnect`, with reason codes represented by enums. Login rejection before success logs tenant ID and a generic category but never username, normalized identifier, source address, bucket hash, User-Agent, or password; committed events may add user/session UUIDs. Emit success/creation/revocation/password events only after commit. Repository/SQL error text never enters these events.

- [ ] **Step 4: Run service and repository tests**

Run:

```bash
cd backend-school
cargo fmt --all
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_service_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_repository_tests --bin backend-school -- --nocapture
rg -n "Login attempt|username\s*=|password\s*=|user_agent\s*=|source\s*=" src/modules/auth
```

Expected: tests pass; the scan finds only typed fields/test fixtures and no tracing statement containing those values.

- [ ] **Step 5: Commit the auth service slice**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/src/modules/auth.rs backend-school/src/modules/auth/audit.rs backend-school/src/modules/auth/events.rs backend-school/src/modules/auth/services.rs backend-school/src/modules/auth/session_repository.rs backend-school/src/modules/auth/session_service.rs backend-school/src/modules/auth/session_service_tests.rs
git add -u backend-school/src/modules/auth/tests.rs
git commit -m "feat(auth): orchestrate school sessions"
```

### Task 5: Add typed session HTTP handlers, cookies, and OpenAPI contracts without routing them yet

**Files:**
- Create: `backend-school/src/modules/auth/http.rs`
- Create: `backend-school/src/modules/auth/runtime.rs`
- Create: `backend-school/src/modules/auth/session_handlers.rs`
- Create: `backend-school/src/modules/auth/session_http_tests.rs`
- Modify: `backend-school/src/modules/auth/models.rs`
- Modify: `backend-school/src/modules/auth.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/src/utils/subdomain.rs`
- Modify: `backend-school/src/utils/tenant.rs`
- Modify: `backend-school/src/main.rs`

**Interfaces:**
- Consumes: Task 4 services, standard `ApiResponse<T>`, `ConnectInfo<SocketAddr>`, tenant resolution, `SessionConfig`, permission cache, and the process-local session event sender.
- Produces: `AuthRuntime`, cookie helpers, exact-Origin helpers, seven typed but unrouted auth/session endpoints, and their final component schemas. `main.rs` constructs the runtime but still routes/documents the old JWT handlers until Task 6; all seven path registrations swap atomically with the router cutover.

- [ ] **Step 1: Write failing cookie, origin, DTO, and handler-response tests**

Add pure/oneshot tests with concrete headers and generated JSON shapes:

```rust
#[test]
fn session_cookie_has_host_prefix_and_required_attributes() {
    let header = set_session_cookie("opaque-value", None);
    let session = header.to_str().unwrap();
    assert!(session.starts_with("__Host-schoolorbit_session=opaque-value;"));
    for attribute in ["HttpOnly", "SameSite=Lax", "Secure", "Path=/"] {
        assert!(session.split("; ").any(|part| part == attribute));
    }
    assert!(!session.contains("Max-Age="));
    assert!(!session.contains("Domain="));
    let remembered = set_session_cookie("opaque-value", Some(2_592_000)).to_str().unwrap().to_string();
    assert!(remembered.contains("Max-Age=2592000"));
    assert!(!remembered.contains("Domain="));
    let near_absolute = set_session_cookie("opaque-value", Some(86_399)).to_str().unwrap().to_string();
    assert!(near_absolute.contains("Max-Age=86399"));
    assert!(!near_absolute.contains("Max-Age=2592000"));
}

#[test]
fn logout_expires_new_and_legacy_cookies() {
    let headers = expire_auth_cookies();
    assert_eq!(headers.len(), 2);
    assert!(headers.iter().any(|value| value.to_str().unwrap().starts_with("__Host-schoolorbit_session=")));
    assert!(headers.iter().any(|value| value.to_str().unwrap().starts_with("auth_token=")));
    assert!(headers.iter().all(|value| value.to_str().unwrap().contains("Max-Age=0")));
    let session_expiry = headers.iter().find(|value| {
        value.to_str().unwrap().starts_with("__Host-schoolorbit_session=")
    }).unwrap().to_str().unwrap();
    assert!(session_expiry.contains("Secure"));
    assert!(session_expiry.contains("Path=/"));
    assert!(!session_expiry.contains("Domain="));
}

#[test]
fn unsafe_origin_must_equal_the_resolved_tenant_origin() {
    let policy = TenantOriginPolicy::for_tests("schoolorbit.app", []);
    assert!(policy.validate("https://demo.schoolorbit.app", "demo").is_ok());
    assert!(policy.validate("https://other.schoolorbit.app", "demo").is_err());
    assert!(policy.validate("https://demo.schoolorbit.app.evil.test", "demo").is_err());
}

#[test]
fn current_user_json_has_no_default_pii() {
    let value = serde_json::to_value(CurrentUserResponse::fixture()).unwrap();
    for forbidden in ["nationalId", "email", "phone", "dateOfBirth", "address", "createdAt"] {
        assert!(value.get(forbidden).is_none(), "unexpected field {forbidden}");
    }
}
```

Add handler tests for generic `401`, `Retry-After` on `429`, `503` logout retaining the session cookie, successful logout expiring both cookies, selected-session ownership returning `404`, current-session deletion expiring the cookie, logout-all expiring only after commit, password-change failure emitting no credential headers, and password-change success emitting exactly one replacement cookie plus `X-CSRF-Token` without placing either value in JSON.

Add request-smuggling boundary cases: duplicate session-cookie names across one or multiple `Cookie` headers return `401`; duplicate `Origin`, `Referer`, `X-School-Subdomain`, or `X-CSRF-Token` values return `403`; malformed cookie pairs never panic; and a legacy cookie beside exactly one valid opaque cookie is ignored rather than treated as identity.

- [ ] **Step 2: Run the HTTP-focused tests and verify the red state**

Run:

```bash
cd backend-school
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_http_tests --bin backend-school -- --nocapture
cargo test modules::auth::http::tests --bin backend-school -- --nocapture
cargo test utils::subdomain::tests --bin backend-school -- --nocapture
cargo test api_contract::tests --bin backend-school
```

Expected: compilation fails because new DTOs, handlers, cookie helpers, and OpenAPI registrations do not exist.

- [ ] **Step 3: Implement runtime state, exact wire DTOs, and thin handlers**

Create a cloneable runtime that shares existing application resources:

```rust
#[derive(Clone)]
pub struct AuthRuntime {
    pub admin_client: Arc<AdminClient>,
    pub pool_manager: Arc<PoolManager>,
    pub permission_cache: Arc<PermissionCache>,
    pub config: Arc<SessionConfig>,
    pub session_events: broadcast::Sender<SessionRevocationEvent>,
}

impl axum::extract::FromRef<AppState> for AuthRuntime {
    fn from_ref(state: &AppState) -> Self { state.auth_runtime.clone() }
}
```

Add `auth_runtime: AuthRuntime` to `AppState`, construct `SessionConfig::from_env()` before binding the server, and create a broadcast channel with capacity 100. Do not add the new middleware or replace routes in this task.

Add these minimal contract types alongside the old `UserResponse` and old `LoginData` needed by the still-routed JWT handlers in this intermediate commit; retain `ProfileResponse` unchanged. Use the temporary Rust-only name `SessionLoginData` in the unwired session login handler. Task 6 deletes `UserResponse`/old `LoginData` with the old login/me implementation and renames `SessionLoginData` to the final wire schema `LoginData`, so legacy and final login schemas never share one OpenAPI path:

```rust
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUserResponse {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_role_name: Option<String>,
    #[schema(required = true)]
    pub profile_image_file_id: Option<Uuid>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionLoginData { pub user: CurrentUserResponse }

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub id: Uuid,
    pub device_label: String,
    pub remember_me: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
    pub is_current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionListData { pub sessions: Vec<SessionResponse> }
```

In `http.rs`, parse every `Cookie` header manually and append one `Set-Cookie` header per response cookie. Implement `set_session_cookie(encoded: &str, max_age_seconds: Option<u64>)`, `expire_auth_cookies`, `csrf_response_header`, `presented_session_token`, and constant-time `validate_csrf`. Require exactly one syntactically valid opaque session cookie when authentication is attempted; reject duplicate session-cookie names even when split across headers. Origin/Referer/tenant-hint/CSRF helpers likewise require at most one authoritative value and never silently choose the first/last duplicate. Do not use `tower-cookies` in new code.

Refactor subdomain parsing around a strict `url::Url` origin parser using configured `BASE_DOMAIN`; accept an allowlisted development origin only when a valid `X-School-Subdomain` is also present. For unsafe requests, prefer `Origin`; use the origin portion of `Referer` only when `Origin` is absent. A valid `Origin` has no credentials, query, or fragment and its URL path is exactly `/`; reject any non-root path, opaque/null value, or production port other than implicit 443. A fallback `Referer` may have a path, but compare only its normalized origin and never log the full value.

Add `parse_realtime_tenant_hint(raw_query: Option<&str>) -> Result<Option<String>, AppError>` using `url::form_urlencoded`: ignore unrelated query keys, accept at most one `school_subdomain`, percent-decode once, apply the same ASCII label validator, and reject an empty/duplicate/malformed value with fixed `403`. Task 6 uses it for SSE and Task 11 reuses it for WebSocket instead of writing a second parser.

Add `resolve_auth_tenant_context(runtime: &AuthRuntime, headers: &HeaderMap, dev_realtime_tenant_hint: Option<&str>) -> Result<TenantContext, AppError>` in `utils/tenant.rs`; it uses the runtime's existing admin client/pool manager and strict origin parser. Ordinary HTTP callers pass `None` and may use the existing validated `X-School-Subdomain` only for an allowlisted development Origin. SSE/WebSocket callers may instead pass a validated `school_subdomain` query hint because browser streaming APIs cannot set that header. Accept either development hint as authority only when the exact Origin is in `SCHOOL_ALLOWED_DEV_ORIGINS`; reject missing, duplicate, malformed, or conflicting development hints. For a production Origin, always derive the tenant from the hostname and, if any hint is present, require it to match without treating it as authority. New session handlers use this function, while existing public feature handlers retain `resolve_tenant_context(&AppState, ...)`.

Map an unavailable tenant directory or tenant pool to fixed `503` reason codes in this auth resolver, and never log a database URL, upstream error body, Origin, or tenant hint. A syntactically valid but unknown school remains `404`; login credential responses become generic only after a tenant is resolved.

Implement these handlers in `session_handlers.rs` and annotate every path. In `api_contract.rs`, register only the new component schemas in this task; leave every old auth path registration untouched and do not advertise the three new session paths until the Task 6 router cutover:

```rust
pub async fn login(State(AuthRuntime), ConnectInfo<SocketAddr>, HeaderMap, Result<Json<LoginRequest>, JsonRejection>) -> Result<Response, AppError>;
pub async fn logout(State(AuthRuntime), HeaderMap) -> Result<Response, AppError>;
pub async fn me(State(AuthRuntime), Extension<AuthenticatedSession>) -> Result<Response, AppError>;
pub async fn list_sessions(State(AuthRuntime), Extension<AuthenticatedSession>) -> Result<Response, AppError>;
pub async fn revoke_session(State(AuthRuntime), Extension<AuthenticatedSession>, Path<Uuid>) -> Result<Response, AppError>;
pub async fn logout_all(State(AuthRuntime), Extension<AuthenticatedSession>) -> Result<Response, AppError>;
pub async fn change_password(State(AuthRuntime), Extension<AuthenticatedSession>, Result<Json<ChangePasswordRequest>, JsonRejection>) -> Result<Response, AppError>;
```

`me` maps only `session_service::load_current_user` into `CurrentUserResponse`; it must not call `find_user_by_id`, `ProfileResponse`, or any national-ID decryption helper. `list_sessions` marks `is_current` only by comparing each owned session UUID with the extension's session UUID.

Map `JsonRejection` to the standard fixed error envelope (`413` for the route-local limit, otherwise `400`) without logging the body or deserializer input.

Login validates exact Origin, resolves tenant with `dev_realtime_tenant_hint = None`, selects the trusted source, calls the service, sets the new session cookie, expires `auth_token`, returns the temporary `SessionLoginData`, and exposes current CSRF. Logout validates exact Origin; absent/malformed/not-found session expires both cookies with `200`; a valid session authenticates in `TouchOnly` mode, requires CSRF, and commits revocation before expiry; a database error returns `503` with no expiry header.

For the protected handlers, mutation CSRF is owned by Task 6 middleware. Selected-session deletion expires both cookies only when the committed result includes the current session. Logout-all expires both only after its revocation commit. Password change receives `PasswordChangeResult`, then sets exactly its committed replacement cookie and CSRF header; a validation/auth/database failure sends neither. Other-device deletion and session listing never modify cookies.

- [ ] **Step 4: Run HTTP tests and OpenAPI validation**

Run:

```bash
cd backend-school
cargo fmt --all
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_http_tests --bin backend-school -- --nocapture
cargo test modules::auth::http::tests --bin backend-school -- --nocapture
cargo test utils::subdomain::tests --bin backend-school -- --nocapture
cargo test api_contract::tests --bin backend-school -- --nocapture
cargo check --bin backend-school
```

Expected: all tests pass; the binary compiles while `main.rs` still points browser traffic at the old handlers.

- [ ] **Step 5: Commit the unwired HTTP contract slice**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/src/api_contract.rs backend-school/src/main.rs backend-school/src/modules/auth.rs backend-school/src/modules/auth/http.rs backend-school/src/modules/auth/models.rs backend-school/src/modules/auth/runtime.rs backend-school/src/modules/auth/session_handlers.rs backend-school/src/modules/auth/session_http_tests.rs backend-school/src/utils/subdomain.rs backend-school/src/utils/tenant.rs
git commit -m "feat(auth): define session HTTP contract"
```

### Task 6: Cut the school router over once to the central session boundary

**Files:**
- Create: `backend-school/src/middleware/session.rs`
- Create: `backend-school/src/app.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/middleware.rs`
- Modify: `backend-school/src/middleware/permission.rs`
- Modify: `backend-school/src/utils/request_context.rs`
- Modify: `backend-school/src/utils/tenant.rs`
- Modify: `backend-school/src/modules/admission.rs`
- Modify: `backend-school/src/modules/auth/audit.rs`
- Modify: `backend-school/src/modules/auth/handlers.rs`
- Modify: `backend-school/src/modules/auth/models.rs`
- Modify: `backend-school/src/modules/auth/services.rs`
- Modify: `backend-school/src/modules/consent/handlers.rs`
- Modify: `backend-school/src/modules/lookup/handlers.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Delete: `backend-school/src/middleware/auth.rs`
- Delete: `backend-school/src/utils/jwt.rs`

**Interfaces:**
- Consumes: `AuthRuntime`, `AuthenticatedSession`, `SessionAuthentication`, new session handlers, public/staff admission route sets, and existing internal/deploy-key middleware.
- Produces: `build_app(AppState) -> Router`, one router-level `session_middleware`, `actor_tenant_context_from_session`, `current_user_tenant_context_from_session`, and temporary new-session-only header compatibility helpers for handler files migrated in Tasks 7–10.

- [ ] **Step 1: Write failing router-boundary and cutover guards**

Add static tests and authenticated HTTP tests with these invariants:

```rust
#[test]
fn browser_auth_uses_one_session_boundary_and_no_jwt_runtime() {
    let main = read_source(manifest_dir().join("src/main.rs"));
    let app = read_source(manifest_dir().join("src/app.rs"));
    assert!(!main.contains("auth_middleware"));
    assert_eq!(
        app.matches("from_fn_with_state(runtime, session_middleware)").count(),
        1
    );
    assert!(!manifest_dir().join("src/utils/jwt.rs").exists());
    assert!(!manifest_dir().join("src/middleware/auth.rs").exists());
    assert!(!app.contains("Authorization"));
    assert!(!read_source(manifest_dir().join("Cargo.toml")).contains("jsonwebtoken"));
}

#[test]
fn public_and_protected_routes_are_explicitly_partitioned() {
    let app = read_source(manifest_dir().join("src/app.rs"));
    assert!(app.contains("public_routes()"));
    assert!(app.contains("protected_routes()"));
    assert!(app.contains("admission_public_routes()"));
    assert!(app.contains("admission_staff_routes()"));
    assert!(app.contains("/internal/migrate-all"));
    assert!(app.contains("/api/admin/routes/sync"));
}
```

HTTP cases must prove: no cookie → `401`; a fake `auth_token` only → `401`; `Authorization: Bearer ...` only → `401`; valid opaque cookie → protected read succeeds; a tenant-A cookie presented from tenant B resolves only tenant B and returns `401`; inactive user → `401`; table-driven `POST`, `PUT`, `PATCH`, and `DELETE` requests with missing/wrong Origin → `403`; the same four methods with missing/wrong CSRF → `403`; current and previous credentials accept the same logical-session CSRF during grace; a different session's CSRF fails; a due SSE handshake does not rotate; and the next ordinary rotating response contains both `Set-Cookie` and `X-CSRF-Token`.

Add a body-limit case proving an auth JSON payload above 16 KiB returns `413` before bcrypt/service invocation while upload routes retain the existing 20 MiB application limit.

- [ ] **Step 2: Run the cutover tests and verify the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture browser_auth_uses_one_session_boundary_and_no_jwt_runtime -- --exact --nocapture
cargo test --test static_architecture public_and_protected_routes_are_explicitly_partitioned -- --exact --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_http_tests::protected_router --bin backend-school -- --nocapture
```

Expected: failures show route-local JWT middleware, mixed admission routing, and accepted legacy JWT code still present.

- [ ] **Step 3: Implement middleware, router partition, and temporary session-only compatibility**

Implement middleware with this flow and signature:

```rust
pub async fn session_middleware(
    State(runtime): State<AuthRuntime>,
    mut request: Request,
    next: Next,
) -> Response {
    // resolve tenant (strict Origin plus SSE-only dev query hint)
    // -> parse opaque cookie -> authenticate active user
    // streaming/current-credential mutations use TouchOnly;
    // ordinary HTTP uses RotateAndTouch
    // unsafe method: exact Origin then session-derived constant-time CSRF
    // insert AuthenticatedSession -> call next
    // on rotation: append replacement cookie and CSRF response header
    // on GET /api/auth/me: append the stable logical-session CSRF
}
```

Convert errors through `IntoResponse`; map session-store failures to `503`. Never insert raw token or CSRF into request extensions. Origin/CSRF rejection and rotation-store failure call only the typed audit helpers from Task 4; tests must prove no supplied header/cookie/origin value is formatted into a tracing field. Choose maintenance mode from a closed route/method match before authentication:

```rust
fn is_session_revoke_route(method: &Method, path: &str) -> bool {
    method == Method::DELETE
        && path
            .strip_prefix("/api/auth/sessions/")
            .and_then(|value| value.parse::<Uuid>().ok())
            .is_some()
}

let method = request.method();
let path = request.uri().path();
let must_defer_rotation = (method == Method::GET && path == "/api/notifications/stream")
    || (method == Method::POST && path == "/api/auth/logout-all")
    || (method == Method::POST && path == "/api/auth/me/change-password")
    || is_session_revoke_route(method, path);
let maintenance = if must_defer_rotation {
    SessionMaintenanceMode::TouchOnly
} else {
    SessionMaintenanceMode::RotateAndTouch
};
```

The DELETE prefix must additionally parse as the registered single-UUID route; do not let arbitrary paths opt out of rotation. Logout is public and already chooses `TouchOnly` in its handler. These current-credential mutation handlers own their expire/replacement cookie, so middleware must never append a competing rotation after they return. For the exact SSE route only, call the shared `parse_realtime_tenant_hint` on the raw query and pass its result to `resolve_auth_tenant_context`; the parser rejects duplicate or malformed `school_subdomain` values and the strict Origin policy decides whether the hint is permitted. No other protected route accepts a query tenant hint.

Add static/HTTP cases proving a malformed/duplicate hint is rejected, a permitted localhost hint resolves the expected tenant, a due SSE handshake does not rotate, the next ordinary `/api/auth/me` request does, current-session deletion/logout-all cannot be followed by a replacement cookie, and password change emits only its transaction-owned replacement. `/api/auth/me` and every successful rotation response expose the same stable logical-session CSRF regardless of whether authentication matched the current or previous credential. Add a two-tab/concurrent-rotation test proving response order cannot change that value. If ordinary-request rotation commits, add response credentials only after `next` returns. If the downstream response already contains `Set-Cookie`, append rather than replace it.

Move route assembly to `app.rs`:

```rust
pub fn build_app(state: AppState) -> Router {
    let runtime = state.auth_runtime.clone();
    public_routes()
        .merge(protected_routes().route_layer(from_fn_with_state(runtime, session_middleware)))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .with_state(state)
}
```

`public_routes()` owns root, health, readiness, public calendar, consent types, public file content/delivery, school public info, new login/logout, `admission_public_routes()`, deploy-key route sync, WebSocket upgrade, and internal-secret routes. `protected_routes()` owns all other browser endpoints, including SSE and `admission_staff_routes()`.

Apply a route-local `DefaultBodyLimit::max(16 * 1024)` to login and change-password JSON routes and prove it remains narrower than the existing 20 MiB upload-capable application limit. Do not reduce file/multipart limits globally.

Split `admission.rs` exactly as follows:

```rust
pub fn admission_public_routes() -> Router<AppState> {
    Router::new()
        .route("/apply/rounds", get(handlers::rounds::list_public_rounds))
        .route("/apply/round/{id}", get(handlers::rounds::get_public_round_info))
        .route("/apply/{round_id}", post(handlers::applications::submit_application))
        .route("/portal/check", post(handlers::portal::check_application))
        .route("/portal/status", post(handlers::portal::get_status))
        .route("/portal/confirm", post(handlers::portal::confirm_enrollment))
        .route("/portal/form", post(handlers::portal::get_enrollment_form).put(handlers::portal::submit_enrollment_form))
        .route("/portal/application", put(handlers::portal::update_application))
        .route("/portal/upload", post(handlers::portal::portal_upload_document))
        .route("/portal/documents/{doc_type}", delete(handlers::portal::portal_delete_document))
        .route("/portal/documents/{file_id}/download", post(handlers::portal::portal_download_document))
        .route("/portal/exam-seat", post(handlers::portal::get_exam_seat))
}
```

`admission_staff_routes()` contains every route formerly in `admission_routes()` except those twelve public routes; copy each existing route verbatim so no staff endpoint is lost.

Add typed helpers:

```rust
pub async fn actor_tenant_context_from_session(
    state: &AppState,
    session: &AuthenticatedSession,
) -> Result<ActorTenantContext, AppError>;

pub fn current_user_tenant_context_from_session(
    session: &AuthenticatedSession,
) -> CurrentUserTenantContext;
```

For Tasks 6–10 only, retain the existing header helper names but make them call the opaque session service in `TouchOnly` mode—never JWT—then delegate to the typed helpers. The central middleware exclusively owns any ordinary-request rotation; this also prevents the still-unmigrated SSE handler in the Task 6 intermediate commit from rotating invisibly. The compatibility path temporarily causes a second indexed session read for unmigrated handlers while allowing reviewable commits. Migrate the explicit `Claims` consumers in auth profile, consent, and lookup handlers now to `Extension<AuthenticatedSession>`.

Delete `Claims`, JWT generation/parsing, route-local auth middleware, `CookieManagerLayer`, `jsonwebtoken`, and `tower-cookies`. Serve with peer information:

```rust
axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await
```

In the same cutover diff, remove legacy `login`, `logout`, `me`, and `change_password` implementations from `modules/auth/handlers.rs` while retaining/migrating only `get_profile` and `update_profile`. Remove the now-unused active-login-by-ID and standalone password-update helpers from `services.rs`; retain only the optional login lookup/current-user/profile/primary-role functions owned by the new service flow. Delete legacy `UserResponse` and its PII conversion, delete the old `LoginData`, rename `SessionLoginData` to final `LoginData`, replace the four legacy OpenAPI path registrations with `session_handlers`, and add the three new session-management paths now that they are routed. Update component/API tests to require `CurrentUserResponse`/new `LoginData` and prove `UserResponse` is absent; `ProfileResponse` remains explicit and unchanged.

- [ ] **Step 4: Run cutover, HTTP, static, and compile tests**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test --test static_architecture browser_auth_uses_one_session_boundary_and_no_jwt_runtime -- --exact --nocapture
cargo test --test static_architecture public_and_protected_routes_are_explicitly_partitioned -- --exact --nocapture
cargo test --test static_architecture feature_modules_do_not_parse_jwt_directly -- --exact --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_http_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_service_tests --bin backend-school -- --nocapture
cargo test api_contract::tests --bin backend-school -- --nocapture
cargo check --bin backend-school
rg -n "Claims|JwtService|auth_middleware|authenticate_for_tenant|Authorization: Bearer" src Cargo.toml
```

Expected: all tests and compile pass; the final scan has no output. Valid opaque sessions work, and JWT/cookie bearer compatibility is absent.

- [ ] **Step 5: Commit the one-time browser auth cutover**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/Cargo.toml backend-school/Cargo.lock backend-school/src/api_contract.rs backend-school/src/app.rs backend-school/src/main.rs backend-school/src/middleware.rs backend-school/src/middleware/session.rs backend-school/src/middleware/permission.rs backend-school/src/utils/request_context.rs backend-school/src/utils/tenant.rs backend-school/src/modules/admission.rs backend-school/src/modules/auth/audit.rs backend-school/src/modules/auth/handlers.rs backend-school/src/modules/auth/models.rs backend-school/src/modules/auth/services.rs backend-school/src/modules/consent/handlers.rs backend-school/src/modules/lookup/handlers.rs backend-school/tests/static_architecture.rs
git add -u backend-school/src/middleware/auth.rs backend-school/src/utils/jwt.rs
git commit -m "feat(auth): cut school browser auth to sessions"
```

### Task 7: Migrate people and platform handlers to typed request identity

**Files:**
- Modify: `backend-school/src/modules/achievement/handlers.rs`
- Modify: `backend-school/src/modules/files/handlers.rs`
- Modify: `backend-school/src/modules/menu/handlers/admin.rs`
- Modify: `backend-school/src/modules/menu/handlers/public.rs`
- Modify: `backend-school/src/modules/notification/handlers.rs`
- Modify: `backend-school/src/modules/parents/handlers.rs`
- Modify: `backend-school/src/modules/school/handlers.rs`
- Modify: `backend-school/src/modules/staff/handlers/organization_delegations.rs`
- Modify: `backend-school/src/modules/staff/handlers/organization_members.rs`
- Modify: `backend-school/src/modules/staff/handlers/organization_permissions.rs`
- Modify: `backend-school/src/modules/staff/handlers/permissions.rs`
- Modify: `backend-school/src/modules/staff/handlers/roles.rs`
- Modify: `backend-school/src/modules/staff/handlers/staff.rs`
- Modify: `backend-school/src/modules/staff/handlers/user_roles.rs`
- Modify: `backend-school/src/modules/students/handlers.rs`
- Modify: `backend-school/src/modules/students/handlers_parents.rs`
- Modify: `backend-school/src/modules/system/handlers/feature_toggles.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: `Extension<AuthenticatedSession>`, `actor_tenant_context_from_session(&AppState, &AuthenticatedSession)`, and `current_user_tenant_context_from_session(&AuthenticatedSession)` from Task 6.
- Produces: all listed protected handlers use the already-validated request extension and perform no second session database lookup.

- [ ] **Step 1: Add a failing inventory guard for this handler group**

Add a reusable static assertion and this exact inventory:

```rust
fn assert_typed_session_handlers(paths: &[&str]) {
    for path in paths {
        let source = read_source(manifest_dir().join(path));
        assert!(
            source.contains("Extension(session): Extension<AuthenticatedSession>"),
            "{path} must extract the central authenticated session"
        );
        assert!(!source.contains("actor_tenant_context(&state, &headers)"));
        assert!(!source.contains("current_user_tenant_context_from_headers"));
    }
}

#[test]
fn people_and_platform_handlers_use_typed_session_identity() {
    assert_typed_session_handlers(&[
        "src/modules/achievement/handlers.rs",
        "src/modules/files/handlers.rs",
        "src/modules/menu/handlers/admin.rs",
        "src/modules/menu/handlers/public.rs",
        "src/modules/notification/handlers.rs",
        "src/modules/parents/handlers.rs",
        "src/modules/school/handlers.rs",
        "src/modules/staff/handlers/organization_delegations.rs",
        "src/modules/staff/handlers/organization_members.rs",
        "src/modules/staff/handlers/organization_permissions.rs",
        "src/modules/staff/handlers/permissions.rs",
        "src/modules/staff/handlers/roles.rs",
        "src/modules/staff/handlers/staff.rs",
        "src/modules/staff/handlers/user_roles.rs",
        "src/modules/students/handlers.rs",
        "src/modules/students/handlers_parents.rs",
        "src/modules/system/handlers/feature_toggles.rs",
    ]);
}
```

- [ ] **Step 2: Run the inventory guard and verify the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture people_and_platform_handlers_use_typed_session_identity -- --exact --nocapture
```

Expected: failure identifies the first file still using a header compatibility helper.

- [ ] **Step 3: Apply the typed extractor transformation to every protected handler**

For actor-authorized handlers, use this exact transformation while retaining `HeaderMap` only when a non-auth response/header operation still needs it:

```rust
use axum::extract::{Extension, State};
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::utils::request_context::actor_tenant_context_from_session;

pub async fn handler(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    // Keep the existing permission/policy/service/response body unchanged.
}
```

For own-user handlers, use:

```rust
let context = current_user_tenant_context_from_session(&session);
let pool = context.tenant.pool;
let user_id = context.user_id;
```

Do not move permission or resource-policy checks, do not change DTOs, and do not add session SQL to feature modules. In the notification file, migrate the SSE extractor now but leave its stream loop unchanged until Task 11.

Update existing static tests whose source assertions name the old header helpers for any file in this group; preserve the authorization invariant they were checking and assert the typed session helper instead. Do not weaken or delete an existing guard merely to satisfy the new inventory.

- [ ] **Step 4: Run the group guard, affected authorization tests, and compile**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test --test static_architecture people_and_platform_handlers_use_typed_session_identity -- --exact --nocapture
cargo test --test static_architecture auth_responses_use_shared_effective_permission_resolver -- --exact --nocapture
cargo test --test static_architecture menu_and_feature_handlers_do_not_parse_auth_or_query_permissions_directly -- --exact --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::staff::services::status_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::students::mutation_tests --bin backend-school -- --nocapture
cargo check --bin backend-school
```

Expected: all checks pass and behavior remains permission/policy-equivalent.

- [ ] **Step 5: Commit the people/platform identity migration**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/src/modules/achievement/handlers.rs backend-school/src/modules/files/handlers.rs backend-school/src/modules/menu/handlers/admin.rs backend-school/src/modules/menu/handlers/public.rs backend-school/src/modules/notification/handlers.rs backend-school/src/modules/parents/handlers.rs backend-school/src/modules/school/handlers.rs backend-school/src/modules/staff/handlers backend-school/src/modules/students/handlers.rs backend-school/src/modules/students/handlers_parents.rs backend-school/src/modules/system/handlers/feature_toggles.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(auth): inject session into platform handlers"
```

### Task 8: Migrate academic structure, activity, assessment, course-planning, and subject handlers

**Files:**
- Modify: `backend-school/src/modules/academic/handlers.rs`
- Modify: `backend-school/src/modules/academic/handlers/activity.rs`
- Modify: `backend-school/src/modules/academic/handlers/assessment.rs`
- Modify: `backend-school/src/modules/academic/handlers/course_planning.rs`
- Modify: `backend-school/src/modules/academic/handlers/subjects.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: the same typed identity functions defined in Task 6 and unchanged academic service/policy interfaces.
- Produces: these five academic handler files contain no compatibility session lookup and preserve exact permission checks.

- [ ] **Step 1: Add a failing academic group-A inventory test**

```rust
#[test]
fn academic_group_a_handlers_use_typed_session_identity() {
    assert_typed_session_handlers(&[
        "src/modules/academic/handlers.rs",
        "src/modules/academic/handlers/activity.rs",
        "src/modules/academic/handlers/assessment.rs",
        "src/modules/academic/handlers/course_planning.rs",
        "src/modules/academic/handlers/subjects.rs",
    ]);
}
```

- [ ] **Step 2: Run the guard and verify the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture academic_group_a_handlers_use_typed_session_identity -- --exact --nocapture
```

Expected: failure reports the existing `HeaderMap`-based actor context.

- [ ] **Step 3: Replace only auth context extraction in all five files**

Every protected signature receives:

```rust
Extension(session): Extension<AuthenticatedSession>,
```

Every actor context becomes:

```rust
let context = actor_tenant_context_from_session(&state, &session).await?;
```

Every own-user context becomes:

```rust
let context = current_user_tenant_context_from_session(&session);
```

Keep query/path/body extractor ordering valid for Axum, and leave permission constants, organization scopes, academic SQL services, response DTOs, and event emission byte-for-byte equivalent except for import formatting.

Update every existing static assertion for these five files that names a removed header helper so it checks the typed extractor/helper while preserving its original permission or layering invariant.

- [ ] **Step 4: Run focused academic tests, static guard, and compile**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test --test static_architecture academic_group_a_handlers_use_typed_session_identity -- --exact --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::services::academic_structure_service_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::services::activity_service_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::services::course_planning_service_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::services::subject_service_tests --bin backend-school -- --nocapture
cargo check --bin backend-school
```

Expected: all checks pass with no academic behavior change.

- [ ] **Step 5: Commit academic group A**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/src/modules/academic/handlers.rs backend-school/src/modules/academic/handlers/activity.rs backend-school/src/modules/academic/handlers/assessment.rs backend-school/src/modules/academic/handlers/course_planning.rs backend-school/src/modules/academic/handlers/subjects.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(auth): inject sessions into academic handlers"
```

### Task 9: Migrate academic study-plan, exam, and timetable handlers

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/exam_schedule.rs`
- Modify: `backend-school/src/modules/academic/handlers/study_plans.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable_templates.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: `AuthenticatedSession`, typed request-context helpers, and existing exam/study-plan/timetable services.
- Produces: the remaining ordinary academic HTTP handlers perform no second session lookup; WebSocket code remains for Task 11.

- [ ] **Step 1: Add a failing academic group-B inventory test**

```rust
#[test]
fn academic_group_b_handlers_use_typed_session_identity() {
    assert_typed_session_handlers(&[
        "src/modules/academic/handlers/exam_schedule.rs",
        "src/modules/academic/handlers/study_plans.rs",
        "src/modules/academic/handlers/timetable.rs",
        "src/modules/academic/handlers/timetable_templates.rs",
    ]);
}
```

- [ ] **Step 2: Run the guard and verify the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture academic_group_b_handlers_use_typed_session_identity -- --exact --nocapture
```

Expected: failure reports compatibility context calls in these files.

- [ ] **Step 3: Apply the typed identity transformation without altering timetable behavior**

Use exactly:

```rust
use axum::extract::Extension;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::utils::request_context::{
    actor_tenant_context_from_session,
    current_user_tenant_context_from_session,
};
```

Actor handlers call `actor_tenant_context_from_session(&state, &session).await?`; `/api/me/*` handlers call `current_user_tenant_context_from_session(&session)`. Preserve all semester access, self-service user-type decisions, conflict status codes, timetable event publication, and exact permission checks.

Rewrite affected pre-existing static source assertions to the typed helper names without relaxing their original behavior checks.

- [ ] **Step 4: Run focused academic tests and compile**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test --test static_architecture academic_group_b_handlers_use_typed_session_identity -- --exact --nocapture
cargo test --test static_architecture academic_exam_schedule_routes_are_registered_and_authorized -- --exact --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::services::study_plan_service_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::services::exam_schedule_service_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::services::timetable_service_tests --bin backend-school -- --nocapture
cargo check --bin backend-school
```

Expected: all checks pass; service behavior and realtime mutation payloads are unchanged.

- [ ] **Step 5: Commit academic group B**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/src/modules/academic/handlers/exam_schedule.rs backend-school/src/modules/academic/handlers/study_plans.rs backend-school/src/modules/academic/handlers/timetable.rs backend-school/src/modules/academic/handlers/timetable_templates.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(auth): inject sessions into timetable handlers"
```

### Task 10: Finish typed identity migration and remove the compatibility lookup

**Files:**
- Modify: `backend-school/src/modules/admission/handlers/applications.rs`
- Modify: `backend-school/src/modules/admission/handlers/exam_rooms.rs`
- Modify: `backend-school/src/modules/admission/handlers/rounds.rs`
- Modify: `backend-school/src/modules/admission/handlers/scores.rs`
- Modify: `backend-school/src/modules/admission/handlers/selections.rs`
- Modify: `backend-school/src/modules/calendar/handlers.rs`
- Modify: `backend-school/src/modules/facility/handlers.rs`
- Modify: `backend-school/src/modules/question_bank/handlers.rs`
- Modify: `backend-school/src/modules/supervision/handlers.rs`
- Modify: `backend-school/src/modules/work/handlers.rs`
- Modify: `backend-school/src/modules/workflow/handlers.rs`
- Modify: `backend-school/src/utils/request_context.rs`
- Modify: `backend-school/src/middleware/permission.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: central middleware identity and existing public tenant helpers.
- Produces: all protected ordinary HTTP handlers use typed request identity; `request_context.rs` contains no header-auth compatibility function; one indexed session validation occurs per ordinary protected request.

- [ ] **Step 1: Add failing final inventory and credential-parsing boundary tests**

```rust
#[test]
fn remaining_vertical_handlers_use_typed_session_identity() {
    assert_typed_session_handlers(&[
        "src/modules/admission/handlers/applications.rs",
        "src/modules/admission/handlers/exam_rooms.rs",
        "src/modules/admission/handlers/rounds.rs",
        "src/modules/admission/handlers/scores.rs",
        "src/modules/admission/handlers/selections.rs",
        "src/modules/calendar/handlers.rs",
        "src/modules/facility/handlers.rs",
        "src/modules/question_bank/handlers.rs",
        "src/modules/supervision/handlers.rs",
        "src/modules/work/handlers.rs",
        "src/modules/workflow/handlers.rs",
    ]);
}

#[test]
fn only_auth_boundary_parses_browser_session_credentials() {
    let allowed = [
        "src/middleware/session.rs",
        "src/modules/auth/http.rs",
        "src/modules/auth/session_handlers.rs",
        "src/modules/academic/websockets.rs",
    ];
    for file in rust_files_under(&manifest_dir().join("src")) {
        let relative = repo_relative(&file);
        let source = read_source(&file);
        if !allowed.contains(&relative.as_str()) {
            assert!(!source.contains("presented_session_token("), "credential parse in {relative}");
            assert!(!source.contains("SESSION_COOKIE_NAME"), "cookie access in {relative}");
        }
    }
    let context = read_source(manifest_dir().join("src/utils/request_context.rs"));
    assert!(!context.contains("_from_headers"));
    assert!(!context.contains("HeaderMap"));
}
```

- [ ] **Step 2: Run final inventory tests and verify the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture remaining_vertical_handlers_use_typed_session_identity -- --exact --nocapture
cargo test --test static_architecture only_auth_boundary_parses_browser_session_credentials -- --exact --nocapture
```

Expected: failures identify the remaining compatibility calls and header-auth functions.

- [ ] **Step 3: Migrate protected functions and delete compatibility APIs**

For mixed public/protected admission and calendar files, change only protected functions. Public functions retain `tenant_context`, `tenant_pool`, or `tenant_context_by_subdomain` and do not receive `AuthenticatedSession`.

Protected functions use:

```rust
Extension(session): Extension<AuthenticatedSession>,
let context = actor_tenant_context_from_session(&state, &session).await?;
```

or, for own-user behavior:

```rust
Extension(session): Extension<AuthenticatedSession>,
let context = current_user_tenant_context_from_session(&session);
```

After the inventory has no caller, delete `actor_tenant_context(&AppState, &HeaderMap)`, `current_user_tenant_context_from_headers`, and every permission helper that authenticates headers. Keep `get_cached_user_permissions`, `ActorContext`, permission matching, and `load_actor_context_for_session(user_id, tenant, pool, cache)`.

Search the whole static suite for the deleted helper names and update each affected assertion to the final typed boundary. An obsolete-string assertion may be replaced, but its route, permission, or credential-parsing protection must remain represented.

Use these final request-context signatures:

```rust
pub async fn actor_tenant_context_from_session(
    state: &AppState,
    session: &AuthenticatedSession,
) -> Result<ActorTenantContext, AppError>;

pub fn current_user_tenant_context_from_session(
    session: &AuthenticatedSession,
) -> CurrentUserTenantContext;
```

- [ ] **Step 4: Prove no compatibility lookup remains**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test --test static_architecture remaining_vertical_handlers_use_typed_session_identity -- --exact --nocapture
cargo test --test static_architecture only_auth_boundary_parses_browser_session_credentials -- --exact --nocapture
cargo check --bin backend-school
rg -n "actor_tenant_context\(&state, &headers\)|current_user_tenant_context_from_headers|load_actor_context\(.*headers|authenticate.*headers" src --glob '*.rs'
```

Expected: tests and compile pass; the final scan has no output. Public tenant resolution remains, but no feature handler reparses a browser credential.

- [ ] **Step 5: Commit final typed request identity**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/src/modules/admission/handlers/applications.rs backend-school/src/modules/admission/handlers/exam_rooms.rs backend-school/src/modules/admission/handlers/rounds.rs backend-school/src/modules/admission/handlers/scores.rs backend-school/src/modules/admission/handlers/selections.rs backend-school/src/modules/calendar/handlers.rs backend-school/src/modules/facility/handlers.rs backend-school/src/modules/question_bank/handlers.rs backend-school/src/modules/supervision/handlers.rs backend-school/src/modules/work/handlers.rs backend-school/src/modules/workflow/handlers.rs backend-school/src/utils/request_context.rs backend-school/src/middleware/permission.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(auth): finish typed session identity"
```

### Task 11: Bind SSE and WebSocket lifetime to authoritative sessions

**Files:**
- Modify: `backend-school/src/modules/notification/handlers.rs`
- Modify: `backend-school/src/modules/academic/websockets.rs`
- Modify: `backend-school/src/modules/auth/events.rs`
- Modify: `backend-school/src/modules/auth/session_service.rs`
- Modify: `backend-school/src/utils/subdomain.rs`
- Modify: `backend-school/src/utils/tenant.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: `AuthenticatedSession`, `SessionRevocationEvent`, `session_events`, `revalidate(&AuthenticatedSession, DateTime<Utc>) -> Result<bool, AppError>`, permission events, and the existing 30-second WebSocket heartbeat.
- Produces: local revocation closes matching realtime connections immediately; database revalidation closes expired/revoked/inactive sessions within thirty seconds; SSE emits only fixed `session_invalid`/`session_unavailable` control events before ending; close/event payloads contain no sensitive values.

- [ ] **Step 1: Write failing realtime lifecycle tests**

Extend unit/static tests with these decisions:

```rust
#[test]
fn revocation_event_targets_current_selected_and_user_sessions() {
    let tenant = "demo";
    let user = Uuid::new_v4();
    let current = Uuid::new_v4();
    let other = Uuid::new_v4();
    assert!(SessionRevocationEvent::session(tenant, user, current).applies_to(tenant, user, current));
    assert!(!SessionRevocationEvent::session(tenant, user, current).applies_to(tenant, user, other));
    assert!(SessionRevocationEvent::user(tenant, user, None).applies_to(tenant, user, other));
    assert!(!SessionRevocationEvent::user(tenant, user, Some(current)).applies_to(tenant, user, current));
}

#[tokio::test]
async fn websocket_session_revocation_wins_before_room_initialization() {
    let (sender, mut receiver) = session_channel(8);
    let session = authenticated_session("demo");
    sender.send(SessionRevocationEvent::session("demo", session.user_id, session.session_id)).unwrap();
    assert_eq!(
        queued_session_decision(&mut receiver, &session),
        SocketSessionDecision::Disconnect
    );
}
```

Add SSE stream tests for a matching local signal yielding exactly one empty `session_invalid` event before ending, nonmatching signals being ignored, a false DB revalidation doing the same, and a DB error yielding exactly one empty `session_unavailable` event without error details. Update the WebSocket static order test to require session-event and permission-event subscriptions before authentication, immediate revalidation before room join, and DB revalidation before heartbeat ping.

- [ ] **Step 2: Run realtime tests and verify the red state**

Run:

```bash
cd backend-school
cargo test modules::auth::events::tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::websockets::security_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::notification::handlers::tests --bin backend-school -- --nocapture
cargo test --test static_architecture timetable_websocket_handler_orders_session_auth_before_room_state -- --exact --nocapture
```

Expected: missing session receivers/revalidation decisions cause compile or assertion failures.

- [ ] **Step 3: Add subscribe-before-check, local events, and thirty-second DB checks**

For SSE, subscribe to all channels before the immediate authoritative check:

```rust
let mut notification_rx = state.notification_channel.subscribe();
let mut permission_rx = state.permission_event_channel.subscribe();
let mut work_rx = state.work_event_channel.subscribe();
let mut session_rx = state.auth_runtime.session_events.subscribe();
if !session_service::revalidate(&session, Utc::now()).await? {
    return Err(AppError::AuthError("Authentication required".to_string()));
}
```

Inside the stream, use `tokio::select!` over the existing receivers, matching session revocation, and a thirty-second interval whose first tick is scheduled thirty seconds after the already-completed immediate check; configure `MissedTickBehavior::Delay` so a stalled replica never bursts revalidation queries. On matching revocation or invalid/expired/inactive revalidation, yield one `Event::default().event("session_invalid").data("{}")`, then end. On database error or a closed auth channel, yield only `session_unavailable` with `{}`, then end. Ignore lagged events because the next DB tick remains authoritative. Keep `KeepAlive` and `X-Accel-Buffering: no` behavior; never put a reason, UUID, status, or error string in SSE data.

For WebSocket handshake, extend `WsParams` with `school_subdomain: Option<String>`, call the shared `parse_realtime_tenant_hint` on the raw query (never a second parser), reject duplicate query keys, and subscribe to session and permission channels before tenant/session authentication. Resolve the tenant from exact Origin through `resolve_auth_tenant_context(..., parsed_dev_hint.as_deref())`; the query hint is authoritative only for an allowlisted development Origin and production still derives the tenant from its Origin hostname. Parse the opaque cookie, call the same `authenticate` service with `SessionMaintenanceMode::TouchOnly`, authorize timetable access using its user ID, then immediately call `revalidate` and drain queued session/permission signals before room initialization. Assert missing/malformed/conflicting localhost hints fail before the room is read, a rotation-due WebSocket handshake leaves both token hashes unchanged, and the next ordinary HTTP request owns observable credential rotation while the logical-session CSRF remains stable.

Pass the full `AuthenticatedSession` and both receivers into `handle_socket`. Add a biased session-revocation branch before permission/incoming/broadcast branches. Start its delayed-missed-tick heartbeat thirty seconds after the immediate pre-room revalidation, not with Tokio's immediate first tick. On each heartbeat:

```rust
if !session_service::revalidate(&authenticated, Utc::now()).await.unwrap_or(false) {
    if socket.send(Message::Close(Some(CloseFrame {
        code: 1008,
        reason: "Authentication required".into(),
    }))).await.is_err() {
        tracing::debug!(reason = "session_close_send_failed");
    }
    break;
}
if socket.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
```

Use close code `1008` and only `Authentication required` or `Permission changed`; emit a redacted `session_realtime_disconnect` reason code with tenant ID/user ID/session UUID but no token, Origin, address, or User-Agent.

- [ ] **Step 4: Run realtime, auth, static, and compile tests**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test modules::auth::events::tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::academic::websockets::security_tests --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::notification::handlers::tests --bin backend-school -- --nocapture
cargo test --test static_architecture timetable_websocket_handler_orders_session_auth_before_room_state -- --exact --nocapture
cargo test --test static_architecture timetable_websocket_authorization_authenticates_active_user_before_permissions -- --exact --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth::session_service_tests --bin backend-school -- --nocapture
cargo check --bin backend-school
```

Expected: all checks pass; local revocation is immediate and cross-replica validity is bounded by thirty seconds through the database.

- [ ] **Step 5: Commit realtime session enforcement**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/src/main.rs backend-school/src/modules/auth/events.rs backend-school/src/modules/auth/session_service.rs backend-school/src/modules/notification/handlers.rs backend-school/src/modules/academic/websockets.rs backend-school/src/utils/subdomain.rs backend-school/src/utils/tenant.rs backend-school/tests/static_architecture.rs
git commit -m "feat(auth): enforce sessions on realtime connections"
```

### Task 12: Regenerate auth contracts and make the frontend transport session-aware

**Files:**
- Create: `frontend-school/src/lib/api/session-security.ts`
- Create: `frontend-school/tests/static/session-auth-contract.test.mjs`
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/src/lib/api/client.ts`
- Modify: `frontend-school/src/lib/api/auth.ts`
- Modify: `frontend-school/src/lib/stores/auth.ts`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`

**Interfaces:**
- Consumes: backend OpenAPI schemas introduced in Task 5 and final path registrations from Task 6, `X-CSRF-Token`, standard JSON envelope, and HTTP status/`Retry-After` headers.
- Produces: `ApiResponse<T>` transport metadata, `ApiClientError`, module-memory CSRF helpers, generated session DTO aliases, and auth/session API methods. A temporary boolean `checkAuth()` adapter remains until Task 13 so this task compiles independently.

- [ ] **Step 1: Write failing transport and generated-contract tests**

Create executable pure tests for memory-only CSRF behavior:

```js
import assert from 'node:assert/strict';
import test from 'node:test';

test('captures CSRF in module memory and injects only backend unsafe methods', async () => {
	const security = await import('../../src/lib/api/session-security.ts');
	security.clearSessionSecurity();
	security.captureSessionSecurityHeaders(
		new Headers({ 'X-CSRF-Token': 'csrf-one' })
	);
	security.captureSessionSecurityHeaders(new Headers());
	const callerHeaders = new Headers({ 'X-CSRF-Token': 'caller-controlled' });
	const postHeaders = security.withSessionSecurityHeaders('POST', callerHeaders);
	const getHeaders = security.withSessionSecurityHeaders('GET', callerHeaders);
	assert.equal(postHeaders.get('X-CSRF-Token'), 'csrf-one');
	assert.equal(getHeaders.has('X-CSRF-Token'), false);
	security.clearSessionSecurity();
	assert.equal(
		security.withSessionSecurityHeaders('DELETE', new Headers()).has('X-CSRF-Token'),
		false
	);
});

test('parses delta-seconds Retry-After and rejects dates or invalid values', async () => {
	const { retryAfterSeconds } = await import('../../src/lib/api/session-security.ts');
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': '30' })), 30);
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': '31' })), undefined);
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': '0' })), undefined);
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': '-1' })), undefined);
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': 'Wed, 21 Oct 2030 07:28:00 GMT' })), undefined);
});
```

Add static assertions that `session-security.ts` contains no `localStorage`, `sessionStorage`, cookie write, or exported raw token; every backend fetch path calls the shared header capture; generated schemas include `CurrentUserResponse`, `SessionResponse`, and `SessionListData`; and auth routes include list/revoke/logout-all.

- [ ] **Step 2: Run contract tests and verify the red state**

Run from `frontend-school`:

```bash
node --test tests/static/session-auth-contract.test.mjs tests/static/api-response-contract.test.mjs
npm run check:api-contracts
```

Expected: missing module/schema/path failures and stale generated-contract check failure.

- [ ] **Step 3: Generate artifacts, implement one transport pipeline, and consume generated DTOs**

Generate; do not edit either generated artifact directly:

```bash
cd frontend-school
npm run generate:api-contracts
```

Implement this module-memory API:

```ts
const CSRF_HEADER = 'X-CSRF-Token';
const unsafeMethods = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);
let csrfToken: string | null = null;

export function captureSessionSecurityHeaders(headers: Headers): void {
	const value = headers.get(CSRF_HEADER)?.trim();
	if (value) csrfToken = value;
}

export function withSessionSecurityHeaders(method: string, headers: Headers): Headers {
	const result = new Headers(headers);
	result.delete(CSRF_HEADER);
	if (csrfToken && unsafeMethods.has(method.toUpperCase())) result.set(CSRF_HEADER, csrfToken);
	return result;
}

export function clearSessionSecurity(): void { csrfToken = null; }

export function retryAfterSeconds(headers: Headers): number | undefined {
	const value = headers.get('Retry-After');
	if (!value || !/^\d+$/.test(value)) return undefined;
	const parsed = Number(value);
	return Number.isSafeInteger(parsed) && parsed >= 1 && parsed <= 30 ? parsed : undefined;
}
```

Separate wire envelope parsing from transport metadata:

```ts
export interface ApiResponse<T> {
	success: boolean;
	data?: T;
	error?: string;
	message?: string;
	status: number;
	retryAfterSeconds?: number;
}

export class ApiClientError extends Error {
	constructor(
		message: string,
		readonly status: number,
		readonly retryAfterSeconds?: number
	) { super(message); }
}
```

Create one private `fetchBackend(endpoint, options)` in `APIClient`: delete any caller-provided `X-School-Subdomain`/`X-CSRF-Token`, apply the sanitized configured tenant and memory-only CSRF headers, use `credentials: 'include'`, call `fetch`, then immediately call `captureSessionSecurityHeaders(response.headers)`. Route JSON, blob, post-blob, body-blob, and multipart methods through it. Keep `getExternalBlob` isolated with `credentials: 'omit'` and no session header capture/injection. Add a static guard that feature modules contain neither security header literal.

Every returned backend result includes `status` and parsed retry seconds. On confirmed `401`, call `clearSessionSecurity()`, `authStore.clearUser()`, preserve only a same-origin path in `redirectAfterLogin`, and redirect outside `/login`. Do not clear or redirect on `403`, `429`, `503`, or network exceptions. `requireApiData` throws `ApiClientError` using transport metadata.

Update generated aliases and API behavior:

```ts
export type CurrentUserDto = Schemas['CurrentUserResponse'];
export type SessionDto = Schemas['SessionResponse'];
type SessionListData = Schemas['SessionListData'];

async listSessions(): Promise<SessionDto[]>;
async revokeSession(
	sessionId: string,
	options: { current?: boolean } = {}
): Promise<void>;
async logoutAll(): Promise<void>;
```

Map only minimal current-user fields and never synthesize a missing PII/timestamp value. In this intermediate commit, make the legacy `User.createdAt` property optional so the minimized DTO compiles; leave removal of all obsolete optional PII properties and permission duplication to Task 14 after its UI call-site scan. Login handles `429` through `ApiClientError` and shows only a generic retry message with the validated delta-seconds value when present, never an account-specific lockout message. Logout clears user/permissions/CSRF only after a successful server response; a `503` or network failure throws and preserves auth state. `revokeSession` clears the same local state only after a confirmed successful response when `options.current === true`; selected-device failure/success leaves current identity intact. `logoutAll` clears only after confirmed success. Keep the existing temporary boolean `refreshCurrentUser()` plus `checkAuth(): Promise<boolean>` adapter compiling for this transport-focused commit; Task 13 replaces both call-site semantics with explicit refresh outcomes.

Update `api-response-contract.test.mjs` to expect `CurrentUserResponse`, no PII fields, and these exact auth paths: login, logout, me, profile GET/PUT, change-password, sessions GET, sessions/{id} DELETE, logout-all POST.

- [ ] **Step 4: Run focused tests, generated checks, static tests, and type check**

Run:

```bash
cd frontend-school
node --test tests/static/session-auth-contract.test.mjs tests/static/api-response-contract.test.mjs
npm run check:api-contracts
npm run test:api-contracts
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all checks pass; mutation requests share one memory-only CSRF pipeline and generated DTOs own all session responses.

- [ ] **Step 5: Commit frontend transport and generated contracts**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts frontend-school/src/lib/api/session-security.ts frontend-school/src/lib/api/client.ts frontend-school/src/lib/api/auth.ts frontend-school/src/lib/stores/auth.ts frontend-school/tests/static/session-auth-contract.test.mjs frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "feat(auth): make frontend transport session aware"
```

### Task 13: Preserve auth state across availability failures and remove boolean bootstrap semantics

**Files:**
- Create: `frontend-school/src/lib/auth/auth-refresh-policy.ts`
- Create: `frontend-school/tests/static/auth-session-state.test.mjs`
- Modify: `frontend-school/src/lib/api/auth.ts`
- Modify: `frontend-school/src/lib/stores/auth.ts`
- Modify: `frontend-school/src/routes/(app)/+layout.svelte`
- Modify: `frontend-school/src/routes/login/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/debug/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/profile/+page.svelte`

**Interfaces:**
- Consumes: `ApiResponse.status`, `ApiClientError`, confirmed-`401` clearing, current auth-store state, and existing login redirect validation.
- Produces: `AuthRefreshResult = 'authenticated' | 'unauthenticated' | 'unavailable'`, a retryable app-layout availability state, and no `checkAuth(): Promise<boolean>` call site.

- [ ] **Step 1: Write failing state-policy and static call-site tests**

```js
test('auth refresh policy preserves users only for unavailable transport', async () => {
	const { authRefreshDecision } = await import('../../src/lib/auth/auth-refresh-policy.ts');
	assert.deepEqual(authRefreshDecision(200), { result: 'authenticated', clear: false });
	assert.deepEqual(authRefreshDecision(401), { result: 'unauthenticated', clear: true });
	assert.deepEqual(authRefreshDecision(403), { result: 'unavailable', clear: false });
	assert.deepEqual(authRefreshDecision(503), { result: 'unavailable', clear: false });
});

test('all auth bootstrap call sites branch on explicit refresh states', async () => {
	for (const file of [appLayout, loginPage, debugPage, staffProfile]) {
		const source = await readFile(file, 'utf8');
		assert.doesNotMatch(source, /\bcheckAuth\s*\(/);
	}
	assert.match(appLayoutSource, /authStatus\s*=\s*'unavailable'/);
	assert.match(appLayoutSource, /retryAuthentication/);
});
```

Add a store assertion that `clearUser()` clears permissions while `setUnavailable()` preserves `user`/`isAuthenticated` and sets `isLoading: false` plus `isUnavailable: true`.

- [ ] **Step 2: Run state tests and verify the red state**

Run:

```bash
cd frontend-school
node --test tests/static/auth-session-state.test.mjs
```

Expected: missing policy and boolean `checkAuth()` references fail.

- [ ] **Step 3: Implement explicit refresh outcomes and retryable UI behavior**

Create the pure decision function:

```ts
export type AuthRefreshResult = 'authenticated' | 'unauthenticated' | 'unavailable';

export function authRefreshDecision(status: number): {
	result: AuthRefreshResult;
	clear: boolean;
} {
	if (status >= 200 && status < 300) return { result: 'authenticated', clear: false };
	if (status === 401) return { result: 'unauthenticated', clear: true };
	return { result: 'unavailable', clear: false };
}
```

Extend `AuthState` with `isUnavailable: boolean`; `setUser` and `clearUser` reset it, while `setUnavailable()` preserves all identity fields. Implement:

```ts
async refreshCurrentUser(options: { silent?: boolean } = {}): Promise<AuthRefreshResult> {
	const response = await apiClient.get<CurrentUserDto>('/api/auth/me');
	const decision = authRefreshDecision(response.status);
	if (decision.result === 'authenticated') {
		if (!response.data) {
			authStore.setUnavailable();
			return 'unavailable';
		}
		authStore.setUser(normalizeCurrentUser(response.data));
		return 'authenticated';
	}
	if (decision.clear) {
		authStore.clearUser();
	} else {
		authStore.setUnavailable();
	}
	return decision.result;
}
```

Set/reset loading in `try/finally` according to `silent`. Catch fetch/network exceptions and return `unavailable` without clearing. Treat a malformed successful envelope or missing `data` as `unavailable`, never `authenticated`, and preserve any prior identity. The stable logical-session CSRF header is captured by the shared transport on every successful `/me`, including requests authenticated through previous-token grace. Delete the boolean adapter.

In `(app)/+layout.svelte`, use `AuthStatus = 'checking' | 'authenticated' | 'unavailable' | 'redirecting'`. Initial `unauthenticated` stores the safe redirect and goes to login; `unavailable` renders `PageState` with title `ระบบยืนยันตัวตนไม่พร้อมใช้งาน`, a retry button, and no redirect. `retryAuthentication()` reruns refresh and branches identically. If an already-authenticated in-memory user encounters transient unavailability, leave the app shell visible and show a retry toast instead of unmounting it.

Login redirects only for `authenticated`, renders normally for `unauthenticated`, and shows a retry toast for `unavailable`. Debug/profile refresh calls accept the explicit result and never clear locally. Retain safe `redirectAfterLogin` validation exactly.

Because this task edits Svelte, use the loaded Svelte skills: run the autofixer on every changed `.svelte` file and apply every issue-level correction.

- [ ] **Step 4: Run policy tests, Svelte analysis, and frontend checks**

Run:

```bash
cd frontend-school
node --test tests/static/auth-session-state.test.mjs
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/+layout.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/login/+page.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/debug/+page.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/profile/+page.svelte'
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: tests and Svelte/type analysis pass with no issue-level autofixer output.

- [ ] **Step 5: Commit explicit auth state handling**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add frontend-school/src/lib/auth/auth-refresh-policy.ts frontend-school/src/lib/api/auth.ts frontend-school/src/lib/stores/auth.ts 'frontend-school/src/routes/(app)/+layout.svelte' frontend-school/src/routes/login/+page.svelte 'frontend-school/src/routes/(app)/debug/+page.svelte' 'frontend-school/src/routes/(app)/staff/profile/+page.svelte' frontend-school/tests/static/auth-session-state.test.mjs
git commit -m "feat(auth): preserve state during auth outages"
```

### Task 14: Add the shared account-security route and remove duplicated password forms

**Files:**
- Create: `frontend-school/src/lib/features/session-security/session-state.ts`
- Create: `frontend-school/src/lib/features/session-security/SessionSecurityPanel.svelte`
- Create: `frontend-school/src/routes/(app)/account/security/+page.svelte`
- Create: `frontend-school/src/routes/(app)/account/security/+page.ts`
- Create: `frontend-school/tests/static/account-security.test.mjs`
- Modify: `frontend-school/src/lib/auth/route-access.ts`
- Modify: `frontend-school/src/lib/components/layout/ProfileMenu.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/profile/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/settings/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/student/settings/+page.svelte`
- Modify: `frontend-school/src/lib/api/auth.ts`
- Modify: `frontend-school/src/lib/stores/auth.ts`
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`

**Interfaces:**
- Consumes: `authAPI.listSessions`, `revokeSession`, `logoutAll`, `changePassword`, generated `SessionDto`, auth store, PageShell/PageSkeleton/PageState/LoadingButton, and existing shadcn card/badge/alert-dialog primitives.
- Produces: an authenticated guard-only `/account/security` page shared by staff/student/parent, action-specific session mutation state, profile/settings links, and a minimal `User` view model with no default PII fields.

- [ ] **Step 1: Write failing route, state, and UI contract tests**

```js
test('account security is guard-only and available to every authenticated user type', async () => {
	assert.match(pageMeta, /access:\s*\{\s*authenticated:\s*true\s*\}/s);
	assert.doesNotMatch(pageMeta, /\bmenu\s*:/);
	assert.match(routeAccess, /authenticated\?:\s*boolean/);
	assert.match(routeAccess, /access\.authenticated/);
});

test('session state patches only the revoked row and preserves current identity', async () => {
	const { passwordValidation, removeRevokedSession } = await import('../../src/lib/features/session-security/session-state.ts');
	const sessions = [
		{ id: 'current', isCurrent: true },
		{ id: 'other', isCurrent: false },
	];
	assert.deepEqual(removeRevokedSession(sessions, 'other'), [{ id: 'current', isCurrent: true }]);
	assert.equal(passwordValidation('current-pass', 'ก'.repeat(23), 'ก'.repeat(23)), null);
	assert.match(passwordValidation('current-pass', 'ก'.repeat(24), 'ก'.repeat(24)) ?? '', /ยาวเกิน/);
});

test('default auth/menu UI contains no minimized PII field', async () => {
	assert.doesNotMatch(authStore, /nationalId|email\?:|phone\?:|createdAt/);
	assert.doesNotMatch(authStore, /permissions\?:/);
	assert.doesNotMatch(authApi, /authStore\.user\.permissions/);
	assert.doesNotMatch(profileMenu, /user\.email/);
	assert.doesNotMatch(staffProfile, /user\?\.(nationalId|createdAt)/);
});
```

Assert the panel includes loading, error, empty, current-device badge, selected revoke, current logout, logout-all, password mismatch/minimum-length, and action-specific loading controls; settings files must link to `/account/security` and contain no password input.

- [ ] **Step 2: Run account-security tests and verify the red state**

Run:

```bash
cd frontend-school
node --test tests/static/account-security.test.mjs tests/static/api-global-contract.test.mjs
```

Expected: missing route/component/state helper and duplicated password forms fail.

- [ ] **Step 3: Build the shared feature with Svelte 5 runes and guard-only metadata**

Implement pure state helpers:

```ts
import type { SessionDto } from '$lib/api/auth';

export function removeRevokedSession(sessions: SessionDto[], id: string): SessionDto[] {
	return sessions.filter((session) => session.id !== id);
}

export function keepCurrentSession(sessions: SessionDto[]): SessionDto[] {
	return sessions.filter((session) => session.isCurrent);
}

export function passwordValidation(
	currentPassword: string,
	newPassword: string,
	confirmPassword: string
): string | null {
	if (!currentPassword || !newPassword || !confirmPassword) return 'กรุณากรอกข้อมูลให้ครบถ้วน';
	if (newPassword !== confirmPassword) return 'รหัสผ่านใหม่ไม่ตรงกัน';
	if ([...newPassword].length < 8 || [...newPassword].length > 128) return 'รหัสผ่านต้องมี 8–128 ตัวอักษร';
	if (new TextEncoder().encode(newPassword).length > 71) return 'รหัสผ่านยาวเกินขีดจำกัดที่ปลอดภัย';
	return null;
}
```

The panel uses `$state.raw<SessionDto[]>([])`, `onMount(loadSessions)`, keyed `{#each sessions as session (session.id)}`, `revokingSessionId`, `isLoggingOutAll`, and `isChangingPassword`. Load failure shows `PageState` with retry. Empty shows a safe empty state. Each row shows coarse device label/timestamps, remembered state, and a `อุปกรณ์นี้` badge.

After successful other-device revoke, call only `removeRevokedSession`. After successful current revoke, call `authAPI.revokeSession(session.id, { current: true })`; after logout-all, use its confirmed-success AuthAPI method; then navigate to `/login`. A failed current/all mutation preserves the current user and CSRF state. After password change, call only `keepCurrentSession`, clear form fields, and keep the current user signed in. Use alert dialogs for current/all destructive actions and `LoadingButton` for each mutation.

Create the page:

```ts
export const _meta = { access: { authenticated: true } } as const;
```

```svelte
<script lang="ts">
	import { PageShell } from '$lib/components/app-layout';
	import SessionSecurityPanel from '$lib/features/session-security/SessionSecurityPanel.svelte';
</script>

<PageShell title="ความปลอดภัยของบัญชี" description="จัดการรหัสผ่านและอุปกรณ์ที่เข้าสู่ระบบ">
	<SessionSecurityPanel />
</PageShell>
```

Extend both metadata and normalized route access with `authenticated?: boolean`; include an access record when it is true; `userCanAccessRoute` rejects a null user before user-type/permission checks.

Add `ความปลอดภัยของบัญชี` with a shield icon to `ProfileMenu` for every user type and remove email rendering. In the staff profile page, read national ID and creation time only from the explicit `ProfileResponse`, never as fallback fields from the shell user. Replace each staff/student settings security password form with one card/button linking to `/account/security`; preserve application, install, and notification settings. Remove `nationalId`, `email`, `phone`, `createdAt`, and duplicate `permissions` from the `User` view model only after all uses are removed; permissions remain exclusively in the permission store. Change the store boundary to `setUser(user: User, permissions: string[])`, have `normalizeCurrentUser` return `{ user, permissions }`, and update login plus refresh call sites to pass `userData.permissions` separately. Add a whole-frontend scan proving no call site reads `authStore.user.permissions` or `user.permissions` as an auth source.

Because this task edits/creates Svelte, use the loaded Svelte skills and resolve every autofixer issue.

- [ ] **Step 4: Run feature tests, Svelte autofixers, generated checks, and frontend checks**

Run:

```bash
cd frontend-school
node --test tests/static/account-security.test.mjs tests/static/api-global-contract.test.mjs tests/static/api-response-contract.test.mjs
npx @sveltejs/mcp svelte-autofixer src/lib/features/session-security/SessionSecurityPanel.svelte
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/account/security/+page.svelte'
npx @sveltejs/mcp svelte-autofixer src/lib/components/layout/ProfileMenu.svelte
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/profile/+page.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/settings/+page.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/student/settings/+page.svelte'
npm run check:api-contracts
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all tests/checks pass and every autofixer reports no issue-level findings.

- [ ] **Step 5: Commit shared account security**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add frontend-school/src/lib/features/session-security frontend-school/src/routes/'(app)'/account/security frontend-school/src/lib/auth/route-access.ts frontend-school/src/lib/api/auth.ts frontend-school/src/lib/components/layout/ProfileMenu.svelte 'frontend-school/src/routes/(app)/staff/profile/+page.svelte' 'frontend-school/src/routes/(app)/staff/settings/+page.svelte' 'frontend-school/src/routes/(app)/student/settings/+page.svelte' frontend-school/src/lib/stores/auth.ts frontend-school/tests/static/account-security.test.mjs frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "feat(auth): add shared account security"
```

### Task 15: Make frontend realtime reconnect depend on authoritative auth refresh

**Files:**
- Create: `frontend-school/src/lib/realtime/auth-recovery.ts`
- Modify: `frontend-school/src/lib/api/client.ts`
- Modify: `frontend-school/src/lib/stores/notification.ts`
- Modify: `frontend-school/src/lib/utils/timetable-socket-runtime.ts`
- Modify: `frontend-school/src/lib/stores/timetable-socket.ts`
- Modify: `frontend-school/tests/static/notification-sse-proxy.test.mjs`
- Modify: `frontend-school/tests/static/timetable-socket-runtime.test.mjs`
- Modify: `frontend-school/tests/static/timetable-realtime-security.test.mjs`

**Interfaces:**
- Consumes: `AuthRefreshResult`, sanitized `PUBLIC_SCHOOL_SUBDOMAIN`, SSE closed state, WebSocket `CloseEvent`, and the existing bounded reconnect runtime.
- Produces: exported `getSchoolSubdomainHint()`, `realtimeAuthRecovery(refresh) -> Promise<'reconnect' | 'retry' | 'stop'>`; SSE/WS append the non-secret tenant query hint only when configured, stop retrying after confirmed revocation, and preserve retry behavior during transient unavailability.

- [ ] **Step 1: Write failing recovery and close-event tests**

```js
test('realtime auth recovery maps authoritative refresh outcomes', async () => {
	const { realtimeAuthRecovery } = await import('../../src/lib/realtime/auth-recovery.ts');
	assert.equal(await realtimeAuthRecovery(async () => 'authenticated'), 'reconnect');
	assert.equal(await realtimeAuthRecovery(async () => 'unavailable'), 'retry');
	assert.equal(await realtimeAuthRecovery(async () => 'unauthenticated'), 'stop');
});

test('policy close passes the CloseEvent and never schedules runtime reconnect', () => {
	const runtime = createRuntimeHarness();
	runtime.connect(params);
	runtime.open();
	runtime.close({ code: 1008, reason: 'Authentication required' });
	assert.equal(runtime.onCloseEvents[0].code, 1008);
	assert.equal(runtime.reconnectTimers.length, 0);
});
```

Update static SSE assertions to require listeners for fixed `session_invalid` and `session_unavailable` control events, an auth refresh before every manual reconnect, no reconnect for `unauthenticated`, and exactly one sanitized `school_subdomain` query parameter when configured. Update timetable store assertions to require the same query-hint source, refresh on code `1008`, and no auth clearing on ordinary network close. Add a client test that invalid `PUBLIC_SCHOOL_SUBDOMAIN` values yield no hint.

- [ ] **Step 2: Run realtime frontend tests and verify the red state**

Run:

```bash
cd frontend-school
node --test tests/static/notification-sse-proxy.test.mjs tests/static/timetable-socket-runtime.test.mjs tests/static/timetable-realtime-security.test.mjs
```

Expected: recovery helper is absent and runtime `onClose` receives no event.

- [ ] **Step 3: Gate reconnects through explicit auth outcomes**

Implement:

```ts
import type { AuthRefreshResult } from '$lib/auth/auth-refresh-policy';

export async function realtimeAuthRecovery(
	refresh: () => Promise<AuthRefreshResult>
): Promise<'reconnect' | 'retry' | 'stop'> {
	const result = await refresh();
	if (result === 'authenticated') return 'reconnect';
	if (result === 'unavailable') return 'retry';
	return 'stop';
}
```

Change runtime dependency `onClose(): void` to `onClose(event: CloseEvent): void` and pass the original event before applying current code-1008 stop behavior. Preserve debounce, jitter, online listener, stale-socket generation, and replacement-intent behavior.

Export the already-sanitized tenant accessor from `client.ts` as `getSchoolSubdomainHint(): string | null`. Build both realtime URLs with `URL`/`URLSearchParams`; append `school_subdomain` exactly once only when this accessor returns a value. Never derive or accept the tenant from arbitrary page query input.

Add one idempotent SSE `recoverAfterSessionSignal()` that first closes/nulls the source, dynamically imports AuthAPI, and awaits `realtimeAuthRecovery(() => authAPI.refreshCurrentUser({ silent: true }))`. Both named control events call it immediately; `onerror` calls it when `readyState === CLOSED` as a handshake/fallback path. For `reconnect`, schedule the existing bounded reconnect. For `retry`, schedule another auth-recovery check before opening a stream. For `stop`, clear the pending timer and leave the source null so the app auth effect redirects. A generation/in-flight guard prevents an old event/error from starting a second recovery or reconnect after `closeSSE()`.

In timetable store `onClose(event)`, clear realtime state as before. Only for code `1008`, run the same auth recovery: `unauthenticated` leaves the stopped runtime and relies on the app auth effect; `unavailable` leaves it stopped until the next user/page action; `authenticated` means a permission change and keeps refreshed permission state without blindly reconnecting into a denial loop.

- [ ] **Step 4: Run realtime tests and frontend check**

Run:

```bash
cd frontend-school
node --test tests/static/notification-sse-proxy.test.mjs tests/static/timetable-socket-runtime.test.mjs tests/static/timetable-realtime-security.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all tests pass; revocation stops loops, transient failures retain bounded recovery, and permission changes refresh the authoritative store.

- [ ] **Step 5: Commit frontend realtime recovery**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add frontend-school/src/lib/realtime/auth-recovery.ts frontend-school/src/lib/api/client.ts frontend-school/src/lib/stores/notification.ts frontend-school/src/lib/utils/timetable-socket-runtime.ts frontend-school/src/lib/stores/timetable-socket.ts frontend-school/tests/static/notification-sse-proxy.test.mjs frontend-school/tests/static/timetable-socket-runtime.test.mjs frontend-school/tests/static/timetable-realtime-security.test.mjs
git commit -m "feat(auth): stop realtime reconnect after revocation"
```

### Task 16: Provision school-session secrets, proxy headers, and trusted runtime configuration

**Files:**
- Modify: `.env.example`
- Modify: `backend-school/.env.example`
- Modify: `docker-compose.yml`
- Modify: `podman-compose.yml`
- Modify: `nginx-configs/school-api.conf.template`
- Modify: `nginx-configs/school-api.maintenance.conf.template`
- Modify: `scripts/lib/schoolorbit-installer/config.sh`
- Modify: `scripts/lib/schoolorbit-installer/vps.sh`
- Create: `scripts/lib/schoolorbit-installer/remote/provision_school_session_runtime.sh`
- Modify: `scripts/tests/installer/fixtures/secrets.json`
- Modify: `scripts/tests/installer/fixtures/runtime.env`
- Modify: `scripts/tests/installer/config_state.bats`
- Modify: `scripts/tests/installer/vps.bats`
- Create: `scripts/tests/installer/provision_school_session_runtime.bats`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: required `SESSION_HMAC_KEY`, rollback-only `SCHOOL_ROLLBACK_JWT_SECRET`, `BASE_DOMAIN`, `TRUSTED_PROXY_CIDRS`, optional `SCHOOL_ALLOWED_DEV_ORIGINS`, canonical Compose ownership, and Nginx-owned CORS.
- Produces: backend-school receives stable session config and its own rotated rollback JWT key; backend-admin continues receiving existing `JWT_SECRET`; browser CORS permits and exposes `X-CSRF-Token`.

- [ ] **Step 1: Write failing installer, Compose, proxy, and secret-separation tests**

Add these assertions to the existing deployment/static suites:

```js
test('school session runtime is required and isolated from admin JWT', async () => {
	const compose = await readRepo('podman-compose.yml');
	const config = await readRepo('scripts/lib/schoolorbit-installer/config.sh');
	const vps = await readRepo('scripts/lib/schoolorbit-installer/vps.sh');
	assert.match(compose, /backend-admin:[\s\S]*JWT_SECRET:\s*\$\{JWT_SECRET\}/);
	assert.match(compose, /backend-school:[\s\S]*JWT_SECRET:\s*\$\{SCHOOL_ROLLBACK_JWT_SECRET\}/);
	assert.match(compose, /SESSION_HMAC_KEY:\s*\$\{SESSION_HMAC_KEY\}/);
	assert.match(compose, /BASE_DOMAIN:\s*\$\{BASE_DOMAIN/);
	assert.match(compose, /TRUSTED_PROXY_CIDRS:/);
	assert.match(config, /SESSION_HMAC_KEY/);
	assert.match(config, /SCHOOL_ROLLBACK_JWT_SECRET/);
	assert.match(vps, /_dotenv_line SESSION_HMAC_KEY/);
	assert.match(vps, /_dotenv_line SCHOOL_ROLLBACK_JWT_SECRET/);
});

test('school proxy permits and exposes only memory CSRF header additions', async () => {
	for (const file of [schoolTemplate, maintenanceTemplate]) {
		const source = await readFile(file, 'utf8');
		for (const line of source.split('\n').filter((value) => value.includes('Access-Control-Allow-Headers'))) {
			assert.match(line, /X-CSRF-Token/);
		}
	}
	for (const file of [schoolTemplate, maintenanceTemplate]) {
		const source = await readFile(file, 'utf8');
		assert.match(source, /Access-Control-Expose-Headers[^\n]*X-CSRF-Token/);
	}
});
```

Add Bats cases that reject 31-character values for each new secret, accept 32 characters, render both into the mode-0600 runtime file, render `BASE_DOMAIN`/proxy CIDRs, and never print secret values to the fake command log.

- [ ] **Step 2: Run focused deployment tests and verify the red state**

Run from repository root:

```bash
bats scripts/tests/installer/config_state.bats scripts/tests/installer/vps.bats
node --test frontend-school/tests/static/deployment-installer.test.mjs
cd backend-school && cargo test --test static_architecture school_session_runtime_is_deployment_owned -- --exact --nocapture
```

Expected: tests fail because session secrets/config and CSRF proxy headers are absent and backend-school still shares `JWT_SECRET`.

- [ ] **Step 3: Wire exact production/local configuration and CORS headers**

Add `SESSION_HMAC_KEY` and `SCHOOL_ROLLBACK_JWT_SECRET` to `SO_REQUIRED_SECRETS`; validate both with minimum length 32. The primary installer accepts operator-supplied stable random values and never echoes them. Add both to `render_runtime_env` and the remote required-name loop. Provide a standalone operator-run VPS helper for the current cutover that generates both values on the target without displaying them, preserves the existing admin `JWT_SECRET`, creates a mode-`0600` backup, writes atomically, and refuses an accidental second rotation.

Render non-secret runtime values exactly:

```bash
_dotenv_line BASE_DOMAIN "${SO_CONFIG[base_domain]}"
_dotenv_line TRUSTED_PROXY_CIDRS '10.0.0.0/8,172.16.0.0/12'
_dotenv_line SCHOOL_ALLOWED_DEV_ORIGINS ''
```

Map Compose environment as follows:

```yaml
backend-admin:
  environment:
    JWT_SECRET: ${JWT_SECRET}

backend-school:
  environment:
    JWT_SECRET: ${SCHOOL_ROLLBACK_JWT_SECRET}
    SESSION_HMAC_KEY: ${SESSION_HMAC_KEY}
    BASE_DOMAIN: ${BASE_DOMAIN:-schoolorbit.app}
    TRUSTED_PROXY_CIDRS: ${TRUSTED_PROXY_CIDRS:-10.0.0.0/8,172.16.0.0/12}
    SCHOOL_ALLOWED_DEV_ORIGINS: ${SCHOOL_ALLOWED_DEV_ORIGINS:-}
```

Use the same separation in local `docker-compose.yml`; examples document a unique random 32+-character key, a different rollback key, local allowlist `http://localhost:5173,http://127.0.0.1:5173`, and no real value. Production fixture values remain clearly synthetic and pass validation.

In every school API `Access-Control-Allow-Headers`, append `X-CSRF-Token`. In every school API `Access-Control-Expose-Headers`, append `X-CSRF-Token`. Add an expose header to the maintenance response. Do not edit `admin-api.conf.template` and do not move CORS into Axum.

Add a backend static test that asserts `SessionConfig::from_env` is initialized before `build_app`, Compose maps the two JWT variables to different services, and the school runtime receives base-domain/proxy configuration.

- [ ] **Step 4: Run the complete deployment matrix required by `.rules`**

Run from repository root:

```bash
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
cd backend-school && cargo test --test static_architecture school_session_runtime_is_deployment_owned -- --exact --nocapture
```

Expected: every command exits zero; rendered runtime values satisfy Compose without exposing a production secret.

- [ ] **Step 5: Commit deployment ownership**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add .env.example backend-school/.env.example docker-compose.yml podman-compose.yml nginx-configs/school-api.conf.template nginx-configs/school-api.maintenance.conf.template scripts/lib/schoolorbit-installer/config.sh scripts/lib/schoolorbit-installer/vps.sh scripts/tests/installer/fixtures/secrets.json scripts/tests/installer/fixtures/runtime.env scripts/tests/installer/config_state.bats scripts/tests/installer/vps.bats frontend-school/tests/static/deployment-installer.test.mjs backend-school/tests/static_architecture.rs
git commit -m "ops(auth): provision school session runtime"
```

### Task 17: Upgrade smoke and multi-context browser coverage

**Files:**
- Modify: `scripts/smoke_test.sh`
- Modify: `scripts/tests/installer/orchestration.bats`
- Modify: `frontend-school/tests/e2e/login.spec.ts`
- Create: `frontend-school/tests/e2e/session-security.spec.ts`
- Modify: `frontend-school/src/lib/features/session-security/SessionSecurityPanel.svelte`
- Modify: `frontend-school/tests/static/account-security.test.mjs`

**Interfaces:**
- Consumes: staging/runtime `SMOKE_*`, normal `E2E_*`, and dedicated disposable `E2E_SESSION_USERNAME`/`E2E_SESSION_PASSWORD` credentials, `__Host-schoolorbit_session`, response `X-CSRF-Token`, session-security UI, and two isolated Playwright browser contexts.
- Produces: proxy-path smoke coverage for CSRF/session APIs and browser proof of selected revocation, logout-all, forced legacy rejection, and optional tenant isolation. It deliberately does not change a shared staging password.

- [ ] **Step 1: Write failing shell/static/browser assertions**

Update static/UI tests to require stable selectors:

```js
assert.match(panel, /data-testid="session-list"/);
assert.match(panel, /data-testid={`session-row-\$\{session\.id\}`}/);
assert.match(panel, /data-current={session\.isCurrent}/);
assert.match(panel, /data-testid="logout-all-sessions"/);
```

Update login E2E expectation:

```ts
const cookies = await page.context().cookies();
const session = cookies.find((cookie) => cookie.name === '__Host-schoolorbit_session');
expect(session).toBeDefined();
expect(session?.httpOnly).toBe(true);
expect(session?.secure).toBe(true);
expect(session?.sameSite).toBe('Lax');
expect(cookies.some((cookie) => cookie.name === 'auth_token')).toBe(false);
```

Add a smoke static assertion in the orchestration fixture that login returns `X-CSRF-Token` plus the new cookie, and mutation requests include the header. Add a browser static guard that the destructive session spec reads `E2E_SESSION_USERNAME`/`E2E_SESSION_PASSWORD` and never falls back to the shared smoke or normal E2E account.

- [ ] **Step 2: Run static and E2E discovery checks and verify the red state**

Run:

```bash
bats scripts/tests/installer/orchestration.bats
cd frontend-school
node --test tests/static/account-security.test.mjs
npx playwright test --list tests/e2e/login.spec.ts tests/e2e/session-security.spec.ts
```

Expected: fixture/new-cookie/selectors fail and the new E2E file is absent.

- [ ] **Step 3: Capture CSRF privately in smoke and implement two-context E2E**

In `smoke_test.sh`, keep the existing private temporary directory/cookie jar and add:

```bash
csrf_token=''
capture_csrf() {
    local headers=$1 value
    value=$(awk 'BEGIN { IGNORECASE=1 } /^X-CSRF-Token:/ { sub(/^[^:]+:[[:space:]]*/, ""); gsub(/\r/, ""); print }' "$headers" | tail -n 1)
    [[ -n $value ]] || return 1
    csrf_token=$value
}

refresh_csrf_if_present() {
    local headers=$1 value
    value=$(awk 'BEGIN { IGNORECASE=1 } /^X-CSRF-Token:/ { sub(/^[^:]+:[[:space:]]*/, ""); gsub(/\r/, ""); print }' "$headers" | tail -n 1)
    [[ -z $value ]] || csrf_token=$value
}

csrf_header_args() {
    [[ -n $csrf_token ]] || return 1
    printf '%s\n' "X-CSRF-Token: $csrf_token"
}
```

Never print `csrf_token`. Require `capture_csrf` after login and `/api/auth/me`; after any other rotation-capable response, call `refresh_csrf_if_present` so a non-rotating response retains the existing value. Assert the cookie jar contains `__Host-schoolorbit_session` and not `auth_token`. Before login, prove an injected legacy `auth_token` alone leaves `/api/auth/me` at `401`, then reset the jar. Add `-H "X-CSRF-Token: $csrf_token"` to every authenticated unsafe file/profile/notification cleanup request. Add `GET /api/auth/sessions`, assert exactly one current session in the smoke flow, and finish with current `POST /api/auth/logout` plus CSRF—not logout-all—so a shared smoke account's other devices are untouched.

Update the Bats fake curl response to write:

```bash
printf '%s\n' 'Access-Control-Allow-Headers: content-type,x-school-subdomain,x-csrf-token' >>"$headers"
printf '%s\n' 'Access-Control-Expose-Headers: x-csrf-token' >>"$headers"
printf '%s\n' 'X-CSRF-Token: fixture-csrf-token' >>"$headers"
printf '%s\n' '#HttpOnly_school-api.example.test FALSE / TRUE 0 __Host-schoolorbit_session fixture' >"$cookie_output"
```

Add `data-testid` attributes from Step 1 to the shared component. Build `session-security.spec.ts` as `test.describe.serial` with a `login(context)` helper using only required `E2E_SESSION_USERNAME`/`E2E_SESSION_PASSWORD`; fail fast when either is missing. Log in contexts A and B, open B `/account/security`, and read B's exact UUID from its `data-current="true"` row. Open A, revoke precisely `session-row-${bSessionId}`, and assert B reaches `/login` after its next protected navigation. Log B in again, click A logout-all, and assert both contexts reach login on the next auth check. Use `try/finally` to close contexts and best-effort current-session logout after a failed assertion; never select an arbitrary non-current row because the disposable account may contain stale test sessions.

Create a third context containing only a synthetic `auth_token` scoped to the API host resolved as `E2E_API_URL ?? SMOKE_API_URL ?? 'https://school-api.schoolorbit.app'`; assert protected navigation redirects to login. When `E2E_OTHER_TENANT_URL` exists, log in to the primary tenant, navigate the same context to the other tenant, and assert tenant-B bootstrap rejects tenant-A's database-local session. Skip only that optional case when the variable is absent.

Do not call change-password in browser E2E; Task 4 owns password atomicity with an isolated database fixture.

Because this task edits Svelte, run the Svelte autofixer and resolve every issue.

- [ ] **Step 4: Run static/shell checks and authenticated staging workflows**

Run:

```bash
shellcheck scripts/smoke_test.sh
shfmt -d -i 4 -ci scripts/smoke_test.sh
bats scripts/tests/installer/orchestration.bats
cd frontend-school
node --test tests/static/account-security.test.mjs
npx @sveltejs/mcp svelte-autofixer src/lib/features/session-security/SessionSecurityPanel.svelte
E2E_BASE_URL="$E2E_BASE_URL" E2E_API_URL="$E2E_API_URL" E2E_USERNAME="$E2E_USERNAME" E2E_PASSWORD="$E2E_PASSWORD" E2E_SESSION_USERNAME="$E2E_SESSION_USERNAME" E2E_SESSION_PASSWORD="$E2E_SESSION_PASSWORD" npx playwright test tests/e2e/login.spec.ts tests/e2e/session-security.spec.ts
cd ..
SMOKE_TENANT_URL="$SMOKE_TENANT_URL" SMOKE_API_URL="$SMOKE_API_URL" SMOKE_ADMIN_API_URL="$SMOKE_ADMIN_API_URL" SMOKE_SUBDOMAIN="$SMOKE_SUBDOMAIN" SMOKE_USERNAME="$SMOKE_USERNAME" SMOKE_PASSWORD="$SMOKE_PASSWORD" scripts/smoke_test.sh
```

Expected: all commands exit zero against a disposable/staging tenant; secrets and CSRF values are absent from output.

- [ ] **Step 5: Commit runtime workflow coverage**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add scripts/smoke_test.sh scripts/tests/installer/orchestration.bats frontend-school/tests/e2e/login.spec.ts frontend-school/tests/e2e/session-security.spec.ts frontend-school/src/lib/features/session-security/SessionSecurityPanel.svelte frontend-school/tests/static/account-security.test.mjs
git commit -m "test(auth): cover school session revocation"
```

### Task 18: Update durable standards, backlog, rollout runbook, and run the final gate

**Files:**
- Modify: `.rules`
- Modify: `TODO.md`
- Modify: `backend-school/README.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/OPERATIONS.md`
- Modify: `docs/PODMAN_SETUP.md`
- Modify: `frontend-school/tests/static/documentation-policy.test.mjs`

**Interfaces:**
- Consumes: the completed runtime contract from Tasks 1–17 and `.rules` change-type matrix.
- Produces: canonical session/auth development rules, exact test recipes, stable secret/cutover/rollback operations, a backlog containing only unfinished identity work, and one final evidence gate.

- [ ] **Step 1: Write failing documentation-policy assertions for the durable contract**

Add assertions that canonical owners state the new boundary without creating another Markdown file:

```js
test('canonical docs own the school session and cutover contract', async () => {
	assert.match(rules, /AuthenticatedSession/);
	assert.match(rules, /__Host-schoolorbit_session/);
	assert.match(rules, /SESSION_HMAC_KEY/);
	assert.doesNotMatch(rules, /current_user_tenant_context_from_claims/);
	assert.match(testing, /X-CSRF-Token/);
	assert.match(testing, /session-security\.spec\.ts/);
	assert.match(operations, /SCHOOL_ROLLBACK_JWT_SECRET/);
	assert.match(operations, /thirty-day|30-day/i);
	assert.match(podmanSetup, /SESSION_HMAC_KEY/);
});
```

Add a backlog assertion that `AUTH-001` is absent and `AUTH-002` retains only explicit profile/PII hardening work.

- [ ] **Step 2: Run documentation policy and verify the red state**

Run:

```bash
cd frontend-school
node --test tests/static/documentation-policy.test.mjs
```

Expected: stale JWT/header-helper/secret text and unfinished backlog wording fail.

- [ ] **Step 3: Update only canonical durable owners**

In `.rules`, replace request-context guidance with:

```text
Authenticated browser handlers receive AuthenticatedSession from the single router-level session middleware. Feature handlers pass it to actor_tenant_context_from_session or current_user_tenant_context_from_session and never parse Cookie, Authorization, session tokens, or CSRF headers. Public tenant handlers continue to use tenant_context/tenant_pool. Browser mutations require the exact tenant Origin plus X-CSRF-Token; the backend-host-only __Host-schoolorbit_session cookie is opaque, HttpOnly, Secure, SameSite=Lax, Path=/, and has no Domain.
```

Add that CSRF is the stable domain-separated HMAC of tenant UUID plus session UUID (not the rotating raw credential), realtime handshakes use `TouchOnly`, remembered replacement cookies use remaining absolute lifetime, and new bcrypt passwords must pass the shared 8–128-scalar/71-byte non-truncating validator.

Replace required backend-school `JWT_SECRET` wording with required `SESSION_HMAC_KEY`, while documenting `SCHOOL_ROLLBACK_JWT_SECRET` as rollback-only and `JWT_SECRET` as backend-admin-owned. Add session/realtime verification commands and keep the generated-contract requirements unchanged.

In `docs/TESTING.md`, replace `auth_token` examples with the new cookie, explain private CSRF capture, list focused Rust session/schema/service/HTTP/realtime commands, list `session-auth-contract`, account-security, auth-state, and Playwright commands, and retain environment-only credentials. Document that `E2E_SESSION_USERNAME`/`E2E_SESSION_PASSWORD` must identify a dedicated disposable account because the suite intentionally exercises logout-all; never reuse `SMOKE_*` or an operator account.

In `docs/OPERATIONS.md`, document stable session HMAC ownership, trusted proxy CIDRs, normal/remembered/rotation/retention limits, redacted observability, and this cutover order:

```text
1. Enter backend-school maintenance and provision SESSION_HMAC_KEY plus a newly generated SCHOOL_ROLLBACK_JWT_SECRET without printing either.
2. Run the centralized all-tenant migration gate through migration 034 and stop on any tenant failure.
3. Deploy session-enabled backend-school while maintenance remains active; backend-admin keeps JWT_SECRET unchanged.
4. Deploy frontend-school, validate Nginx CORS/preflight, then run login, /me, protected read, CSRF mutation, session list/revoke, logout-all, SSE, WebSocket, smoke, and two-context Playwright.
5. Leave maintenance only after all checks pass. Every school user performs one clean login.
```

Rollback keeps migration `034`, deploys the prior backend-school image with `SCHOOL_ROLLBACK_JWT_SECRET` mapped to its `JWT_SECRET`, rolls back frontend-school, and requires another clean login. Never restore the old shared school JWT key and never modify `_sqlx_migrations`.

Update `docs/PODMAN_SETUP.md` and `backend-school/README.md` to list the new runtime variables and local allowed-origin behavior. Do not duplicate the full runbook outside Operations.

Remove completed `AUTH-001` from `TODO.md`. Rewrite `AUTH-002` as the still-unfinished explicit `/api/auth/me/profile` PII/step-up/audit hardening; state that default `/api/auth/me` minimization is complete so it is not re-planned. Leave admission, notification, admin, and other backlog items unchanged.

- [ ] **Step 4: Run the complete final evidence matrix**

Run from repository root with an isolated direct PostgreSQL test URL and staging credentials already exported:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::auth --bin backend-school -- --nocapture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test --bin backend-school
cargo check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/+layout.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/login/+page.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/debug/+page.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/profile/+page.svelte'
npx @sveltejs/mcp svelte-autofixer src/lib/features/session-security/SessionSecurityPanel.svelte
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/account/security/+page.svelte'
npx @sveltejs/mcp svelte-autofixer src/lib/components/layout/ProfileMenu.svelte
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/settings/+page.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/student/settings/+page.svelte'
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run build
node --test tests/static/documentation-policy.test.mjs
cd ..
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh scripts/smoke_test.sh \
  scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh scripts/smoke_test.sh \
  scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
SMOKE_TENANT_URL="$SMOKE_TENANT_URL" SMOKE_API_URL="$SMOKE_API_URL" SMOKE_ADMIN_API_URL="$SMOKE_ADMIN_API_URL" SMOKE_SUBDOMAIN="$SMOKE_SUBDOMAIN" SMOKE_USERNAME="$SMOKE_USERNAME" SMOKE_PASSWORD="$SMOKE_PASSWORD" scripts/smoke_test.sh
cd frontend-school
E2E_BASE_URL="$E2E_BASE_URL" E2E_API_URL="$E2E_API_URL" E2E_USERNAME="$E2E_USERNAME" E2E_PASSWORD="$E2E_PASSWORD" E2E_SESSION_USERNAME="$E2E_SESSION_USERNAME" E2E_SESSION_PASSWORD="$E2E_SESSION_PASSWORD" npx playwright test tests/e2e/login.spec.ts tests/e2e/session-security.spec.ts
cd ..
git diff --check
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git diff --name-only origin/main...HEAD | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git diff --name-only origin/main...HEAD -- backend-school/migrations | rg -v '^backend-school/migrations/034_auth_sessions\.sql$' && exit 1 || true
git diff --stat
git status --short
```

Expected: every automated command exits zero; smoke/E2E prove the deployed proxy path; generated artifacts are clean; both admin-app scans and the applied-migration scan have no output; final status contains only intentional school-session changes.

Review the final diff manually against every success criterion in `docs/superpowers/specs/2026-08-09-school-session-foundation-design.md`. Confirm migration files `001`–`033` are byte-for-byte untouched, no response/log contains a raw credential/PII addition, and no route moved between public/protected sets accidentally.

- [ ] **Step 5: Commit canonical docs after the full gate is green**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add .rules TODO.md backend-school/README.md docs/TESTING.md docs/OPERATIONS.md docs/PODMAN_SETUP.md frontend-school/tests/static/documentation-policy.test.mjs
git commit -m "docs(auth): document school session operations"
git diff --check
git status --short --branch
```
