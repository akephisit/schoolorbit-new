# Retire Rebaseline Toolchain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the completed one-time tenant rebaseline tooling and its documentation/test ownership while preserving the normal sequential migration and deployment gates.

**Architecture:** Delete only the three scripts whose contracts require exactly migration `001`. Remove their canonical documentation, `.rules`, backlog, and static-test references in the same coherent change. Keep the immutable active migration timeline, legacy audit history, runtime migration runner, and deployment migration verification unchanged.

**Tech Stack:** Bash, Rust static architecture tests, Node.js documentation-policy tests, Markdown.

## Global Constraints

- Never edit an applied migration.
- Keep `backend-school/migrations_legacy/` as audit history.
- Keep `backend-school/src/bin/migrate_tenant_schema.rs`, `/internal/migrate-all`, `/internal/migration-status`, and deployment migration gates.
- Do not add a static change detector for retired filenames; existing documentation-link, migration-timeline, and deployment guards own the remaining behavior.

---

### Task 1: Retire one-time rebaseline ownership

**Files:**
- Delete: `scripts/check_migration_rebaseline_ready.sh`
- Delete: `scripts/prepare_clean_tenant_db.sh`
- Delete: `scripts/cutover_tenant_data.sh`
- Modify: `.rules`
- Modify: `README.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/OPERATIONS.md`
- Modify: `TODO.md`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: the active sequential SQLx migration timeline and runtime deployment migration gate.
- Produces: canonical rules and documentation that describe only the ongoing migration lifecycle.

- [x] **Step 1: Verify the existing guards pass before retirement**

Run:

```bash
cd frontend-school
node --test tests/static/documentation-policy.test.mjs

cd ../backend-school
cargo test --test static_architecture active_migrations_are_clean_sequential_timeline
cargo test --test static_architecture tenant_data_cutover_script_has_safety_guards
cargo test --test static_architecture clean_tenant_prepare_script_has_safety_guards
```

Expected: all commands pass against the pre-cleanup tree.

- [x] **Step 2: Delete the one-time scripts and their obsolete static guards**

Delete the three Bash files. Remove `tenant_data_cutover_script_has_safety_guards` and `clean_tenant_prepare_script_has_safety_guards` from `backend-school/tests/static_architecture.rs`. Do not change `active_migrations_are_clean_sequential_timeline`.

- [x] **Step 3: Update durable rules, documentation, and backlog**

Remove the two rebaseline-script bullets from `.rules` while retaining baseline immutability, sequential forward migrations, legacy audit history, and isolated test database requirements. Update the root README's Operations summary from tenant cutover to tenant migrations and recovery. Remove the read-only rebaseline commands from `docs/TESTING.md`; replace the cutover-script procedure in `docs/OPERATIONS.md` with the normal migration lifecycle owned by provisioning and deployment. Remove only the completed readiness-script bullet from `OPS-002` in `TODO.md`.

- [x] **Step 4: Run focused verification**

Run:

```bash
cd frontend-school
node --test tests/static/documentation-policy.test.mjs
node --test tests/static/deployment-installer.test.mjs

cd ../backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

Expected: every command exits zero and no test references a deleted script.

- [x] **Step 5: Run repository final checks**

Run:

```bash
git diff --check
git diff --stat
git diff -- .rules README.md docs/TESTING.md docs/OPERATIONS.md TODO.md backend-school/tests/static_architecture.rs
git status --short
```

Expected: only the planned documentation, static test, workflow-plan, and three script deletions appear; no migration, runtime runner, or deployment workflow changes appear.
