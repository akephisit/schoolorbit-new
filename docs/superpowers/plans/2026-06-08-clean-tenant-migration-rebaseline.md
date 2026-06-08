# Clean Tenant Migration Rebaseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace backend-school active migrations with one clean baseline migration for new tenant databases.

**Architecture:** Archive legacy migrations for audit only, make `backend-school/migrations/001_baseline.sql` the only active migration, and simplify the tenant migration runner to the normal SQLx migrator plus permission sync. Existing tenant databases with old `_sqlx_migrations` history must not be used with this branch until they are migrated/copied to a clean database.

**Tech Stack:** Rust, Axum, SQLx migrations, PostgreSQL, Bash verification scripts.

---

### Task 1: Active Migration Cutover

**Files:**
- Move: `backend-school/migrations/*.sql` to `backend-school/migrations_legacy/`
- Create: `backend-school/migrations/001_baseline.sql`
- Remove: `backend-school/baseline/127_baseline.sql`

- [x] **Step 1: Archive legacy migration files**

Run:

```bash
mkdir -p backend-school/migrations_legacy
git mv backend-school/migrations/*.sql backend-school/migrations_legacy/
```

Expected: `backend-school/migrations_legacy/` contains old migration versions 1-127.

- [x] **Step 2: Install the clean baseline as active migration**

Run:

```bash
mkdir -p backend-school/migrations
cp backend-school/baseline/127_baseline.sql backend-school/migrations/001_baseline.sql
rm -rf backend-school/baseline
```

Expected: `backend-school/migrations/` contains only `001_baseline.sql`.

### Task 2: Simplify Migration Runner

**Files:**
- Modify: `backend-school/src/db/migration.rs`
- Modify: `backend-school/src/bin/migrate_tenant_schema.rs`
- Modify: `backend-school/src/bin/seed_sandbox.rs`

- [x] **Step 1: Remove legacy checkpoint code**

Delete baseline fast-path/checkpoint helpers and make `run_tenant_migrations()` call `all_migrations_without_db_lock().run(pool).await` then `permission_sync::sync_permissions(pool).await`.

- [x] **Step 2: Keep operational bins on the central runner**

Keep `seed_sandbox` and `migrate_tenant_schema` using `migration::run_tenant_migrations(&pool)` so tests and sandbox provisioning use the same path.

### Task 3: Static Guards And Docs

**Files:**
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `.rules`
- Modify: `docs/TESTING.md`
- Modify: `IMPROVEMENT_PLAN.md`

- [x] **Step 1: Update static guards**

Add/adjust guards so active migrations contain exactly one `001_baseline.sql`, legacy migrations are not under the active migrator, and the baseline contains canonical organization/permission contracts.

- [x] **Step 2: Update docs**

Document that this branch is clean-only: old tenant DBs must be copied/cut over before deployment points at them.

### Task 4: Verification

- [x] **Step 1: Format and compile**

Run:

```bash
cd backend-school
cargo fmt --check
cargo check --bins
```

- [x] **Step 2: Run focused tests**

Run:

```bash
cd backend-school
cargo test db::migration::tests --bin backend-school
cargo test --test static_architecture
```

- [x] **Step 3: Runtime schema smoke**

Run `migrate_tenant_schema` against a temporary schema in `schoolorbit_test`, verify `_sqlx_migrations` has one row and max version 1, then drop the schema.
