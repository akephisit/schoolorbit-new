# School Login Username Case Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore login for existing case-sensitive school usernames such as `T0001` without changing passwords, persistence, HTTP contracts, or frontend behavior.

**Architecture:** Keep a single login identifier definition across account lookup and per-identifier throttling. `normalize_login_identifier` will remove surrounding whitespace while preserving case; the existing session service will continue passing that one value to both the HMAC throttle bucket and the exact PostgreSQL username lookup.

**Tech Stack:** Rust, Axum authentication service, SQLx/PostgreSQL integration tests, bcrypt, Cargo.

## Global Constraints

- Modify only backend-school authentication policy/service tests and the policy implementation; do not modify backend-admin or frontend-admin.
- Do not edit an applied migration or add a migration for this regression.
- Do not change, reset, rehash, store, or log a password or plaintext national ID.
- Keep public login failures generic and retain both identifier and source-address throttling.
- Preserve the generated API and permission contracts because the HTTP contract does not change.

---

### Task 1: Preserve Username Case Through Session Login

**Files:**
- Modify: `backend-school/src/modules/auth/session_policy.rs:65-67,235-239`
- Test: `backend-school/src/modules/auth/session_service_tests.rs:297-360`

**Interfaces:**
- Consumes: `normalize_login_identifier(value: &str) -> String`, `AuthServiceFixture::insert_user`, and `AuthServiceFixture::login`.
- Produces: `normalize_login_identifier(value: &str) -> String` with trim-only, case-preserving behavior used unchanged by `session_service::login` for both `identifier_bucket` and `find_session_login_user_by_username`.

- [x] **Step 1: Write the failing policy and login regression tests**

Rename the policy test and replace its expectation with the case-preserving literal:

```rust
#[test]
fn login_identifier_normalization_trims_and_preserves_case() {
    assert_eq!(normalize_login_identifier("  Teacher.หนึ่ง  "), "Teacher.หนึ่ง");
}
```

Add this service-level regression test after `unknown_wrong_and_inactive_logins_share_one_public_error`:

```rust
#[tokio::test]
async fn uppercase_username_login_uses_the_existing_password_hash() {
    let fixture = AuthServiceFixture::new("service_uppercase_username_login").await;
    let user_id = fixture
        .insert_user("T0001", "correct-password", "active")
        .await;

    let result = fixture.login("T0001", "correct-password").await.unwrap();

    assert_eq!(result.user.id, user_id);
    assert_eq!(result.user.username, "T0001");
    assert_eq!(fixture.session_count().await, 1);
}
```

The production mutation caught by these tests is reintroducing lowercase conversion before the exact username lookup. The tests exercise the real PostgreSQL user row, bcrypt verification, and session insert without mocks.

- [x] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml \
  login_identifier_normalization_trims_and_preserves_case -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml \
  uppercase_username_login_uses_the_existing_password_hash -- --nocapture
```

Expected: the policy test fails because it receives `teacher.หนึ่ง`; the service test fails with the generic unauthorized error because the lookup searches for `t0001` while the fixture stored `T0001`.

- [x] **Step 3: Implement the minimal case-preserving normalization**

Change only the normalization function:

```rust
pub fn normalize_login_identifier(value: &str) -> String {
    value.trim().to_string()
}
```

Do not change the session service call sites: their current reuse of the normalized value for the throttle bucket and lookup is the desired aligned data flow once normalization preserves case.

- [x] **Step 4: Run focused tests and verify GREEN**

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml \
  login_identifier_normalization_trims_and_preserves_case -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml \
  uppercase_username_login_uses_the_existing_password_hash -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml \
  modules::auth::session_policy -- --nocapture
```

Expected: all selected tests pass. If the integration-test database is unavailable, configure the repository's existing `TEST_DATABASE_URL` rather than replacing the real database fixture with a mock.

- [x] **Step 5: Run the backend-school verification matrix**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
cd ..
git diff --check
git diff -- backend-school/src/modules/auth/session_policy.rs \
  backend-school/src/modules/auth/session_service_tests.rs
git status --short
```

Expected: formatting, architecture tests, and compilation exit successfully; the final diff contains only the approved school authentication regression and its tests, plus the already approved plan document.

- [x] **Step 6: Commit and publish the fix**

Run:

```bash
git add \
  backend-school/src/modules/auth/session_policy.rs \
  backend-school/src/modules/auth/session_service_tests.rs \
  docs/superpowers/plans/2026-08-10-school-login-username-case-regression.md
git commit -m "fix(auth): preserve school username case"
git push origin main
```

Monitor the backend-school workflow through completion. Do not deploy or modify an admin service. After deployment, wait for any existing short login throttle to expire and validate a real school account through login, session-cookie receipt, and `/api/auth/me`; never place the password in logs or command history.
