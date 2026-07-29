# Legacy Operations Script Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove retired VPS hotfix and compatibility scripts without weakening the current deployment, rollback, migration, discovery, setup, or smoke-test paths.

**Architecture:** The Backend School workflow will consume the already-cut-over runtime environment directly and remove only the exact stale File Platform upgrader path on the VPS. Obsolete standalone utilities will be deleted rather than archived, while current tracked configuration and operational tools remain the source of truth.

**Tech Stack:** GitHub Actions YAML, Bash, Rust static architecture tests, Podman deployment, Nginx, Cloudflare R2.

## Global Constraints

- Do not add a replacement preflight for the new R2 settings.
- Do not edit any applied migration.
- Do not retain or log plaintext national IDs in the retired utilities.
- Delete only the exact stale VPS upgrader path; never remove a runtime directory or use a broad glob.
- Keep Nginx test/reload/rollback, application readiness, bucket/scanner checks, and current operational scripts intact.
- Execute inline in the current workspace; do not dispatch subagents.

---

### Task 1: Retire the File Platform environment upgrader

**Files:**

- Modify: `.github/workflows/deploy-backend-school.yml`
- Delete: `backend-school/scripts/upgrade_file_platform_env.sh`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `docs/OPERATIONS.md`
- Modify: `docs/superpowers/specs/2026-07-30-legacy-operations-script-cleanup-design.md`

**Interfaces:**

- Consumes: the existing `/opt/stack/.env` values and Backend School readiness flow.
- Produces: a deployment workflow with no legacy R2 conversion and an exact stale-file cleanup at `/opt/stack/file-platform-runtime/scripts/upgrade_file_platform_env.sh`.

- [ ] **Step 1: Run the focused legacy-reference audit and verify it fails**

Run:

```bash
if rg -n \
  'source:.*upgrade_file_platform_env\.sh|bash .*upgrade_file_platform_env\.sh' \
  .github/workflows/deploy-backend-school.yml ||
  rg -n 'upgrade_file_platform_env\.sh' \
    backend-school/tests/static_architecture.rs \
    docs/OPERATIONS.md ||
  rg -n 'R2_BUCKET_NAME' \
    .github/workflows/deploy-backend-school.yml \
    backend-school/tests/static_architecture.rs
then
  exit 1
fi
```

Expected: exit 1 with the current workflow upload/call, compatibility test, and rollout documentation.

- [ ] **Step 2: Make the minimal workflow cleanup**

Change the SCP source list from:

```yaml
source: backend-school/docker-compose.yml,backend-school/scripts/upgrade_file_platform_env.sh,nginx-configs/school-api.schoolorbit.app.conf,nginx-configs/school-api.schoolorbit.app.maintenance.conf
```

to:

```yaml
source: backend-school/docker-compose.yml,nginx-configs/school-api.schoolorbit.app.conf,nginx-configs/school-api.schoolorbit.app.maintenance.conf
```

Remove the command that executes the upgrader and the existing
`required_name` completeness loop. In their place, remove only the retired
staged copy:

```bash
rm -f /opt/stack/file-platform-runtime/scripts/upgrade_file_platform_env.sh
```

Do not add a loop or preflight for the R2 configuration values.

- [ ] **Step 3: Delete the compatibility implementation and its obsolete test**

Delete `backend-school/scripts/upgrade_file_platform_env.sh`.

Delete only the Rust test function:

```rust
#[test]
fn file_platform_runtime_config_upgrade_is_safe_and_idempotent()
```

including its temporary-file setup and assertions. Do not change neighboring architecture tests.

- [ ] **Step 4: Remove the completed-rollout documentation**

Delete the `docs/OPERATIONS.md` paragraph beginning:

```text
For the first production rollout, `upgrade_file_platform_env.sh` performs...
```

Keep the durable requirement that `R2_BUCKET_NAME` is not a runtime fallback.

- [ ] **Step 5: Re-run the focused audit**

Run the Step 1 command again.

Expected: exit 0 with no matches in the workflow, architecture tests, or rollout documentation.

### Task 2: Delete superseded and unsafe standalone utilities

**Files:**

- Delete: `nginx-configs/quick-fix.sh`
- Delete: `backend-admin/scripts/create_prod_admin.sh`
- Delete: `backend-admin/scripts/deploy_tenant.sh`
- Delete: `backend-admin/scripts/diagnose_login.sh`
- Delete: `backend-admin/scripts/verify_admin.sh`
- Delete: `backend-school/scripts/set_encryption_role.sh`

**Interfaces:**

- Consumes: current GitHub deployment workflows, tracked Nginx configuration, container environment secrets, and Deploy All tenant discovery.
- Produces: a repository where retired host-Nginx, bootstrap-login, manual tenant deployment, and database-role encryption paths cannot be invoked accidentally.

- [ ] **Step 1: Verify all retirement targets currently exist**

Run:

```bash
for path in \
  nginx-configs/quick-fix.sh \
  backend-admin/scripts/create_prod_admin.sh \
  backend-admin/scripts/deploy_tenant.sh \
  backend-admin/scripts/diagnose_login.sh \
  backend-admin/scripts/verify_admin.sh \
  backend-school/scripts/set_encryption_role.sh
do
  test -f "$path"
done
```

Expected: exit 0, proving the audit targets the current tree.

- [ ] **Step 2: Delete the six exact files**

Use file-scoped deletions only. Do not remove either scripts directory because
current setup and other operational utilities remain there.

- [ ] **Step 3: Verify the retired names have no live references**

Run:

```bash
if rg -n \
  'quick-fix\.sh|create_prod_admin\.sh|deploy_tenant\.sh|diagnose_login\.sh|verify_admin\.sh|set_encryption_role\.sh' \
  --glob '!docs/superpowers/specs/2026-07-30-legacy-operations-script-cleanup-design.md' \
  --glob '!docs/superpowers/plans/2026-07-30-legacy-operations-script-cleanup.md' \
  .
then
  exit 1
fi
```

Expected: exit 0.

### Task 3: Verify repository and runtime boundaries

**Files:**

- Verify: `.github/workflows/deploy-backend-school.yml`
- Verify: `backend-school/tests/static_architecture.rs`
- Verify: remaining shell scripts and Rust crates

**Interfaces:**

- Consumes: the changes from Tasks 1 and 2.
- Produces: verification evidence for syntax, architecture, compilation, public readiness, and a clean diff.

- [ ] **Step 1: Check shell syntax for all remaining tracked shell scripts**

Run:

```bash
while IFS= read -r -d '' path; do
  if [[ -f "$path" ]]; then
    bash -n "$path"
  fi
done < <(git ls-files '*.sh' -z)
```

Expected: exit 0.

- [ ] **Step 2: Check touched workflow and documentation formatting**

Run from `frontend-school`:

```bash
npx prettier --check \
  ../.github/workflows/deploy-backend-school.yml \
  ../docs/OPERATIONS.md \
  ../docs/superpowers/specs/2026-07-30-legacy-operations-script-cleanup-design.md \
  ../docs/superpowers/plans/2026-07-30-legacy-operations-script-cleanup.md
```

Expected: exit 0.

- [ ] **Step 3: Run the Backend School verification matrix**

Run from `backend-school`:

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

Expected: all commands exit 0 and the static architecture suite reports zero failures.

- [ ] **Step 4: Run the Backend Admin verification matrix**

Run from `backend-admin`:

```bash
cargo fmt --all -- --check
cargo check
```

Expected: both commands exit 0.

- [ ] **Step 5: Run the public deployment smoke test**

Run from the repository root without embedding credentials:

```bash
./scripts/smoke_test.sh
```

Expected: public tenant, Admin API, School API, readiness, CORS, and unauthenticated validation checks pass; authenticated checks may report `SKIP` when local smoke credentials are absent.

- [ ] **Step 6: Review the final diff and repository state**

Run:

```bash
git diff --check
git diff --stat
git diff -- .github/workflows/deploy-backend-school.yml docs/OPERATIONS.md
git status --short
```

Expected: no whitespace errors; only the approved cleanup, plan/spec update, and exact file deletions appear.

- [ ] **Step 7: Commit the implementation**

```bash
git add \
  .github/workflows/deploy-backend-school.yml \
  backend-admin/scripts/create_prod_admin.sh \
  backend-admin/scripts/deploy_tenant.sh \
  backend-admin/scripts/diagnose_login.sh \
  backend-admin/scripts/verify_admin.sh \
  backend-school/scripts/set_encryption_role.sh \
  backend-school/scripts/upgrade_file_platform_env.sh \
  backend-school/tests/static_architecture.rs \
  docs/OPERATIONS.md \
  docs/superpowers/specs/2026-07-30-legacy-operations-script-cleanup-design.md \
  docs/superpowers/plans/2026-07-30-legacy-operations-script-cleanup.md \
  nginx-configs/quick-fix.sh
git commit -m "chore: remove legacy operations scripts"
```

Expected: a local implementation commit. Do not push until remote publication is explicitly confirmed.
