# Backend School Crate Workspace Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert `backend-school` into a non-virtual Cargo workspace and extract the canonical permission registry and tenant migration runner into reusable, guarded library crates without changing runtime or contract behavior.

**Architecture:** Keep the existing `backend-school` package as the sole application/composition root and add two one-way dependencies: `backend-school -> school-permissions` and `backend-school -> school-migrations -> school-permissions`. Move ownership directly, remove the root compatibility modules and production `#[path]` reuse, and make static tests, generation, CI, documentation, and compile measurements recognize the workspace boundary.

**Tech Stack:** Rust 2021, Cargo workspaces, Axum 0.8, Tokio 1, SQLx 0.8/PostgreSQL, DashMap 6, Serde 1, Node.js 24 contract generators, GitHub Actions, Podman.

**Spec:** [`docs/superpowers/specs/2026-09-14-backend-school-crate-workspace-design.md`](../specs/2026-09-14-backend-school-crate-workspace-design.md), governed by [`docs/superpowers/specs/2026-09-14-backend-school-crate-architecture-design.md`](../specs/2026-09-14-backend-school-crate-architecture-design.md)

## Global Constraints

- The workspace root is `backend-school/Cargo.toml`; it remains both the `backend-school` package and a non-virtual Cargo workspace using resolver `2`.
- The deployed executable remains `backend-school`; no service, process, image, port, environment variable, route, DTO, OpenAPI, session, realtime, or proxy topology changes.
- `backend-admin` is not a workspace member.
- Never edit any file under `backend-school/migrations/` or `backend-school/migrations_legacy/` during this checkpoint.
- Permission codes, metadata, order, contract digest, lock content, and generated TypeScript output remain byte-for-byte equivalent.
- The generated Rust registry owner is `backend-school/crates/school-permissions/src/registry_generated.rs`; generated files are never edited manually.
- No compatibility re-export remains under `crate::permissions`, and no production Rust target uses `#[path]` to share implementation.
- Internal crates must not depend on the root `backend-school` package or accept its complete `AppState`.
- Dependency versions and `dead_code`/`unused_imports` deny lints are centralized at the workspace root; every member opts into workspace lints.
- `school-migrations` preserves the exact `Result` signatures and error text of the current migration and permission-sync operations.
- Database-backed tests use `TEST_DATABASE_URL` through `scripts/test_backend_school.sh`; no database URL or secret is logged or committed.
- Compile evidence uses the same branch, toolchain, machine, Cargo configuration, dependency cache, and target directory; three comparable runs use the median, and a clean local-package regression above 10 percent blocks completion.
- Cargo timing output and benchmark notes stay outside the repository and are not committed.

---

## File Map

New owners:

- `backend-school/crates/school-permissions/Cargo.toml` — permission crate manifest and shared lint/dependency opt-ins.
- `backend-school/crates/school-permissions/src/lib.rs` — public `registry` module declaration only.
- `backend-school/crates/school-permissions/src/registry.rs` — `PermissionDef` plus the generated artifact include.
- `backend-school/crates/school-permissions/src/registry_generated.rs` — generator-owned constants and `ALL_PERMISSIONS`.
- `backend-school/crates/school-migrations/Cargo.toml` — migration crate manifest and dependency direction.
- `backend-school/crates/school-migrations/build.rs` — migration timeline invalidation for Cargo.
- `backend-school/crates/school-migrations/src/lib.rs` — permission sync, tenant migrator, and `MigrationTracker` public API plus database-independent unit tests.
- `backend-school/src/db/migration_tests.rs` — root binary database-backed permission reconciliation tests using the existing test database harness.

Modified integration points:

- `backend-school/Cargo.toml` — non-virtual workspace, shared dependency versions/lints, and the two path dependencies.
- `backend-school/Cargo.lock` — Cargo-generated local package graph.
- `backend-school/src/main.rs`, `backend-school/src/db.rs`, `backend-school/src/utils.rs` — remove obsolete root module ownership.
- `backend-school/src/db/pool_manager.rs`, `backend-school/src/test_helpers.rs`, `backend-school/src/middleware/permission.rs`, `backend-school/src/modules/system/services/provision_service.rs`, certificate/academic schema tests, and utility binaries — consume `school_migrations` directly.
- Every Rust file under `backend-school/src/` that currently resolves `crate::permissions::registry` — consume `school_permissions::registry` directly.
- `scripts/generate-permissions.mjs` and `frontend-school/tests/static/api-global-contract.test.mjs` — use the canonical generated Rust path.
- `backend-school/tests/static_architecture.rs` — cover workspace source and dependency/ownership rules.
- `.rules`, `backend-school/README.md`, `docs/TESTING.md` — record crate admission and workspace verification.
- `.github/workflows/permission-contract.yml`, `.github/workflows/api-contract.yml` — watch workspace source and check all workspace targets.

Removed owners:

- `backend-school/src/permissions.rs`
- `backend-school/src/permissions/registry.rs`
- `backend-school/src/permissions/registry_generated.rs`
- `backend-school/src/db/migration.rs`
- `backend-school/src/utils/permission_sync.rs`

## Task 1: Capture the Pre-Split Compile Baseline

**Files:**

- Read: `backend-school/Cargo.toml`
- Read: `backend-school/src/db/migration.rs`
- Do not create repository files; Cargo artifacts live under `/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915`.

**Interfaces:**

- Consumes: current single-package `backend-school` graph at the accepted spec commit.
- Produces: terminal-recorded medians for warm no-change check, local-package rebuild, migration-only invalidation, multi-target test compilation, and Cargo timing package status.

- [x] **Step 1: Verify the source baseline is uncontaminated**

Run:

```bash
git status --short
git diff -- backend-school/src backend-school/Cargo.toml backend-school/Cargo.lock
rustc --version
cargo --version
test ! -e /home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915
```

Expected: the working tree is clean, the backend source/manifests have no diff from the plan commit, and the benchmark target does not exist.

- [x] **Step 2: Warm external dependencies once**

Run from `backend-school`:

```bash
CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --all-targets
```

Expected: PASS for the current root package and all three binary targets.

- [x] **Step 3: Measure three no-change warm checks**

Define this shell helper once, then run the measurement three times and record its elapsed seconds
from the terminal:

```bash
measure_seconds() {
  measurement_label="$1"
  shift
  measurement_start=$(date +%s%N)
  "$@"
  measurement_status=$?
  measurement_end=$(date +%s%N)
  awk -v label="$measurement_label" \
    -v start="$measurement_start" -v end="$measurement_end" \
    'BEGIN { printf "%s seconds=%.3f\n", label, (end - start) / 1000000000 }'
  return "$measurement_status"
}
measure_seconds 'baseline warm' env \
  CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --all-targets
```

Expected: PASS three times; retain the median in the execution notes, not in a repository file.

- [x] **Step 4: Measure three local-package rebuilds with external dependencies warm**

For each of three runs, execute:

```bash
CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo clean -p backend-school
measure_seconds 'baseline local rebuild' env \
  CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --all-targets
```

Expected: PASS three times; Cargo rebuilds `backend-school` while already-built third-party dependencies remain reusable.

- [x] **Step 5: Measure three reversible migration-source invalidations**

For run numbers 1, 2, and 3, use `apply_patch` to add this one line immediately above
`fn all_migrations_without_db_lock()`, substituting the literal run number:

```rust
// Compile-boundary measurement probe 1; removed immediately after this check.
```

Run:

```bash
measure_seconds 'baseline migration change run=1' env \
  CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --all-targets -vv
```

After each run, use `apply_patch` to remove the exact probe line before adding the next numbered
probe. After run 3, remove it and run:

```bash
git diff --exit-code -- backend-school/src/db/migration.rs
```

Expected: all three checks pass, the verbose output shows the source invalidating the application
targets, the median is recorded in execution notes, and the source file returns byte-for-byte to
its original content.

- [x] **Step 6: Measure three multi-target test compilations and emit Cargo timings**

For each of three runs:

```bash
CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo clean -p backend-school
measure_seconds 'baseline test compile' env \
  CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo test --all-targets --no-run
```

Then run:

```bash
CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --all-targets --timings
```

Expected: PASS; timing HTML stays in the temporary target tree and no benchmark artifact appears in `git status`.

## Task 2: Extract the Canonical Permission Registry

**Files:**

- Create: `backend-school/crates/school-permissions/Cargo.toml`
- Create: `backend-school/crates/school-permissions/src/lib.rs`
- Move: `backend-school/src/permissions/registry.rs` to `backend-school/crates/school-permissions/src/registry.rs`
- Generate at new path: `backend-school/crates/school-permissions/src/registry_generated.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `scripts/generate-permissions.mjs`
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `scripts/test_backend_school.sh`
- Modify: `scripts/tests/backend-school-test-database.test.mjs`
- Modify: every `backend-school/src/**/*.rs` file reported by `rg -l 'crate::permissions::registry|permissions::registry::codes' backend-school/src --glob '*.rs'`
- Delete: `backend-school/src/permissions.rs`
- Delete after regeneration: `backend-school/src/permissions/registry_generated.rs`
- Test: `scripts/tests/generate-permissions.test.mjs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Consumes: `contracts/permissions.json`; Serde derive macros.
- Produces: `school_permissions::registry::PermissionDef`, `school_permissions::registry::codes`, and `school_permissions::registry::ALL_PERMISSIONS: &[PermissionDef]` with unchanged values and ordering.

- [x] **Step 1: Add failing ownership and workspace tests**

In `backend-school/tests/static_architecture.rs`, add path helpers and a test with these assertions:

```rust
fn workspace_crate_dir(name: &str) -> PathBuf {
    manifest_dir().join("crates").join(name)
}

#[test]
fn permission_registry_has_one_workspace_owner() {
    let manifest = read_source(manifest_dir().join("Cargo.toml"));
    let crate_root = workspace_crate_dir("school-permissions");
    let wrapper = read_source(crate_root.join("src/registry.rs"));

    assert!(manifest.contains("\"crates/school-permissions\""));
    assert!(manifest.contains("school-permissions = { path = \"crates/school-permissions\" }"));
    assert!(wrapper.contains("include!(\"registry_generated.rs\")"));
    assert!(!manifest_dir().join("src/permissions.rs").exists());
    assert!(!manifest_dir().join("src/permissions").exists());
}
```

Change every existing permission-registry fixture path in this test file from `src/permissions/...` to `crates/school-permissions/src/...`, including the exemption in `backend_rs_files()` scans.

In `frontend-school/tests/static/api-global-contract.test.mjs`, make all four Rust registry reads target:

```javascript
path.join(
  repoRoot,
  'backend-school/crates/school-permissions/src/registry_generated.rs'
)
```

and make the wrapper read target:

```javascript
path.join(repoRoot, 'backend-school/crates/school-permissions/src/registry.rs')
```

- [x] **Step 2: Run the focused tests and verify the new boundary is absent**

Run:

```bash
cd backend-school
cargo test --test static_architecture permission_registry_has_one_workspace_owner -- --exact
cd ../frontend-school
node --test --test-name-pattern='permission' tests/static/api-global-contract.test.mjs
```

Expected: FAIL because the workspace member and new canonical files do not exist.

- [x] **Step 3: Convert the root manifest into a non-virtual workspace**

Keep the current `[package]` section and add these sections to `backend-school/Cargo.toml`:

```toml
[workspace]
members = ["crates/school-permissions"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.lints.rust]
dead_code = "deny"
unused_imports = "deny"

[workspace.dependencies]
aes-gcm = "0.10"
async-stream = "0.3.6"
async-trait = "0.1"
aws-config = "1.1"
aws-credential-types = "1.1"
aws-sdk-s3 = "1.11"
axum = { version = "0.8.7", features = ["multipart", "ws"] }
base64 = "0.22"
bcrypt = "0.19"
bigdecimal = { version = "0.4", features = ["serde"] }
bytes = "1.11"
chrono = { version = "0.4", features = ["serde"] }
chrono-tz = "0.10.4"
dashmap = "6.1.0"
dotenvy = "0.15"
futures = "0.3.31"
hex = "0.4"
hmac = "0.12"
http-body-util = "0.1"
image = "0.25"
ipnet = "2"
lopdf = { version = "0.38", default-features = false }
mime_guess = "2.0"
rand = "0.9"
regex = "1"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"
sha256 = "1.5"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "migrate", "chrono", "uuid", "bigdecimal", "tls-native-tls"] }
subtle = "2.6"
thiserror = "2.0.18"
tokio = { version = "1", features = ["full"] }
tokio-cron-scheduler = "0.15.1"
tokio-stream = "0.1.18"
tokio-test = "0.4"
tower = "0.5"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
ttf-parser = "0.25"
unicode-normalization = "0.1"
url = "2.5"
utoipa = { version = "5.5.0", features = ["chrono", "uuid"] }
uuid = { version = "1.10", features = ["serde", "v4", "v5"] }
web-push = "0.11.0"
zeroize = { version = "1", features = ["derive"] }
school-permissions = { path = "crates/school-permissions" }
```

Replace each version-bearing entry in root `[dependencies]` with the same dependency name using `{ workspace = true }`, preserving comment groupings. Preserve the root-only `tower` `util` feature by writing `tower = { workspace = true, features = ["util"] }` under `[dev-dependencies]`; write every other current dev dependency as `{ workspace = true }`. Add:

```toml
school-permissions = { workspace = true }

[lints]
workspace = true
```

under the root dependency/lint sections. Do not add `default-members`; running plain Cargo commands from `backend-school` must continue to select the root package by default.

- [x] **Step 4: Create the permission crate and move the wrapper**

Create `backend-school/crates/school-permissions/Cargo.toml`:

```toml
[package]
name = "school-permissions"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }

[lints]
workspace = true
```

Create `backend-school/crates/school-permissions/src/lib.rs`:

```rust
pub mod registry;
```

Move the existing wrapper without changing `PermissionDef` fields or the include statement. Remove `mod permissions;` from `backend-school/src/main.rs` and delete `backend-school/src/permissions.rs` after all imports move.

- [x] **Step 5: Point the generator at the new owner and regenerate**

In `scripts/generate-permissions.mjs`, change only the default Rust output path to:

```javascript
path.join(
  repoRoot,
  'backend-school/crates/school-permissions/src/registry_generated.rs'
)
```

Run from the repository root:

```bash
node scripts/generate-permissions.mjs
node scripts/generate-permissions.mjs --check
node --test scripts/tests/generate-permissions.test.mjs
cmp backend-school/crates/school-permissions/src/registry_generated.rs \
  backend-school/src/permissions/registry_generated.rs
```

Expected: all commands PASS and `cmp` reports no difference. Delete the old generated file only after this comparison.

- [x] **Step 6: Move every root permission import directly to the crate**

Apply these exhaustive source transformations under `backend-school/src/`:

```text
crate::permissions::registry::...  -> school_permissions::registry::...
use crate::permissions::registry::codes; -> use school_permissions::registry::codes;
permissions::registry::codes inside use crate::{...} -> a separate use school_permissions::registry::codes;
```

Run:

```bash
rg -n 'crate::permissions::registry|permissions::registry::codes|mod permissions' \
  backend-school/src --glob '*.rs'
cargo fmt --all
cargo check -p backend-school --all-targets
```

Expected: `rg` returns no matches and Cargo PASSes. Do not add a root alias or re-export to make the check pass.

- [x] **Step 7: Run the permission contract and architecture tests**

Run:

```bash
cargo test -p school-permissions
cargo test --test static_architecture permission_registry_has_one_workspace_owner -- --exact
cargo test --test static_architecture permission_registry -- --nocapture
cd ../frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
```

Expected: PASS and `git diff --exit-code` reports no semantic changes for `contracts/permissions.lock.json` or `frontend-school/src/lib/permissions/registry.generated.ts`.

- [x] **Step 8: Commit the permission ownership change**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock \
  backend-school/crates/school-permissions backend-school/src \
  backend-school/tests/static_architecture.rs scripts/generate-permissions.mjs \
  frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "refactor: extract school permission registry"
```

## Task 3: Extract the Tenant Migration Runner

**Files:**

- Create: `backend-school/crates/school-migrations/Cargo.toml`
- Create: `backend-school/crates/school-migrations/build.rs`
- Create: `backend-school/crates/school-migrations/src/lib.rs`
- Create: `backend-school/src/db/migration_tests.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `backend-school/src/db.rs`
- Modify: `backend-school/src/db/pool_manager.rs`
- Modify: `backend-school/src/test_helpers.rs`
- Modify: `backend-school/src/middleware/permission.rs`
- Modify: `backend-school/src/modules/system/services/provision_service.rs`
- Modify: `backend-school/src/modules/certificates/schema_tests.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `backend-school/src/modules/academic/gradebook/services_tests.rs`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Modify: every additional Rust call site returned by `rg -l 'crate::db::migration|crate::utils::permission_sync|\bmigration::run_tenant_migrations' backend-school/src --glob '*.rs'`
- Modify: `backend-school/src/bin/migrate_tenant_schema.rs`
- Modify: `backend-school/src/bin/seed_sandbox.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Delete: `backend-school/src/db/migration.rs`
- Delete: `backend-school/src/utils/permission_sync.rs`

**Interfaces:**

- Consumes: `school_permissions::registry::ALL_PERMISSIONS`, `sqlx::PgPool`.
- Produces: `school_migrations::sync_permissions(&PgPool) -> Result<(), sqlx::Error>`.
- Produces: `school_migrations::run_tenant_migrations(&PgPool) -> Result<(), String>` with the existing `Migration failed: ...` and `Permission sync failed after migrations: ...` prefixes.
- Produces: cloneable `school_migrations::MigrationTracker` with `new()`, `run_migrations_once(&self, &str, &PgPool) -> Result<bool, String>`, `get_migrated_schools(&self) -> Vec<String>`, and `migration_count(&self) -> usize`.

- [x] **Step 1: Tighten the static test around shared migration ownership**

Replace `operational_bins_use_central_tenant_migration_runner` with assertions that scan every production Rust file and enforce the crate API:

```rust
#[test]
fn production_targets_use_the_workspace_migration_runner() {
    let mut path_modules = Vec::new();
    for file in backend_rs_files() {
        let source = strip_cfg_test_modules(&strip_comments(&read_source(&file)));
        if source.contains("#[path =") {
            path_modules.push(relative(&file));
        }
    }

    assert_eq!(path_modules, Vec::<String>::new());
    for binary in ["migrate_tenant_schema.rs", "seed_sandbox.rs"] {
        let source = read_source(manifest_dir().join("src/bin").join(binary));
        assert!(source.contains("school_migrations::run_tenant_migrations(&pool)"));
        assert!(!source.contains("pub mod migration"));
        assert!(!source.contains("permission_sync"));
    }
}
```

Add `school-migrations` to the expected workspace/path dependency assertions created in Task 2.

- [x] **Step 2: Run the focused architecture test and verify it fails**

Run:

```bash
cargo test --test static_architecture production_targets_use_the_workspace_migration_runner -- --exact
```

Expected: FAIL listing `migrate_tenant_schema.rs` and `seed_sandbox.rs` because they still contain production `#[path]` modules.

- [x] **Step 3: Add the migration crate manifest and build invalidation**

Add `"crates/school-migrations"` to `[workspace].members`, add this entry under `[workspace.dependencies]`, and add it to root `[dependencies]` with workspace inheritance:

```toml
school-migrations = { path = "crates/school-migrations" }
```

```toml
school-migrations = { workspace = true }
```

Create `backend-school/crates/school-migrations/Cargo.toml`:

```toml
[package]
name = "school-migrations"
version.workspace = true
edition.workspace = true

[dependencies]
dashmap = { workspace = true }
school-permissions = { workspace = true }
sqlx = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }

[lints]
workspace = true
```

Create `backend-school/crates/school-migrations/build.rs`:

```rust
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
```

- [x] **Step 4: Move the migration implementation and database-independent tests**

Build `backend-school/crates/school-migrations/src/lib.rs` from the current two owners. Preserve the bodies exactly except for these ownership changes:

```rust
use school_permissions::registry::ALL_PERMISSIONS;

fn all_migrations_without_db_lock() -> Migrator {
    let base = sqlx::migrate!("../../migrations");
    Migrator {
        migrations: Cow::Owned(base.iter().cloned().collect()),
        ignore_missing: base.ignore_missing,
        locking: false,
        no_tx: base.no_tx,
    }
}

pub async fn run_tenant_migrations(pool: &PgPool) -> Result<(), String> {
    all_migrations_without_db_lock()
        .run(pool)
        .await
        .map_err(|error| format!("Migration failed: {}", error))?;
    sync_permissions(pool)
        .await
        .map_err(|error| format!("Permission sync failed after migrations: {}", error))?;
    Ok(())
}
```

Keep the current `sync_permissions` body and `MigrationTracker` body unchanged. Move the three database-independent migration tests into the crate and change only the baseline include path:

```rust
let baseline_sql = include_str!("../../../migrations/001_baseline.sql");
```

Do not copy the two PostgreSQL permission-sync tests into this runtime crate; Task 3 Step 5 keeps them in the application test harness until `school-test-db` is introduced by checkpoint 2.

- [x] **Step 5: Preserve the database-backed permission-sync tests at the root**

Create `backend-school/src/db/migration_tests.rs` containing the current `fixture`, `batch_sync_preserves_grants_and_reconciles_in_one_insert_statement`, and `batch_sync_failure_rolls_back_deactivation_and_can_retry` tests. Change imports only:

```rust
use school_migrations::sync_permissions;
use school_permissions::registry::ALL_PERMISSIONS;
use sqlx::PgPool;

async fn fixture(name: &str) -> PgPool {
    let pool = crate::test_helpers::create_named_test_pool(name).await;
    // Preserve the current table/trigger fixture SQL verbatim here.
    pool
}
```

Declare it at the end of `backend-school/src/db.rs`:

```rust
#[cfg(test)]
mod migration_tests;
```

The fixture SQL is copied byte-for-byte from the deleted test module; its trigger makes the batching and transaction rollback behavior observable.

- [x] **Step 6: Move all application and test call sites to the public crate**

Apply these exact ownership transformations:

```text
use super::migration::MigrationTracker; -> use school_migrations::MigrationTracker;
crate::db::migration::run_tenant_migrations -> school_migrations::run_tenant_migrations
crate::utils::permission_sync::sync_permissions -> school_migrations::sync_permissions
migration::run_tenant_migrations in utility binaries -> school_migrations::run_tenant_migrations
```

Remove `pub mod migration;` from `backend-school/src/db.rs` and `pub mod permission_sync;` from `backend-school/src/utils.rs`. In both utility binaries delete the permission wrapper, permission sync, migration, and test-only root module `#[path]` blocks; keep their own normal unit tests and `seed_sandbox` academic cutover test modules because those are test fixtures rather than production implementation sharing.

Run:

```bash
rg -n 'crate::db::migration|crate::utils::permission_sync|pub mod permission_sync|pub mod migration|permissions::registry' \
  backend-school/src --glob '*.rs'
rg -n '#\[path =' backend-school/src/bin --glob '*.rs'
cargo fmt --all
cargo check --workspace --all-targets
```

Expected: the first search has no matches; the second search reports only the two `seed_sandbox` `#[cfg(test)]` academic fixture modules if they remain necessary for that binary's tests; the workspace check PASSes.

- [x] **Step 7: Run package, binary, and database-backed tests**

First add a runner test proving the disposable database can select the `seed_sandbox` binary through
`BACKEND_SCHOOL_TEST_BIN=seed_sandbox`, then change the runner's quoted `--bin` value from its
default `backend-school` to that environment value when present. Run:

```bash
cargo test -p school-migrations
cargo test --bin migrate_tenant_schema
cargo test --bin seed_sandbox -- \
  --skip tests::canonical_seed_is_idempotent_across_student_year_and_placement
cargo test --test static_architecture production_targets_use_the_workspace_migration_runner -- --exact
cd ..
node --test scripts/tests/backend-school-test-database.test.mjs
./scripts/test_backend_school.sh db::migration_tests -- --nocapture --test-threads=1
BACKEND_SCHOOL_TEST_BIN=seed_sandbox ./scripts/test_backend_school.sh \
  tests::canonical_seed_is_idempotent_across_student_year_and_placement \
  -- --exact --nocapture --test-threads=1
```

Expected: PASS. The PostgreSQL tests demonstrate a single batch insert, retained grants,
transactional rollback, retry, exact canonical registry cardinality, and idempotent sandbox seeding
against the complete active migration timeline.

- [x] **Step 8: Commit the migration ownership change**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock \
  backend-school/crates/school-migrations backend-school/src \
  backend-school/tests/static_architecture.rs scripts/test_backend_school.sh \
  scripts/tests/backend-school-test-database.test.mjs
git commit -m "refactor: extract tenant migration runner"
```

## Task 4: Enforce the Workspace Boundary and Update Project Rules

**Files:**

- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `.rules`
- Modify: `backend-school/README.md`
- Modify: `docs/TESTING.md`
- Modify: `.github/workflows/permission-contract.yml`
- Modify: `.github/workflows/api-contract.yml`
- Test: `frontend-school/tests/static/documentation-policy.test.mjs`

**Interfaces:**

- Consumes: the final two-member workspace graph from Tasks 2 and 3.
- Produces: durable admission rules, source inventory, dependency allowlist, canonical ownership guards, and CI commands for all workspace targets.

- [x] **Step 1: Write failing workspace architecture guards**

Expand `backend_rs_files()` to scan both `src/` and `crates/`:

```rust
fn backend_rs_files() -> Vec<PathBuf> {
    [manifest_dir().join("src"), manifest_dir().join("crates")]
        .into_iter()
        .flat_map(|root| {
            list_files(root, |path| {
                path.extension().is_some_and(|ext| ext == "rs")
                    && !is_rust_test_module(path)
            })
        })
        .collect()
}
```

Add a manifest guard that parses member manifests as text and enforces the exact graph:

```rust
#[test]
fn workspace_crates_follow_the_approved_dependency_graph() {
    let root = read_source(manifest_dir().join("Cargo.toml"));
    let permissions = read_source(
        workspace_crate_dir("school-permissions").join("Cargo.toml"),
    );
    let migrations = read_source(
        workspace_crate_dir("school-migrations").join("Cargo.toml"),
    );

    assert!(root.contains("members = ["));
    assert!(root.contains("\"crates/school-permissions\""));
    assert!(root.contains("\"crates/school-migrations\""));
    for manifest in [&permissions, &migrations] {
        assert!(manifest.contains("[lints]\nworkspace = true"));
        assert!(!manifest.contains("backend-school"));
    }
    assert!(!permissions.contains("school-migrations"));
    assert!(migrations.contains("school-permissions = { workspace = true }"));
}
```

Add a source guard that rejects `crate::AppState`, `backend_school::`, and production `#[path]` in `backend-school/crates/**/*.rs`; allow no exceptions in checkpoint 1.

- [x] **Step 2: Run the new static guards before documentation/CI changes**

Run:

```bash
cargo test --test static_architecture workspace_crates_follow_the_approved_dependency_graph -- --exact
cargo test --test static_architecture workspace_crates_do_not_depend_on_application_state_or_source_paths -- --exact
```

Expected: PASS for code ownership, while the broader documentation-policy test in Step 6 still fails until `.rules` and canonical docs describe workspace verification.

- [x] **Step 3: Add the crate-admission rule to `.rules`**

Add a `Backend-school Cargo workspace` subsection under the Rust backend standards with these normative points:

```text
- Evaluate crate placement when adding a capability or materially expanding an existing one.
- Create a crate only for a cohesive owner with a narrow public interface, acyclic intentionally directed dependencies, meaningful focused tests, and expected compile-scope value.
- Keep small same-owner/same-lifecycle code as a module; file count alone never justifies a crate.
- Prefer the smallest useful crate set; prohibit generic common/shared/utils crates and moving unrelated code downward to hide cycles.
- Keep router, full AppState, startup, OpenAPI composition, schedulers, and cross-domain adapters in the backend-school application package.
- Internal crates consume minimum state/typed ports, never the root package or full AppState.
- Centralize dependency versions and lints in backend-school/Cargo.toml and verify affected packages plus cargo check --workspace --all-targets.
- Require before/after compile evidence and preserve runtime, API, OpenAPI, permission, migration, security, test, and deployment guarantees.
```

Also change the permission constant owner to `backend-school/crates/school-permissions/src/registry.rs`, change the centralized migration runner owner to `school_migrations::run_tenant_migrations` in `backend-school/crates/school-migrations/src/lib.rs`, and change both backend verification occurrences from `cargo check` to:

```bash
cargo check --workspace --all-targets
```

- [x] **Step 4: Update canonical backend documentation**

In `backend-school/README.md`, add the workspace structure and focused commands:

```bash
cargo check --workspace --all-targets
cargo test -p school-permissions
cargo test -p school-migrations
```

State that plain `cargo run` still runs the root application, the application owns composition, and internal crates expose narrow APIs without `AppState`.

In `docs/TESTING.md`, replace generic backend checks in the baseline section with `cargo check --workspace --all-targets`, document `cargo test -p <package>`, and retain `./scripts/test_backend_school.sh` as the complete database-backed root binary suite.

- [x] **Step 5: Update permission and API workflow coverage**

In both `pull_request.paths` and `push.paths` of `.github/workflows/permission-contract.yml`, replace the old permission path with:

```yaml
- "backend-school/crates/school-permissions/**"
- "backend-school/crates/school-migrations/**"
- "backend-school/Cargo.toml"
- "backend-school/Cargo.lock"
```

Change its backend check to:

```yaml
run: cargo check --workspace --all-targets
```

In both path lists of `.github/workflows/api-contract.yml`, add:

```yaml
- "backend-school/crates/**"
```

Change both backend check steps in that workflow to `cargo check --workspace --all-targets`. Do not change job independence, cache ownership, toolchain version, or artifact generation.

- [x] **Step 6: Update and run the documentation-policy test**

In `frontend-school/tests/static/documentation-policy.test.mjs`, add assertions that `.rules`, `backend-school/README.md`, and `docs/TESTING.md` contain `cargo check --workspace --all-targets`, and that `.rules` contains both canonical crate paths.

Run:

```bash
cd frontend-school
node --test tests/static/documentation-policy.test.mjs
cd ..
podman run --rm -v "$PWD:/repo" -w /repo docker.io/rhysd/actionlint:1.7.7
```

Expected: PASS. If the actionlint image is unavailable because the network is unavailable, report that environmental block separately; do not weaken workflow validation.

- [x] **Step 7: Commit rules, guards, documentation, and CI**

```bash
git add .rules .github/workflows/api-contract.yml \
  .github/workflows/permission-contract.yml backend-school/README.md \
  backend-school/tests/static_architecture.rs docs/TESTING.md \
  frontend-school/tests/static/documentation-policy.test.mjs
git commit -m "chore: enforce backend school crate boundaries"
```

## Task 5: Measure the Split and Run the Full Checkpoint Matrix

**Files:**

- Verify only: `contracts/permissions.lock.json`
- Verify only: `contracts/openapi/school-api.json`
- Verify only: `frontend-school/src/lib/permissions/registry.generated.ts`
- Verify only: `backend-school/migrations/**`
- Do not create committed benchmark or completion-report files.

**Interfaces:**

- Consumes: completed two-crate workspace and Task 1 baseline medians.
- Produces: verified releasable checkpoint, compile-boundary evidence, and a clean working tree.

- [x] **Step 1: Measure the post-split no-change and local-package scenarios**

Run three warm checks:

```bash
measure_seconds 'split warm' env \
  CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --workspace --all-targets
```

Run this pair three times:

```bash
CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo clean -p backend-school -p school-migrations -p school-permissions
measure_seconds 'split local rebuild' env \
  CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --workspace --all-targets
```

Expected: PASS. Compare medians with Task 1; if the comparable local-package median is more than 10 percent slower, stop completion, inspect Cargo timing/package output, and correct the boundary or return to design review.

- [x] **Step 2: Prove migration-only invalidation direction over three runs**

For run numbers 1, 2, and 3, use `apply_patch` to add this exact temporary line immediately before
`fn all_migrations_without_db_lock()` in `crates/school-migrations/src/lib.rs`, substituting the
literal run number:

```rust
// Compile-boundary measurement probe 1; removed immediately after this check.
```

Run:

```bash
measure_seconds 'split migration change run=1' env \
  CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --workspace --all-targets -vv
```

After each run, remove the exact probe line before adding the next numbered probe. After run 3,
remove it and run:

```bash
git diff --exit-code -- crates/school-migrations/src/lib.rs
```

Expected: every run passes; the median is recorded; Cargo rebuilds `school-migrations` and its
actual dependents, reports `school-permissions` fresh, and compiles the migration implementation
once for reuse by the application and utility binaries.

- [x] **Step 3: Measure multi-target test compilation and Cargo timings**

Run the clean/test pair three times:

```bash
CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo clean -p backend-school -p school-migrations -p school-permissions
measure_seconds 'split test compile' env \
  CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo test --workspace --all-targets --no-run
```

Then run:

```bash
CARGO_TARGET_DIR=/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915 \
  cargo check --workspace --all-targets --timings
```

Expected: PASS; no timing output is written inside the repository.

- [x] **Step 4: Run Rust formatting, packages, architecture, and root binary tests**

Run from `backend-school`:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test -p school-permissions
cargo test -p school-migrations
cargo test --bin migrate_tenant_schema
cargo test --bin seed_sandbox -- \
  --skip tests::canonical_seed_is_idempotent_across_student_year_and_placement
cargo test --test static_architecture
```

Expected: every command PASSes with no warnings denied by workspace lints. The database-bound
`backend-school` binary suite runs through the disposable PostgreSQL runner in Step 5 so it has the
required `TEST_DATABASE_URL` instead of relying on ambient developer state.

- [x] **Step 5: Run the database-backed backend suite**

Run from the repository root:

```bash
./scripts/test_backend_school.sh
BACKEND_SCHOOL_TEST_BIN=seed_sandbox ./scripts/test_backend_school.sh \
  tests::canonical_seed_is_idempotent_across_student_year_and_placement \
  -- --exact --nocapture --test-threads=1
```

Expected: PASS against the disposable rootless-Podman PostgreSQL instance, including migration, schema, permission reconciliation, and all binary tests.

- [x] **Step 6: Run permission, API, frontend, and documentation checks**

Run from `frontend-school`:

```bash
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
npm run check:docs
```

Expected: every command PASSes and generation leaves tracked contract artifacts unchanged.

- [x] **Step 7: Build the production backend image and lint workflows**

Run from the repository root:

```bash
podman build -f backend-school/Dockerfile backend-school
podman run --rm -v "$PWD:/repo" -w /repo docker.io/rhysd/actionlint:1.7.7
```

Expected: the workspace-aware cargo-chef build succeeds, the runtime image still contains the `backend-school` executable and migrations, and actionlint PASSes.

- [x] **Step 8: Verify unchanged contracts, migrations, and final diff**

Run:

```bash
git diff origin/main -- contracts/permissions.json contracts/permissions.lock.json \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/permissions/registry.generated.ts \
  backend-school/migrations backend-school/migrations_legacy
git diff --check
git status --short
rg -n 'crate::permissions::registry|crate::db::migration|crate::utils::permission_sync' \
  backend-school --glob '*.rs'
rg -n '#\[path =' backend-school/src/bin --glob '*.rs'
```

Expected: no semantic contract or migration diff; no whitespace errors; the old owner searches are empty; only test-fixture `#[path]` entries may remain in `seed_sandbox`; the working tree is clean after committed documentation status updates.

## Completion Gate

Checkpoint 1 is complete only when all five tasks pass, all relevant commits are present, the old root owners are absent, the workspace dependency direction is enforced, contract/migration diffs are empty, the full verification matrix is green, and the compile median gate is satisfied. After this gate, create the focused checkpoint 2 implementation plan for `school-errors`, `school-http`, `school-authorization`, and dev-only `school-test-db` from the approved full-program specification before editing their source owners.
