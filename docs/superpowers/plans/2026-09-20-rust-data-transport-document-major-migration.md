# Rust Data, Transport, and Document Major Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the Rust data, HTTP transport, encoding, and PDF boundaries to the newest reviewed stable majors while preserving database history, wire formats, TLS behavior, document acceptance, and production rollback.

**Architecture:** `backend-school` remains the single workspace dependency owner for its internal crates, while `backend-admin` keeps its independent manifest. SQLx 0.9 adopts explicit PostgreSQL-only features and explicit `AssertSqlSafe` markers only at audited server-owned dynamic SQL boundaries. Reqwest 0.13 keeps the existing native-TLS behavior through an explicit feature set instead of accepting its new Rustls default. Base64 and Lopdf move behind exact format and PDF-inspection regressions. Direct Rand stays on 0.9.5 in this wave because every current consumer generates security-sensitive bytes; its 0.10 API migration belongs with the Wave 4 authentication and cryptography vectors.

**Tech Stack:** Rust 1.98.1, Cargo, SQLx 0.9/PostgreSQL, Reqwest 0.13/native-tls, Base64 0.23, Lopdf 0.45, Axum/Tokio test servers, rootless Podman/PostgreSQL 18, disposable Neon, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-20-repository-dependency-modernization-design.md`

## Global Constraints

- Read `.rules`, `docs/TESTING.md`, and `docs/OPERATIONS.md` before implementation; `.rules` remains authoritative.
- Start from the accepted `origin/main` on `chore/modernize-rust-data-transport-document-dependencies`; do not implement on `main`.
- Reconfirm the registry versions before editing. The reviewed stable targets on 2026-09-20 are SQLx `0.9.0`, Reqwest `0.13.5`, Base64 `0.23.1`, Rand `0.10.2`, and Lopdf `0.45.0`. If a newer stable release exists at execution time, stop and amend this reviewed plan rather than silently changing its target.
- Rust 1.98.1 is the repository toolchain and satisfies the highest target MSRV, SQLx 0.9's Rust 1.94 requirement. Do not lower the repository toolchain or add an unreviewed toolchain override.
- Never edit, rename, reformat, or regenerate an applied migration. This wave adds no migration and changes no schema, seed data, index, or migration checksum.
- Do not change endpoint paths, request/response DTOs, OpenAPI output, permission definitions, generated permission registries, or realtime payloads.
- Do not add Cargo patch overrides, prereleases, duplicated direct dependency owners, disabled checks, warning allowances, or feature flags merely to make compilation succeed.
- Preserve the current SQL query semantics and round-trip count. SQLx compatibility edits may mark already-audited dynamic SQL as safe, but they must not interpolate untrusted input, add per-row queries, introduce N+1 reads, or conceal a query-plan change. Any genuine query optimization requires representative `EXPLAIN (ANALYZE, BUFFERS)` evidence and a separately reviewed performance change.
- Use `sqlx::AssertSqlSafe` only around a complete query assembled from server-owned constant fragments or an already-validated/quoted test schema identifier. Keep the marker visible at the execution call site; do not hide it behind a generic helper.
- Preserve native TLS for SQLx and Reqwest in this wave. Reqwest 0.13 changes its default TLS backend to Rustls, so both direct manifests must disable defaults and explicitly enable `native-tls`, `charset`, `http2`, `system-proxy`, and `json`; only `backend-admin` enables `query`, because repository search proves it is the only direct owner using `RequestBuilder::query`.
- Keep direct `rand = "0.9.5"` until Wave 4. The only direct source consumers are session-token generation, AES nonce generation, and certificate-proof generation; all are security-format-sensitive. A transitive Rand 0.10 introduced by Lopdf is acceptable and is not a direct-owner migration.
- Preserve exact Base64 alphabets and padding: session tokens and certificate proofs remain URL-safe without padding; encrypted fields remain standard padded Base64. Enable only Base64's `std` feature, not its new default `simd-unsafe`, because these payloads are small security-sensitive values for which the extra unsafe SIMD implementation is not justified.
- Preserve PDF purpose limits, structural validation, encrypted-document rejection, page-count rules, inherited media/crop boxes, normalized rotation, and canonical metadata. Keep Lopdf default features disabled.
- Never add or print plaintext national IDs, credentials, tokens, cookies, database URLs, encryption keys, provider response bodies, or signed URLs in fixtures, logs, commands, or test output.
- Run focused tests after each boundary, then the complete change-type matrix. Treat any unrun database, disposable-Neon, deployment, or authenticated-smoke check as unrun rather than passing.
- Keep this plan on the feature branch while implementing. Remove it after the completed implementation is recorded in branch history and before squash integration, as required by `.rules`.

## Review Focus

- **SQL safety:** every SQLx 0.9 `AssertSqlSafe` use must wrap only a complete server-owned query or validated test identifier; no request value may become SQL text.
- **Database compatibility:** both active migration trees remain byte-for-byte unchanged, existing migrations apply on fresh local PostgreSQL and disposable Neon, and query/result behavior remains covered.
- **Dependency graph:** `backend-school` internal crates continue using workspace dependencies; direct SQLx, Reqwest, Base64, and Lopdf owners resolve to the reviewed versions without forced overrides. Older transitive majors are accepted only when `cargo tree -i` identifies an upstream owner.
- **TLS behavior:** SQLx and Reqwest explicitly retain native TLS and the current default HTTP capabilities; no accidental Rustls-only transport switch occurs.
- **Wire/format compatibility:** Base64 values retain alphabet, padding, length, canonical parsing, and stored ciphertext compatibility. Rand is intentionally not migrated in this wave.
- **Document safety:** valid table/xref-stream PDFs and certificate metadata remain accepted; encrypted, malformed, oversized, multi-page, invalid-box, and invalid-rotation inputs remain rejected with the same public errors.
- **Contracts and security:** OpenAPI and permission artifacts remain unchanged; error handling stays bounded and no sensitive provider or identity data is newly logged.
- **Release acceptance:** both backend-admin deployment and the coordinated backend-school release must succeed for the exact integrated SHA, including readiness, migration audit, authenticated smoke, and accepted-release evidence.

---

## File Structure

### Dependency ownership and durable regression policy

- Modify `backend-school/Cargo.toml` — own SQLx 0.9, Reqwest 0.13, Base64 0.23, the intentional direct Rand 0.9 hold, and Lopdf 0.45 for the school workspace.
- Modify `backend-school/Cargo.lock` — lock the reviewed school graph.
- Modify `backend-admin/Cargo.toml` — own SQLx 0.9 and Reqwest 0.13 for the admin service.
- Modify `backend-admin/Cargo.lock` — lock the reviewed admin graph.
- Modify `backend-school/tests/static_architecture.rs` — enforce direct versions, minimal feature ownership, explicit TLS, and the bounded Rand hold across both manifests.

### SQLx 0.9 compatibility

- Inspect and modify compiler-reported runtime call sites under:
  - `backend-school/crates/school-academic-core/src/services/`
  - `backend-school/crates/school-academic-delivery/src/services/`
  - `backend-school/crates/school-academic-timetable/src/services/`
  - `backend-school/crates/school-academic-assessment/src/`
  - `backend-school/crates/school-academic-results/src/services/`
  - `backend-school/crates/school-academic-lifecycle/src/services/`
  - `backend-school/crates/school-file-platform/src/repository.rs`
  - `backend-school/src/modules/calendar/services/reminders.rs`
  - `backend-school/src/modules/consent/services.rs`
  - `backend-school/src/modules/system/services/route_registration_service.rs`
  - `backend-school/src/bin/migrate_tenant_schema.rs`
- Inspect and modify compiler-reported test/migration-support call sites in:
  - `backend-school/crates/school-test-db/src/lib.rs`
  - `backend-school/crates/school-auth/src/session_repository_tests.rs`
  - `backend-school/crates/school-file-platform/src/schema_tests.rs`
  - `backend-school/src/modules/academic/core/schema_tests.rs`
  - `backend-school/src/modules/academic/core/services_tests.rs`
  - `backend-school/src/modules/academic/cutover_test_support.rs`
  - `backend-school/src/modules/academic/lifecycle/services/promotion_execution_tests.rs`
  - `backend-school/src/modules/academic/lifecycle/services/promotion_review_tests.rs`
  - `backend-school/src/modules/academic/lifecycle/services/provider_tests.rs`
  - `backend-school/src/modules/certificates/schema_tests.rs`
- Inspect `backend-admin/src/main.rs`, `backend-admin/src/bin/create_admin.rs`, `backend-admin/src/models/`, `backend-admin/src/services/`, and `backend-admin/src/handlers/`; modify only compiler-reported SQLx 0.9 incompatibilities.

### Reqwest 0.13 compatibility

- Modify `backend-school/crates/school-tenancy/src/admin_client.rs` only if request/response regression coverage or the new API requires it.
- Modify `backend-school/src/bin/seed_sandbox.rs` only if Reqwest 0.13 requires a compatibility edit.
- Modify `backend-admin/src/clients/neon_client.rs` only if request/response regression coverage or the new API requires it.
- Modify `backend-admin/src/clients/cloudflare_client.rs` only if Reqwest 0.13 requires a compatibility edit.
- Modify `backend-admin/src/clients/backend_school_client.rs` only if Reqwest 0.13 requires a compatibility edit.

### Base64 and PDF compatibility

- Modify `backend-school/crates/school-auth/src/session_crypto.rs` — add exact canonical Base64URL vectors while preserving token behavior.
- Modify `backend-school/crates/school-crypto/src/lib.rs` — add exact standard Base64 and stored-ciphertext compatibility coverage if not already sufficient.
- Modify `backend-school/crates/school-certificates/src/services/proof.rs` only if Base64 0.23 requires a compatibility edit; retain URL-safe no-padding proof behavior.
- Modify `backend-school/crates/school-file-platform/src/file_inspector.rs` — strengthen PDF size/structure characterization only where needed by the Lopdf migration and apply any required 0.45 API compatibility change.

---

### Task 1: Reconfirm the Graph and Record the Rand Security Boundary

**Files:**
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: the reviewed registry targets and current direct-source ownership.
- Produces: an executable policy that keeps Rand 0.10 out of this wave unless a non-security direct consumer exists.

- [ ] **Step 1: Verify the branch, accepted base, clean tree, and toolchain**

Run from the repository root:

```bash
git fetch origin
git status --short --branch
git merge-base --is-ancestor origin/main HEAD
git rev-list --left-right --count origin/main...HEAD
rustc --version
cargo --version
```

Expected: the Wave 3 branch is clean and based on the accepted `origin/main`; Rust and Cargo report 1.98.1. If `origin/main` advanced, merge it into the Wave 3 branch before continuing and re-run this step.

- [ ] **Step 2: Reconfirm every reviewed stable target and feature/MSRV boundary**

Run:

```bash
cargo info sqlx@0.9.0 --verbose
cargo info reqwest@0.13.5 --verbose
cargo info base64@0.23.1 --verbose
cargo info rand@0.10.2 --verbose
cargo info lopdf@0.45.0 --verbose
```

Expected: these remain the newest stable releases; SQLx requires Rust 1.94, Reqwest/Rand require 1.85, Lopdf requires 1.88, and Base64 requires 1.71. Confirm Reqwest exposes `native-tls`, `charset`, `http2`, `system-proxy`, `json`, and `query`; confirm Lopdf still supports `default-features = false`.

- [ ] **Step 3: Capture a passing focused behavior baseline**

Run:

```bash
cd backend-school
cargo test -p school-tenancy admin_client::tests
cargo test -p school-auth session_crypto::tests
cargo test -p school-crypto
cargo test -p school-certificates services::proof::tests -- --test-threads=8
cargo test -p school-file-platform file_inspector::tests
cd ../backend-admin
cargo test clients::
cd ..
```

Expected: all focused tests pass before a dependency major changes.

- [ ] **Step 4: Add the failing direct-Rand ownership policy**

Add `rust_data_transport_document_dependencies_follow_the_reviewed_policy` to `backend-school/tests/static_architecture.rs`. Initially assert:

```rust
let school_manifest = read_source(manifest_dir().join("Cargo.toml"));
assert!(school_manifest.contains(
    "rand = \"0.9.5\" # Held for Wave 4 because every direct consumer generates security-sensitive bytes."
));

let rand_consumers = backend_rs_files()
    .into_iter()
    .filter(|file| read_source(file).contains("rand::"))
    .map(|file| repo_relative(&file))
    .collect::<BTreeSet<_>>();
assert_eq!(
    rand_consumers,
    BTreeSet::from([
        "backend-school/crates/school-auth/src/session_crypto.rs".to_string(),
        "backend-school/crates/school-certificates/src/services/proof.rs".to_string(),
        "backend-school/crates/school-crypto/src/lib.rs".to_string(),
    ])
);
```

- [ ] **Step 5: Run the policy test and confirm the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture rust_data_transport_document_dependencies_follow_the_reviewed_policy -- --exact
```

Expected: FAIL because the reviewed Rand hold is not yet explicit in the manifest.

- [ ] **Step 6: Add the exact Rand hold comment without changing its version**

Change the workspace dependency to:

```toml
rand = "0.9.5" # Held for Wave 4 because every direct consumer generates security-sensitive bytes.
```

Do not change the three source consumers in this wave.

- [ ] **Step 7: Run the focused policy test**

Run:

```bash
cargo test --test static_architecture rust_data_transport_document_dependencies_follow_the_reviewed_policy -- --exact
```

Expected: PASS.

- [ ] **Step 8: Commit the explicit security boundary**

```bash
cd ..
git add backend-school/Cargo.toml backend-school/tests/static_architecture.rs
git commit -m "test: record rand security migration boundary"
```

---

### Task 2: Upgrade Both PostgreSQL Owners to SQLx 0.9

**Files:**
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `backend-admin/Cargo.toml`
- Modify: `backend-admin/Cargo.lock`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: only compiler-reported SQLx call sites listed in the File Structure section.

**Interfaces:**
- Consumes: existing static SQL, server-owned dynamic fragments, typed rows, migration runners, and PostgreSQL native TLS.
- Produces: SQLx 0.9-compatible query execution without schema, SQL text, result-shape, transaction, or round-trip changes.

- [ ] **Step 1: Extend the policy test with failing SQLx assertions**

Read both manifests in `rust_data_transport_document_dependencies_follow_the_reviewed_policy` and assert the exact reviewed declarations:

```rust
assert!(school_manifest.contains(
    "sqlx = { version = \"0.9.0\", default-features = false, features = [\"runtime-tokio\", \"postgres\", \"macros\", \"migrate\", \"chrono\", \"uuid\", \"bigdecimal\", \"json\", \"tls-native-tls\"] }"
));
let admin_manifest = read_source(repo_root().join("backend-admin/Cargo.toml"));
assert!(admin_manifest.contains(
    "sqlx = { version = \"0.9.0\", default-features = false, features = [\"runtime-tokio\", \"postgres\", \"macros\", \"migrate\", \"chrono\", \"uuid\", \"json\", \"tls-native-tls\"] }"
));
```

The explicit `macros`, `migrate`, and `json` features replace capabilities formerly inherited from SQLx defaults; disabling defaults prevents direct MySQL, SQLite, and `Any` owners from entering these PostgreSQL-only services.

- [ ] **Step 2: Run the policy test and confirm the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture rust_data_transport_document_dependencies_follow_the_reviewed_policy -- --exact
```

Expected: FAIL on both SQLx 0.9 declarations.

- [ ] **Step 3: Update both manifests and only the relevant lockfile packages**

Apply the exact declarations from Step 1, then run:

```bash
cargo update -p sqlx@0.8.6 --precise 0.9.0
cd ../backend-admin
cargo update -p sqlx@0.8.6 --precise 0.9.0
```

Do not use `cargo update` without a package selector.

- [ ] **Step 4: Run compile checks and retain the SQLx 0.9 failures as the migration worklist**

Run:

```bash
cd ../backend-school
cargo check --workspace --all-targets
cd ../backend-admin
cargo check --locked --all-targets
```

Expected: compilation may fail at dynamic SQL arguments that no longer satisfy `SqlSafeStr`, and may expose migration/executor API differences. Do not suppress these errors.

- [ ] **Step 5: Audit every dynamic SQL argument before marking it safe**

From the repository root, enumerate the candidate sites:

```bash
rg -n 'sqlx::(query|query_as|query_scalar|raw_sql)\s*\((?:&?\s*[A-Za-z_]|&?\s*format!)' \
  backend-school backend-admin -g '*.rs'
```

For each compiler-reported non-literal query:

1. Trace every fragment to a constant, closed enum/branch, or validated generated test schema identifier.
2. Keep all user values in `.bind(...)`; never concatenate them into SQL.
3. Bind the completed owned query directly with `sqlx::AssertSqlSafe(query)`.
4. Leave string literals and `&'static str` constants unwrapped.
5. Do not add a generic `safe_sql` helper and do not change query count or SQL semantics.

Use this shape for an owned string:

```rust
let query = format!("SELECT {SERVER_OWNED_COLUMNS} FROM server_owned_table WHERE id = $1");
sqlx::query_as(sqlx::AssertSqlSafe(query))
    .bind(id)
    .fetch_one(pool)
    .await
```

For test schemas, retain the existing UUID-derived identifier construction and quoting, then wrap only the completed statement. Add or retain a test proving the generated identifier cannot contain quotes or separators.

- [ ] **Step 6: Apply remaining SQLx 0.9 API compatibility edits**

Resolve compiler-reported changes to `migrate!`, `raw_sql`, executor/database types, derives, and query macros at their current owners. Keep typed rows, transaction boundaries, and public errors unchanged. Do not replace typed results with `serde_json::Value`, runtime casts, or unchecked deserialization.

- [ ] **Step 7: Re-run compile and the direct policy test**

Run:

```bash
cd backend-school
cargo test --test static_architecture rust_data_transport_document_dependencies_follow_the_reviewed_policy -- --exact
cargo check --workspace --all-targets
cd ../backend-admin
cargo check --locked --all-targets
```

Expected: PASS with the 0.9 graph locked in both applications.

- [ ] **Step 8: Run focused SQLx package tests against disposable PostgreSQL**

From the repository root, run:

```bash
./scripts/test_backend_school.sh --package school-migrations
./scripts/test_backend_school.sh --package school-test-db
./scripts/test_backend_school.sh --package school-auth session_repository_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-academic-core -- --test-threads=8
./scripts/test_backend_school.sh --package school-academic-delivery -- --test-threads=8
./scripts/test_backend_school.sh --package school-academic-timetable -- --test-threads=8
./scripts/test_backend_school.sh --package school-academic-assessment -- --test-threads=8
./scripts/test_backend_school.sh --package school-academic-results -- --test-threads=8
./scripts/test_backend_school.sh --package school-academic-lifecycle -- --test-threads=8
./scripts/test_backend_school.sh --package school-file-platform
```

Expected: every package suite passes using the canonical migration runner and isolated local PostgreSQL.

- [ ] **Step 9: Prove migration and contract trees are unchanged**

Run from the repository root:

```bash
wave3_base="$(git merge-base HEAD origin/main)"
git diff --exit-code "$wave3_base" -- backend-school/migrations backend-admin/migrations
git diff --exit-code "$wave3_base" -- \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  contracts/permissions.json \
  contracts/permissions.lock.json \
  backend-school/crates/school-permissions/src/registry_generated.rs \
  frontend-school/src/lib/permissions/registry.generated.ts
```

Expected: no output and exit 0.

- [ ] **Step 10: Format and commit the SQLx migration**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cd ../backend-admin
cargo fmt --all -- --check
cd ..
git add backend-school backend-admin
git commit -m "chore: migrate rust backends to sqlx 0.9"
```

Before committing, verify `git diff --cached --name-only` contains no migration, OpenAPI, or permission artifact.

---

### Task 3: Upgrade Reqwest 0.13 Without Changing TLS Ownership

**Files:**
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `backend-admin/Cargo.toml`
- Modify: `backend-admin/Cargo.lock`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify if required: `backend-school/crates/school-tenancy/src/admin_client.rs`
- Modify if required: `backend-school/src/bin/seed_sandbox.rs`
- Modify if required: `backend-admin/src/clients/neon_client.rs`
- Modify if required: `backend-admin/src/clients/cloudflare_client.rs`
- Modify if required: `backend-admin/src/clients/backend_school_client.rs`

**Interfaces:**
- Consumes: backend-admin internal calls, tenant provisioning, Neon and Cloudflare APIs, existing timeout/retry rules, and native certificate stores.
- Produces: Reqwest 0.13 requests with the same methods, paths, headers, JSON bodies, timeout/retry behavior, response parsing, and native-TLS ownership.

- [ ] **Step 1: Extend the policy test with failing Reqwest declarations**

Add these assertions:

```rust
assert!(school_manifest.contains(
    "reqwest = { version = \"0.13.5\", default-features = false, features = [\"json\", \"native-tls\", \"charset\", \"http2\", \"system-proxy\"] }"
));
assert!(admin_manifest.contains(
    "reqwest = { version = \"0.13.5\", default-features = false, features = [\"json\", \"query\", \"native-tls\", \"charset\", \"http2\", \"system-proxy\"] }"
));
```

`query` belongs only to `backend-admin`, where `NeonClient` and `CloudflareClient` call `RequestBuilder::query`; the other explicit features preserve Reqwest 0.12's default HTTP behavior while retaining native TLS instead of Reqwest 0.13's new Rustls default.

- [ ] **Step 2: Run the policy test and confirm the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture rust_data_transport_document_dependencies_follow_the_reviewed_policy -- --exact
```

Expected: FAIL on both Reqwest declarations.

- [ ] **Step 3: Strengthen transport characterization before the version change**

Use the existing local Axum servers; do not call Neon, Cloudflare, or production services. Ensure focused tests cover:

- `school-tenancy`: internal caller/secret headers, GET path, JSON parsing, timeout, retryable statuses, non-retryable 404, invalid JSON, readiness, and non-retried migration-status PUT.
- `backend-admin` Neon client: named-branch query resolution, authorization header, create-database JSON body/path, and sanitized provider failure.
- `backend_school_client`: success-envelope compatibility. If request construction is not currently covered, add a test-only constructor plus a local Axum test that checks `/internal/provision`, both internal headers, and the camelCase JSON body without logging the supplied secret or password.

Run the characterization suites on Reqwest 0.12 first:

```bash
cd backend-school
cargo test -p school-tenancy admin_client::tests
cd ../backend-admin
cargo test clients::
```

Expected: PASS before the major upgrade. Any newly added assertion that exposes an existing defect must be fixed and committed separately before changing Reqwest.

- [ ] **Step 4: Update both manifests and lockfiles**

Apply the exact declaration from Step 1, then run:

```bash
cd ../backend-school
cargo update -p reqwest@0.12.28 --precise 0.13.5
cd ../backend-admin
cargo update -p reqwest@0.12.28 --precise 0.13.5
```

- [ ] **Step 5: Resolve only Reqwest 0.13 compatibility errors**

Run `cargo check --workspace --all-targets` in `backend-school` and `cargo check --locked --all-targets` in `backend-admin`. Update request construction, response consumption, or error typing only where the compiler or focused tests require it. Preserve:

- existing bounded timeouts and retry limits;
- retries only for safe GET transport failures, 429, and 5xx;
- no automatic retry for provisioning or migration-status mutations;
- internal caller and secret header names;
- sanitized user-facing/provider errors and current typed JSON shapes.

- [ ] **Step 6: Prove feature ownership and transport behavior**

Run:

```bash
cd backend-school
cargo test --test static_architecture rust_data_transport_document_dependencies_follow_the_reviewed_policy -- --exact
cargo test -p school-tenancy admin_client::tests
cargo tree -e features -i reqwest@0.13.5
cd ../backend-admin
cargo test clients::
cargo tree -e features -i reqwest@0.13.5
```

Expected: both trees show the direct owners enabling `native-tls`, `json`, `charset`, `http2`, and `system-proxy`; only the admin tree additionally shows `query`. Focused tests pass. Do not claim that Rustls is absent globally if an unrelated upstream crate owns it transitively.

- [ ] **Step 7: Format and commit the transport migration**

```bash
cd ../backend-school
cargo fmt --all -- --check
cd ../backend-admin
cargo fmt --all -- --check
cd ..
git add backend-school backend-admin
git commit -m "chore: migrate rust http clients to reqwest 0.13"
```

---

### Task 4: Upgrade Base64 While Freezing Stored and Browser-Facing Formats

**Files:**
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `backend-school/crates/school-auth/src/session_crypto.rs`
- Modify if required: `backend-school/crates/school-crypto/src/lib.rs`
- Modify if required: `backend-school/crates/school-certificates/src/services/proof.rs`

**Interfaces:**
- Consumes: 32-byte session/CSRF material, nonce-prefixed encrypted fields, and encrypted certificate proofs.
- Produces: byte-for-byte compatible URL-safe-no-pad and standard-padded strings, with canonical parsing unchanged.

- [ ] **Step 1: Add a failing Base64 manifest policy assertion**

Extend the dependency policy test:

```rust
assert!(school_manifest.contains(
    "base64 = { version = \"0.23.1\", default-features = false, features = [\"std\"] }"
));
```

Run the exact policy test and confirm it fails on the current 0.22.1 declaration.

- [ ] **Step 2: Add an exact pre-upgrade session encoding vector**

In `session_crypto.rs`, extend the existing token round-trip test or add a focused test using the test-only byte constructor:

```rust
let token = RawSessionToken::from_bytes([1_u8; 32]);
assert_eq!(
    token.encode().expose_for_cookie(),
    "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"
);
```

Use the module's existing test-only accessor rather than adding a runtime secret-exposure API. Keep the current rejection cases for padding, non-URL-safe characters, surrounding whitespace/quotes, and wrong decoded lengths.

- [ ] **Step 3: Characterize standard padded ciphertext encoding**

In `school-crypto`, retain the existing encrypt/decrypt round trip and add an exact engine vector in the test module:

```rust
assert_eq!(
    general_purpose::STANDARD.encode([0_u8, 1, 2, 253, 254, 255]),
    "AAEC/f7/"
);
```

Also keep this stored ciphertext fixture produced by the pre-upgrade implementation and assert that it decrypts under the existing test-only key:

```rust
env::set_var("ENCRYPTION_KEY", "test-key-for-testing-only");
assert_eq!(
    decrypt("AAECAwQFBgcICQoLMz5OUZn55E+f9hdUy/hd2vqHkvCmExY6AzMgW9b6NkAGj9uHkA==")
        .expect("legacy ciphertext fixture should decrypt"),
    "stored-format-fixture"
);
```

Never place a real key or identity value in source.

- [ ] **Step 4: Run all format tests on Base64 0.22.1**

Run:

```bash
cd backend-school
cargo test -p school-auth session_crypto::tests
cargo test -p school-crypto
cargo test -p school-certificates services::proof::tests -- --test-threads=8
```

Expected: PASS, establishing exact pre-upgrade outputs.

- [ ] **Step 5: Upgrade Base64 and resolve only API compatibility**

Update the workspace manifest to `base64 = { version = "0.23.1", default-features = false, features = ["std"] }`. This deliberately excludes 0.23's new default `simd-unsafe` feature at the token/ciphertext boundary. Because Axum and other upstream crates still own Base64 0.22 transitively, let Cargo add the new direct 0.23 owner without trying to force-replace the older transitive package:

```bash
cargo check -p school-auth --all-targets
cargo test --test static_architecture rust_data_transport_document_dependencies_follow_the_reviewed_policy -- --exact
cargo test -p school-auth session_crypto::tests
cargo test -p school-crypto
cargo test -p school-certificates services::proof::tests -- --test-threads=8
cargo tree -i base64@0.23.1
cargo tree -i base64@0.22.1
```

Expected: the dependency policy and all exact format/round-trip tests pass. The direct school owners use 0.23.1; any retained 0.22.1 owners are upstream transitive crates. Retain `URL_SAFE_NO_PAD` for tokens/proofs and `STANDARD` for encrypted fields; do not substitute an alphabet or padding mode.

- [ ] **Step 6: Format and commit the encoding migration**

```bash
cargo fmt --all -- --check
cd ..
git add backend-school
git commit -m "chore: migrate canonical encodings to base64 0.23"
```

---

### Task 5: Upgrade Lopdf With Bounded PDF Inspection Regressions

**Files:**
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify if required: `backend-school/crates/school-file-platform/src/file_inspector.rs`

**Interfaces:**
- Consumes: uploaded PDF bytes already bounded by the purpose registry.
- Produces: the same `FileInspectionMetadata::Pdf` and `FileInspectionError` decisions for accepted and rejected files.

- [ ] **Step 1: Add a failing Lopdf policy assertion**

Extend the dependency policy test:

```rust
assert!(school_manifest.contains(
    "lopdf = { version = \"0.45.0\", default-features = false }"
));
```

Run the exact policy test and confirm it fails on 0.38.0.

- [ ] **Step 2: Complete pre-upgrade PDF characterization**

Keep the current tests for table xrefs, xref streams, malformed offsets/root/stream lengths, one-page inherited boxes, normalized rotation, encryption, page count, and unsafe page dimensions. Add one explicit test proving the purpose byte limit rejects a PDF-shaped oversized payload before Lopdf parsing:

```rust
let limits = purpose_definition(FilePurpose::Transcript)
    .expect("transcript purpose should exist")
    .limits;
let mut oversized = b"%PDF-1.7\n".to_vec();
oversized.resize(
    usize::try_from(limits.max_bytes).expect("test byte limit should fit usize") + 1,
    b' ',
);
assert_eq!(
    inspect_file(FilePurpose::Transcript, &oversized),
    Err(FileInspectionError::ByteLimitExceeded)
);
```

Run on Lopdf 0.38.0:

```bash
cd backend-school
cargo test -p school-file-platform file_inspector::tests
```

Expected: PASS.

- [ ] **Step 3: Upgrade Lopdf without enabling optional document features**

Update the manifest and lockfile:

```bash
cargo update -p lopdf@0.38.0 --precise 0.45.0
```

Keep `default-features = false`; do not enable Rayon, image embedding, async runtimes, or clock features for inspection-only use.

- [ ] **Step 4: Resolve Lopdf 0.45 compatibility at the inspection boundary**

Compile and run the focused suite. Account for Lopdf's changed encryption-state semantics only if the compiler/test proves it is relevant; retain the application-level raw `/Encrypt` declaration check and the same public error. Do not loosen cross-reference, object-count, trailer-root, page-tree, box, rotation, or purpose limits.

Run:

```bash
cargo test --test static_architecture rust_data_transport_document_dependencies_follow_the_reviewed_policy -- --exact
cargo test -p school-file-platform file_inspector::tests
cargo test -p school-certificates -- --test-threads=8
cargo check -p school-file-platform --all-targets
```

Expected: PASS with exact metadata and rejection behavior unchanged.

- [ ] **Step 5: Inspect the intentional transitive Rand split**

Run:

```bash
cargo tree -i rand@0.9.5
cargo tree -i rand@0.10.2
cargo tree -d
```

Expected: direct school-auth/school-crypto/school-certificates consumers remain on 0.9.5, while any 0.10.2 owner is transitive (for example Lopdf). Record unexpected direct ownership as a failure; do not force a unified Rand major in this wave.

- [ ] **Step 6: Format and commit the PDF migration**

```bash
cargo fmt --all -- --check
cd ..
git add backend-school
git commit -m "chore: migrate pdf inspection to lopdf 0.45"
```

---

### Task 6: Verify the Complete Wave on Local PostgreSQL and Disposable Neon

**Files:**
- Verify: all changed Rust manifests, lockfiles, source, and tests.
- Verify unchanged: both migration trees, API contract, and permission artifacts.

**Interfaces:**
- Consumes: the complete Wave 3 candidate tree.
- Produces: evidence that compilation, behavior, schema compatibility, contracts, and direct dependency ownership all remain valid before integration.

- [ ] **Step 1: Audit direct and duplicate dependency ownership**

Run in each backend:

```bash
cd backend-school
cargo tree -e features -i sqlx@0.9.0
cargo tree -e features -i reqwest@0.13.5
cargo tree -i base64@0.23.1
cargo tree -i lopdf@0.45.0
cargo tree -d
cd ../backend-admin
cargo tree -e features -i sqlx@0.9.0
cargo tree -e features -i reqwest@0.13.5
cargo tree -d
```

Expected: both applications have only their intended direct owners and explicit features. For every older version reported by `cargo tree -d`, run `cargo tree -i` with that exact crate name and version; retain it only when an upstream crate owns it.

- [ ] **Step 2: Run the complete backend-school local matrix**

Run:

```bash
cd ../backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check --workspace --all-targets
RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school
cd ..
./scripts/test_backend_school.sh -- --test-threads=8
```

Expected: formatting, architecture, all targets, deny-warnings locked build, and the full application suite pass.

- [ ] **Step 3: Run the complete backend-admin matrix**

Run:

```bash
cd backend-admin
cargo fmt --all -- --check
cargo test
cargo check --locked --all-targets
cd ..
```

Expected: PASS.

- [ ] **Step 4: Verify generated contracts without rewriting them**

Run:

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run check:permissions
npm run test:permissions
cd ..
```

Expected: all checks pass and generate no tracked diff.

- [ ] **Step 5: Prove immutable and generated trees remain untouched**

Run:

```bash
wave3_base="$(git merge-base HEAD origin/main)"
git diff --exit-code "$wave3_base" -- backend-school/migrations backend-admin/migrations
git diff --exit-code "$wave3_base" -- \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  contracts/permissions.json \
  contracts/permissions.lock.json \
  backend-school/crates/school-permissions/src/registry_generated.rs \
  frontend-school/src/lib/permissions/registry.generated.ts
```

Expected: no output and exit 0.

- [ ] **Step 6: Push the feature candidate and run disposable Neon compatibility**

Run:

```bash
git push -u origin chore/modernize-rust-data-transport-document-dependencies
gh workflow run backend-school-neon-compatibility.yml \
  --ref chore/modernize-rust-data-transport-document-dependencies \
  -f confirm_disposable_branch=true
```

Locate the exact branch-head run with `gh run list`, then watch it to completion:

```bash
feature_sha="$(git rev-parse HEAD)"
neon_run_id=""
for _attempt in {1..12}; do
  neon_run_id="$(
    gh run list \
      --workflow backend-school-neon-compatibility.yml \
      --branch chore/modernize-rust-data-transport-document-dependencies \
      --event workflow_dispatch \
      --json databaseId,headSha \
      --jq ".[] | select(.headSha == \"$feature_sha\") | .databaseId" \
      | head -n 1
  )"
  test -z "$neon_run_id" || break
  sleep 5
done
test -n "$neon_run_id"
gh run watch "$neon_run_id" --exit-status
```

Expected: the workflow creates a disposable direct Neon branch, provisions prerequisites, applies the unchanged migration timeline, passes schema tests, and deletes the branch. Do not proceed on a skipped, cancelled, or cleanup-failed result.

- [ ] **Step 7: Perform final diff and secret review**

Run:

```bash
git diff --check
git status --short
git diff "$(git merge-base HEAD origin/main)"...HEAD --stat
git diff "$(git merge-base HEAD origin/main)"...HEAD -- \
  backend-school backend-admin
```

Review every `AssertSqlSafe`, manifest feature, client error, encoding vector, and PDF decision. Confirm no migration, contract, permission, plaintext identity, credential, token, database URL, or provider body entered the diff.

---

### Task 7: Integrate, Deploy Both Backends, and Accept the Exact SHA

**Files:**
- Delete before integration: `docs/superpowers/plans/2026-09-20-rust-data-transport-document-major-migration.md`
- Preserve: `docs/superpowers/specs/2026-09-20-repository-dependency-modernization-design.md` for Wave 4.

**Interfaces:**
- Consumes: the fully verified feature tree and accepted pre-Wave-3 production baseline.
- Produces: one squash commit on `main`, successful backend-admin and backend-school deployments, and exact-SHA production acceptance.

- [ ] **Step 1: Remove the completed workflow artifact and make the final feature commit**

After all implementation and review steps pass:

```bash
git rm docs/superpowers/plans/2026-09-20-rust-data-transport-document-major-migration.md
git add -A
git commit -m "chore: complete rust data transport document migration"
```

Keep the design spec because the four-wave modernization program is not complete until Wave 4 is accepted.

- [ ] **Step 2: Recheck remote divergence and merge any advanced base on the feature branch**

Run:

```bash
git fetch origin
git status --short --branch
git rev-list --left-right --count origin/main...HEAD
```

If `origin/main` advanced, merge it into the feature branch, resolve conflicts there, and rerun every affected focused and matrix check plus disposable Neon compatibility. Stop if the tree is dirty or conflicts remain.

- [ ] **Step 3: Record the verified feature tree and squash into updated `main`**

Run:

```bash
feature_head="$(git rev-parse HEAD)"
feature_tree="$(git rev-parse HEAD^{tree})"
git switch main
git merge --ff-only origin/main
git merge --squash "$feature_head"
git commit -m "chore: modernize rust data transport document dependencies"
integrated_tree="$(git rev-parse HEAD^{tree})"
test "$feature_tree" = "$integrated_tree"
```

Expected: the tree IDs match exactly. If they do not, do not push; inspect the integrated diff and rerun verification on the actual integrated tree.

- [ ] **Step 4: Push `main` normally and identify exact-SHA workflow runs**

Run:

```bash
git push origin main
release_sha="$(git rev-parse HEAD)"
gh run list --commit "$release_sha" --limit 20
```

Never force-push `main`.

- [ ] **Step 5: Require all applicable exact-SHA CI and deployments**

Watch the runs for the integrated SHA and require success for:

- `API Contract`
- `Permission Contract`
- `Deploy Backend Admin`
- `Deploy School Release`

The school release must resolve to a backend-bearing scope, build the exact SHA, complete readiness and all-tenant migration audit, restore the proxy, pass authenticated academic smoke, and finish `accept-release`. The backend-admin workflow must build, deploy, pass `/ready`, and verify the service identity. Because both workflows share the runtime deployment concurrency group, queued serialization is expected and is not a failure.

- [ ] **Step 6: Verify production readiness and accepted release state**

Use bounded public checks without credentials in output:

```bash
curl --fail --silent --show-error https://admin-api.schoolorbit.app/ready | jq -e \
  '.status == "ready" and .database == "connected"'
curl --fail --silent --show-error https://school-api.schoolorbit.app/health | jq -e \
  '.status == "healthy"'
curl --fail --silent --show-error https://school-api.schoolorbit.app/ready | jq -e \
  '.status == "ready" and .controlPlane == "connected" and .filePlatform == "ready"'
curl --fail --silent --show-error https://school-api.schoolorbit.app/deployment-status | jq .
curl --fail --silent --show-error --output /dev/null https://snwsb.schoolorbit.app/
curl --fail --silent --show-error --output /dev/null https://admin.schoolorbit.app/
```

Download the `school-release-state` artifact for the exact `Deploy School Release` run and verify its `releaseId` and `backendAcceptedSha` equal the integrated SHA. Confirm the deploy-backend and accept-release jobs succeeded; do not infer acceptance from HTTP 200 alone.

- [ ] **Step 7: Retain rollback state until acceptance, then hand off Wave 4**

Keep the feature branch until both deployment workflows and production checks pass. The pre-Wave-3 accepted backend image remains the rollback boundary. After acceptance, report:

- exact integrated SHA;
- direct versions and explicit feature decisions;
- intentional direct Rand 0.9.5 hold and any transitive duplicate owners;
- local, Neon, CI, deployment, smoke, and accepted-release evidence;
- confirmation that migrations, OpenAPI, and permissions were unchanged.

Then prepare the separately reviewed Wave 4 plan for JsonWebToken 11, bcrypt alignment, AES-GCM 0.11, HMAC 0.13, SHA-2 0.11, and direct Rand 0.10 with stored-format and known-vector coverage. Do not begin Wave 4 source changes as part of this plan.
