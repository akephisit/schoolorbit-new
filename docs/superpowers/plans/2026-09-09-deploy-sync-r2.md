# Deployment sync and R2 optimization

> Execute with executing-plans inline, strictly one task at a time. No subagents, commits, pushes, or production mutations.

**Goal:** Reduce redundant deployment database and R2 requests without weakening acceptance gates.

**Approved design:** User approved batch permission reconciliation once per tenant startup, then read/compare/write/verify R2 CORS. Existing permission grants, cache invalidation, migration checks, bucket validation and smoke gates remain intact.

## 1. Permission reconciliation

Files: `backend-school/src/utils/permission_sync.rs`, `backend-school/src/db/migration.rs`, `backend-school/src/db/pool_manager.rs` and their tests.

- [x] Baseline: run existing `db::` release tests using the disposable PostgreSQL runner.
- [x] Add database tests with statement-level triggers to count actual upsert statements, retain a foreign-key grant to a retired permission, restore changed canonical metadata, and force a write failure to verify rollback. Observe failure against the current row-at-a-time implementation.
- [x] Use `sqlx::QueryBuilder<Postgres>::push_values(ALL_PERMISSIONS, ...)` and one upsert inside the same transaction as deactivation. Bind every field; retain the empty-registry no-op guard.
- [x] Keep `run_tenant_migrations` as the single migration-plus-sync operation. Remove the redundant permission tracker and second pool-manager sync. Preserve the successful-operation boolean used for cache invalidation.
- [x] Test the real pool-manager path with concurrent callers, repeat calls and a failed first sync followed by retry; assert one successful reconciliation, no falsely completed tracker, and the existing change notification signal.
- [x] Run focused release tests, formatting, architecture and Cargo checks before starting R2.

## 2. R2 reconciliation

Files: `.github/workflows/deploy-backend-school.yml`, new `scripts/reconcile_r2_cors.sh`, behavioral tests under `scripts/tests/`, existing deployment guards, `docs/OPERATIONS.md` and `docs/TESTING.md`.

- [x] Add a shell behavior test harness with a controlled AWS boundary: matching/reordered policies require one read and no writes; differing/missing policies require read/write/read; failed reads except an explicit missing-CORS response, failed writes, invalid JSON and verification mismatch fail closed.
- [x] Extract only CORS reconciliation into a sourced helper. Read JSON once, normalize set-like array order, compare the entire policy, write only on drift, and verify the complete policy after writing. Never print credentials or raw errors.
- [x] Keep all bucket accessibility checks and private bucket creation behavior. Stage/source the helper in the existing workflow and include its path in workflow triggers. Add a bounded R2 timing record. Wire shell lint and behavioral tests into `.github/workflows/installer.yml` for subsequent changes too.
- [ ] Run behavioral tests and the complete deployment verification matrix from `.rules` (shellcheck, shfmt, installer Bats, static guard, compose dry run, actionlint). Run frontend lint/check/static if its test is changed.

All listed local checks except the compose dry run passed; no frontend source/test was changed.
The dry run invokes `podman ps` even in dry-run mode. Locally extracted tools reached
the missing `newuidmap` requirement; passwordless sudo is unavailable. Run the unchanged
topology gate in Installer Verification CI or a fully configured Podman environment before integration.

## 3. Final verification

- [x] Run release regression suite, applicable permission checks, documentation guards, and review the final diff and worktree status. Report local verification separately from unperformed production timing/smoke acceptance.
