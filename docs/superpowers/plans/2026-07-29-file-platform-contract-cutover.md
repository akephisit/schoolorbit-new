# File Platform Contract Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the path-based File Platform compatibility schema with the final provider-neutral schema in one guarded migration and move all backend-school consumers to it.

**Architecture:** Migration `032` performs a transactional preflight, renames logical identity columns, and drops duplicated locator/metadata columns. The same backend image uses only final-schema names; deployment places school-api in maintenance, migrates all active tenants, and never restores a pre-cutover image after migration begins.

**Tech Stack:** Rust, Axum, SQLx 0.8, PostgreSQL, Bash, Nginx, GitHub Actions, Podman

## Global Constraints

- Never edit migrations `001` through `031`; add only `032_file_platform_contract_cutover.sql`.
- Do not modify `backend-admin` or `frontend-admin`.
- Do not change HTTP routes, JSON envelopes, permissions, or generated API contracts.
- Do not log database URLs, credentials, signed URLs, object keys, filenames, user IDs, or provider responses during migration/deployment audit.
- No dual-write, compatibility trigger, or pre-cutover binary support remains after migration `032`.
- Once any tenant applies `032`, recovery is fix-forward with the cutover image or newer.
- Use `TEST_DATABASE_URL` from the ignored backend-school environment without printing it.

---

### Task 1: Lock the final schema contract with failing database tests

**Files:**
- Modify: `backend-school/src/test_helpers.rs`
- Modify: `backend-school/src/modules/files/schema_tests.rs`

**Interfaces:**
- Consumes: existing `create_test_pool()` and SQLx migrator behavior
- Produces: `create_named_test_pool(test_name: &str) -> PgPool` for isolated destructive migration scenarios, plus schema tests for migration success and fail-closed rollback

- [ ] **Step 1: Add an isolated test-pool helper**

Add a test-only helper that sanitizes the supplied test name, includes the
process ID in the schema name, drops and recreates that exact schema, and
returns a pool whose search path is `<isolated>, public`. It must reuse
`direct_test_database_url` and never print the URL.

- [ ] **Step 2: Add a migration-through-031 helper in schema tests**

Construct a `sqlx::migrate::Migrator` from `sqlx::migrate!("./migrations")`
with only versions `<= 31`, and run it against a named isolated pool. Read
`032_file_platform_contract_cutover.sql` at runtime and apply it with
`sqlx::raw_sql`.

- [ ] **Step 3: Write the successful cutover test**

Create a pre-cutover file row with `purpose_code`, a matching version, and a
ready `current_version_id`. Apply the migration and assert:

```text
files contains owner_user_id, display_filename, created_by
files does not contain user_id, filename, uploaded_by, storage_path,
file_size, mime_type, is_temporary, or is_public
active_files and generate_storage_path do not exist
the file ID and current-version relationship are unchanged
purpose_code is NOT NULL
```

- [ ] **Step 4: Write independent fail-closed tests**

Use a fresh named schema for each case and assert migration `032` returns an
error while `files.storage_path` still exists:

```text
a file has no file_versions row
a ready file lacks a valid current version
a nonblank users.profile_image_url lacks profile_image_file_id
a nonblank staff_achievements.image_path lacks image_file_id
```

Each fixture uses generated UUIDs and inert test strings; no production
locator, filename, or user identifier appears in output.

- [ ] **Step 5: Run tests and verify RED**

Run:

```bash
cd backend-school
cargo test modules::files::schema_tests --bin backend-school
```

Expected: FAIL because migration `032` is absent and final-schema assertions
cannot pass.

- [ ] **Step 6: Commit the RED tests**

```bash
git add backend-school/src/test_helpers.rs backend-school/src/modules/files/schema_tests.rs
git commit -m "test: define file platform contract cutover"
```

### Task 2: Implement the guarded forward-only migration

**Files:**
- Create: `backend-school/migrations/032_file_platform_contract_cutover.sql`
- Modify: `backend-school/src/modules/files/schema_tests.rs`

**Interfaces:**
- Consumes: pre-cutover tables from migrations `001`, `030`, and `031`
- Produces: final `files(owner_user_id, display_filename, created_by, purpose_code, visibility, lifecycle_status, current_version_id, retention_class, expires_at, delete_requested_at, created_at, updated_at, deleted_at)` schema

- [ ] **Step 1: Add fixed-message preflight guards**

Use a `DO $$ ... $$` block with `IF EXISTS` checks. Raise fixed exceptions for:

```sql
filename IS NULL OR btrim(filename) = '' OR purpose_code IS NULL
NOT EXISTS (SELECT 1 FROM file_versions v WHERE v.file_id = f.id)
lifecycle_status = 'ready' without a matching (current_version_id, file_id)
nonblank profile_image_url without profile_image_file_id
nonblank image_path without image_file_id
```

- [ ] **Step 2: Rename logical columns and owner semantics**

Execute:

```sql
DROP VIEW active_files;
DROP FUNCTION generate_storage_path(VARCHAR, VARCHAR, UUID, VARCHAR);
ALTER TABLE files DROP CONSTRAINT files_user_id_fkey;
ALTER TABLE files RENAME COLUMN user_id TO owner_user_id;
ALTER TABLE files RENAME COLUMN filename TO display_filename;
ALTER TABLE files RENAME COLUMN uploaded_by TO created_by;
ALTER TABLE files
    ADD CONSTRAINT files_owner_user_id_fkey
    FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE SET NULL;
```

Rename `idx_files_user_id` to `idx_files_owner_user_id` and
`files_uploaded_by_fkey` to `files_created_by_fkey`.

- [ ] **Step 3: Tighten canonical fields and indexes**

Set `purpose_code` NOT NULL, add
`files_display_filename_nonblank_check`, drop `idx_files_temp_expires`, and
create:

```sql
CREATE INDEX idx_files_temporary_expires
ON files (expires_at)
WHERE retention_class = 'temporary' AND deleted_at IS NULL;
```

- [ ] **Step 4: Drop path-based and duplicated fields**

Drop `users.profile_image_url`, `staff_achievements.image_path`, and these
`files` columns:

```text
school_id, original_filename, file_size, mime_type, storage_path, file_type,
width, height, has_thumbnail, thumbnail_path, is_temporary, is_public, checksum
```

- [ ] **Step 5: Run migration tests and verify GREEN**

Run:

```bash
cd backend-school
cargo test modules::files::schema_tests --bin backend-school
```

Expected: the new cutover/guard tests pass. Existing fixtures that insert
legacy columns may still fail and are updated in Task 3.

- [ ] **Step 6: Commit the migration**

```bash
git add backend-school/migrations/032_file_platform_contract_cutover.sql backend-school/src/modules/files/schema_tests.rs
git commit -m "feat: contract file platform schema"
```

### Task 3: Move the File Platform repository to the final schema

**Files:**
- Modify: `backend-school/src/modules/files/repository.rs`
- Modify: `backend-school/src/modules/files/platform_service.rs`
- Modify: `backend-school/src/modules/files/schema_tests.rs`

**Interfaces:**
- Consumes: migration `032` final schema
- Produces: repository writes logical metadata only to `files` and reads immutable metadata from `file_versions`

- [ ] **Step 1: Run the repository integration test and verify RED**

Run:

```bash
cd backend-school
cargo test modules::files::repository::tests::sql_repository_reserves_reclaims_finalizes_and_deletes_durably --bin backend-school
```

Expected: FAIL because `reserve_upload` references columns removed by `032`.

- [ ] **Step 2: Replace the logical file insert**

Change `reserve_upload` to insert only:

```sql
INSERT INTO files (
    id, owner_user_id, display_filename, created_by, purpose_code, visibility,
    lifecycle_status, retention_class, expires_at
) VALUES (
    $1, $2, $3, $4, $5, $6, 'processing', $7,
    CASE WHEN $7 = 'temporary' THEN now() + INTERVAL '24 hours' ELSE NULL END
)
```

Keep object key, detected MIME type, byte size, checksum, and creator in
`file_versions`; keep derivative object metadata in `file_derivatives`.

- [ ] **Step 3: Replace delivery reads**

Select:

```text
f.owner_user_id, f.display_filename,
v.byte_size, v.detected_mime_type, v.object_key, v.storage_class
```

Map `PlatformFile.byte_size` from `file_versions.byte_size` and remove
`legacy_file_type`.

- [ ] **Step 4: Remove unused persisted dimensions**

Remove `width` and `height` from `NewUpload` and its construction in
`platform_service.rs`. Keep dimension inspection and upload limit validation
unchanged.

- [ ] **Step 5: Update final-schema test fixtures**

Change helper file inserts in `schema_tests.rs` to use
`display_filename`, `purpose_code`, `visibility`, `lifecycle_status`, and
`retention_class`.

- [ ] **Step 6: Run focused tests and verify GREEN**

Run:

```bash
cd backend-school
cargo test modules::files::repository::tests --bin backend-school
cargo test modules::files::platform_service::tests --bin backend-school
cargo test modules::files::schema_tests --bin backend-school
```

- [ ] **Step 7: Commit repository cutover**

```bash
git add backend-school/src/modules/files/repository.rs backend-school/src/modules/files/platform_service.rs backend-school/src/modules/files/schema_tests.rs
git commit -m "refactor: use canonical file platform schema"
```

### Task 4: Move business consumers to canonical file fields

**Files:**
- Modify: `backend-school/src/modules/auth/services.rs`
- Modify: `backend-school/src/modules/school/services.rs`
- Modify: `backend-school/src/modules/achievement/services.rs`
- Modify: `backend-school/src/modules/admission/services/application_service.rs`
- Modify: `backend-school/src/modules/admission/services/portal_service.rs`
- Modify: `backend-school/src/modules/question_bank/services.rs`
- Modify: `backend-school/src/modules/staff/services/staff_service.rs`
- Modify: `backend-school/src/services/cleaner.rs`

**Interfaces:**
- Consumes: `files.owner_user_id`, `files.display_filename`, `files.retention_class`, and current `file_versions`
- Produces: no runtime SQL dependency on removed compatibility columns

- [ ] **Step 1: Run affected focused tests and verify RED**

Run:

```bash
cd backend-school
cargo test modules::auth::services::tests --bin backend-school
cargo test modules::question_bank::services::tests --bin backend-school
cargo test modules::files --bin backend-school
```

Expected: FAIL where test fixtures or queries still reference removed columns.

- [ ] **Step 2: Replace temporary-file finalization**

In auth, school, achievement, admission, and staff services replace:

```sql
SET is_temporary = false, retention_class = 'standard', expires_at = NULL
```

with:

```sql
SET retention_class = 'standard', expires_at = NULL
```

- [ ] **Step 3: Replace question-bank ownership and retention**

Rename `PayloadFileRow.user_id` to `owner_user_id` and
`is_temporary: bool` to `retention_class: String`. Select those final columns,
consider a file temporary only when `retention_class = 'temporary'`, and
finalize with:

```sql
UPDATE files
SET retention_class = 'standard', expires_at = NULL, updated_at = NOW()
WHERE id = ANY($1)
  AND owner_user_id = $2
  AND retention_class = 'temporary'
  AND deleted_at IS NULL
```

- [ ] **Step 4: Replace admission document metadata reads**

Join:

```sql
JOIN files f ON f.id = d.file_id
JOIN file_versions v
  ON v.id = f.current_version_id AND v.file_id = f.id
```

Project `f.display_filename AS original_filename`,
`v.byte_size AS file_size`, and
`v.detected_mime_type AS mime_type` to preserve existing DTOs.

- [ ] **Step 5: Replace expiry cleanup**

Filter:

```sql
WHERE retention_class = 'temporary'
  AND expires_at <= now()
  AND deleted_at IS NULL
  AND lifecycle_status <> 'deleted'
```

- [ ] **Step 6: Update auth test fixtures**

Insert final `files` columns and a matching `file_versions` row, set
`current_version_id`, then assert `retention_class` and `expires_at` only.

- [ ] **Step 7: Run affected tests and verify GREEN**

Run:

```bash
cd backend-school
cargo test modules::auth::services::tests --bin backend-school
cargo test modules::question_bank::services::tests --bin backend-school
cargo test modules::files --bin backend-school
cargo check
```

- [ ] **Step 8: Search for remaining runtime dependencies**

Run:

```bash
rg -n '\b(user_id|filename|uploaded_by|storage_path|original_filename|file_size|mime_type|file_type|is_temporary|is_public|checksum|thumbnail_path|has_thumbnail)\b' \
  backend-school/src/modules/files \
  backend-school/src/modules/auth/services.rs \
  backend-school/src/modules/school/services.rs \
  backend-school/src/modules/achievement/services.rs \
  backend-school/src/modules/admission/services \
  backend-school/src/modules/question_bank/services.rs \
  backend-school/src/modules/staff/services/staff_service.rs \
  backend-school/src/services/cleaner.rs
```

Review each match. DTO field names such as `original_filename`, `file_size`,
and `mime_type` may remain only when populated from canonical SQL aliases.
No SQL may reference a removed database column.

- [ ] **Step 9: Commit consumer cutover**

```bash
git add backend-school/src/modules/auth/services.rs backend-school/src/modules/school/services.rs backend-school/src/modules/achievement/services.rs backend-school/src/modules/admission/services/application_service.rs backend-school/src/modules/admission/services/portal_service.rs backend-school/src/modules/question_bank/services.rs backend-school/src/modules/staff/services/staff_service.rs backend-school/src/services/cleaner.rs
git commit -m "refactor: remove file compatibility consumers"
```

### Task 5: Make the deployment a no-legacy maintenance cutover

**Files:**
- Create: `nginx-configs/school-api.schoolorbit.app.maintenance.conf`
- Modify: `.github/workflows/deploy-backend-school.yml`
- Modify: `docs/OPERATIONS.md`

**Interfaces:**
- Consumes: backend internal `POST /internal/migrate-all`, `/ready`, runtime `INTERNAL_API_SECRET`, and Nginx school-api configuration
- Produces: tenant traffic is blocked while migration `032` is applied; pre-cutover image rollback occurs only before migration starts

- [ ] **Step 1: Add a school-api-only maintenance configuration**

Mirror the production server names/TLS settings but return HTTP `503` with
`Retry-After: 60` for school-api traffic. Do not modify admin proxy
configuration.

- [ ] **Step 2: Upload the maintenance configuration in deployment**

Include the new file in the SCP source list and resolve exact source/target
paths alongside the normal school-api config.

- [ ] **Step 3: Enter maintenance before replacing the backend**

Back up the active school-api config to a validated `mktemp` path, install the
maintenance config, run `nginx -t`, and reload. If this fails, restore the
backup and exit before changing the backend image.

- [ ] **Step 4: Preserve safe pre-migration rollback**

Start the cutover image and wait for `/ready`. If readiness fails before
`migrate-all` starts, restore the previous image and normal proxy because no
tenant request can have applied `032`.

- [ ] **Step 5: Migrate every active tenant without exposing secrets**

Capture `INTERNAL_API_SECRET` from the running container environment without
printing it. Call:

```text
POST http://127.0.0.1:8081/internal/migrate-all
X-Internal-Secret: <captured in memory>
```

Write the response to a restrictive temporary file, parse with `jq`, and
require `failed == 0`, `success == total`, and `latest_version == 32`. Remove
the temporary file on every exit path. If migration fails, leave maintenance
enabled and keep the cutover image running; never retag the old image.

- [ ] **Step 6: Restore traffic and set the rollback floor**

Install the normal tracked proxy config, validate/reload Nginx, and only then
tag the successfully migrated cutover image as both `latest` and `rollback`.
Any later rollback therefore remains schema-compatible.

- [ ] **Step 7: Update operations guidance**

Replace additive migration-031 rollback language with the migration-032
maintenance procedure, the cutover commit/image floor, and fix-forward
recovery. Do not copy secrets or one-off production values into documentation.

- [ ] **Step 8: Validate workflow/config and commit**

Run:

```bash
git diff --check
rg -n 'migration 032|fix-forward|maintenance' docs/OPERATIONS.md .github/workflows/deploy-backend-school.yml
```

Then commit:

```bash
git add nginx-configs/school-api.schoolorbit.app.maintenance.conf .github/workflows/deploy-backend-school.yml docs/OPERATIONS.md
git commit -m "ci: guard file platform schema cutover"
```

### Task 6: Full verification, deployment, and backlog closure

**Files:**
- Modify after successful production verification: `TODO.md`
- Delete after completed outcome: `docs/superpowers/specs/2026-07-29-file-platform-contract-cleanup-design.md`
- Delete after completed outcome: `docs/superpowers/plans/2026-07-29-file-platform-contract-cutover.md`

**Interfaces:**
- Consumes: all earlier tasks and production deployment evidence
- Produces: verified migration `032` across active tenants and closed DB-004

- [ ] **Step 1: Run the backend verification matrix**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test modules::files::schema_tests --bin backend-school
cargo test modules::files::repository::tests --bin backend-school
cargo test modules::auth::services::tests --bin backend-school
cargo test modules::question_bank::services::tests --bin backend-school
cargo test --test static_architecture
cargo check
```

Database-backed tests load `TEST_DATABASE_URL` from the ignored `.env`. Report
any skipped database coverage explicitly.

- [ ] **Step 2: Review the final change**

Run:

```bash
git diff --check
git status --short
git diff --stat
git log --oneline -8
```

Review that migrations `001` through `031` are unchanged and no admin
application file changed.

- [ ] **Step 3: Push and monitor backend deployment**

Push the verified commits to `main`, monitor the backend-school workflow until
it completes, and verify all active tenants report migration version `32`.
Never print the internal migration response body because it contains tenant
identifiers and migration errors.

- [ ] **Step 4: Run production readiness and smoke**

Run repository smoke procedures using ignored environment credentials:

```bash
./scripts/smoke_test.sh
```

Then run the `docs/TESTING.md` File Platform public/private
upload-download-delete smoke using temporary local files and cookie jar.
Never print a signed grant URL or object key.

- [ ] **Step 5: Close DB-004 only after production success**

Remove DB-004 from `TODO.md`, update the design/plan checkboxes if needed for
review, then remove the temporary spec and plan as required by `.rules`.

- [ ] **Step 6: Verify documentation policy and commit closure**

Run:

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
git diff --check
git status --short
```

Commit:

```bash
git add TODO.md docs/superpowers/specs/2026-07-29-file-platform-contract-cleanup-design.md docs/superpowers/plans/2026-07-29-file-platform-contract-cutover.md
git commit -m "docs: close file platform contract cleanup"
```

Push and confirm the documentation-only commit does not trigger an unrelated
application deployment.
