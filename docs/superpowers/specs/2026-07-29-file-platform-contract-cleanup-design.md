# File Platform Contract Cleanup Design

**Date:** 2026-07-29
**Backlog item:** DB-004
**Scope:** `backend-school`, tenant migrations, File Platform tests, and operational rollback guidance
**Out of scope:** `backend-admin`, `frontend-admin`, new File Platform features, and API contract changes

## Context

Migrations `030_file_platform.sql` and
`031_file_platform_domain_references.sql` deliberately left the original
path-based file columns in place. That additive window made the first File
Platform deployment reversible, but the current backend still duplicates data
into those columns and several services still read them:

- upload reservation writes object locators, MIME type, size, checksum, owner,
  visibility, and retention state to both `files` and `file_versions`;
- delivery reads the legacy owner, filename, and size fields;
- admission document queries read legacy filename, size, and MIME fields;
- question-bank ownership and temporary-file handling use `user_id` and
  `is_temporary`;
- the expiry cleaner and domain attachment flows still update or filter
  `is_temporary`;
- `active_files` and `generate_storage_path` preserve the old locator model;
- `users.profile_image_url` and `staff_achievements.image_path` remain even
  though current domain code uses file IDs.

The production system has no retained user files that need a bespoke data
conversion. Short-lived smoke-test or recently replaced image rows may still
exist, including soft-deleted rows, so the migration must verify actual rows
rather than relying on that operational expectation.

The cleanup must never edit migrations `001` through `031`. It must preserve a
known rollback target, must not expose object keys or signed URLs during audit,
and must leave the provider-neutral file-ID boundary intact.

## Decision

Use a two-release expand-and-contract rollout.

Release A introduces the final logical column names, migrates every runtime
consumer to the canonical File Platform model, and installs temporary database
compatibility synchronization. Legacy columns remain in this release so the
immediately previous backend image can still run.

Release B runs only after Release A is deployed and verified. It fails closed
when legacy-only data is found, then removes the compatibility synchronization
and old columns. The Release A backend image becomes the rollback target for
Release B because it no longer references any removed column.

This is preferable to dropping columns in one deployment because the current
binary still depends on them. It is also preferable to retaining duplicate
columns indefinitely because duplicated locator and lifecycle state can drift
and makes future document-system integration harder.

## Final Data Ownership

### Logical file identity

After Release B, `files` owns only logical, provider-neutral state:

- `id`;
- `owner_user_id`;
- `display_filename`;
- `purpose_code`;
- `visibility`;
- `lifecycle_status`;
- `current_version_id`;
- `retention_class`;
- `expires_at`;
- `delete_requested_at`;
- `created_by`;
- `created_at`, `updated_at`, and `deleted_at`.

`display_filename` and `purpose_code` are required for every logical file.
Owner and creator remain nullable because their user foreign keys use
`ON DELETE SET NULL`. `current_version_id` remains nullable while an upload is
processing or failed before publication.

### Immutable object metadata

`file_versions` remains the sole owner of original-object metadata:

- provider and storage class;
- internal object key;
- detected MIME type and canonical extension;
- byte size and checksum;
- scan and storage state;
- version creator and timestamp.

`file_derivatives` remains the sole owner of derivative locators and metadata.
The legacy `width` and `height` columns are not part of an API or current
consumer and will be removed rather than promoted to the logical file row.
Image dimensions remain an inspection-time safety input. If a future consumer
needs persisted dimensions, they belong on the immutable version or derivative
record and will be added with an explicit contract and migration.

### Domain relationships

Domain tables continue to reference opaque file IDs:

- `users.profile_image_file_id`;
- `staff_achievements.image_file_id`;
- school-settings branding file IDs;
- admission document rows;
- question-bank rich-content file IDs;
- future document or attachment tables.

No business module may store an R2 key, provider URL, bucket, signed URL, or a
second path field. A future document system will add its own authorized
relationship to `files(id)`.

## Release A: Contract Preparation

Add forward-only migration `032_file_platform_contract_preparation.sql`.

The migration will:

1. add `files.owner_user_id`, `files.display_filename`, and
   `files.created_by` with user foreign keys matching the old deletion
   behavior;
2. backfill those fields from `user_id`, `filename`, and `uploaded_by`;
3. add canonical owner and temporary-expiry indexes;
4. make legacy required metadata columns nullable so the new repository can
   stop inserting them;
5. require nonblank `display_filename` and `purpose_code` after backfill;
6. install bounded compatibility triggers that synchronize old and new logical
   fields during the rollback window;
7. populate legacy version metadata after a `file_versions` insert so the
   previous binary can still read files created by Release A;
8. populate legacy thumbnail flags after a derivative insert for the same
   rollback window.

The synchronization covers only deterministic mappings:

- `owner_user_id` ↔ `user_id`;
- `display_filename` ↔ `filename`/`original_filename`;
- `created_by` ↔ `uploaded_by`;
- `visibility` ↔ `is_public`;
- `retention_class` ↔ `is_temporary`;
- `purpose_code` → the existing bounded `file_type` values;
- current version byte size, MIME type, object key, and checksum → their
  legacy copies;
- derivative existence and key → the legacy thumbnail fields.

On update, a change made through one side is copied to the other side only when
the counterpart did not also change. Conflicting dual updates are rejected
rather than silently choosing one representation. Trigger errors use fixed
messages and never include filenames, object keys, user IDs, or provider data.

Release A application code then stops depending on compatibility fields:

- the SQL repository inserts and reads the new logical names;
- delivery size and MIME type come from the joined current version;
- admission document queries join `current_version_id` and read
  `display_filename`, `file_versions.byte_size`, and
  `file_versions.detected_mime_type`;
- question-bank ownership uses `owner_user_id`;
- temporary-file validation, finalization, and expiry use
  `retention_class = 'temporary'`;
- domain attachment flows update only `retention_class` and `expires_at`;
- the old `legacy_file_type` application mapping is removed.

No HTTP request or response shape changes. Frontend code and generated API
contracts remain untouched.

## Release A Verification and Rollback Evidence

Release A tests must demonstrate behavior, not only search source text:

- an isolated database migrated through `032` contains both schemas and
  backfilled canonical values;
- a legacy-shaped insert produces canonical values;
- a canonical-shaped insert plus version insert produces the legacy values
  needed by the previous backend;
- conflicting compatibility updates fail;
- the SQL repository reserves, finalizes, loads, and deletes a file without
  directly supplying legacy metadata;
- admission, question-bank, domain attachment, and expiry paths operate on the
  canonical columns.

After focused and backend verification passes, deploy Release A. Production
verification uses readiness, authenticated smoke, and temporary public/private
upload-download-delete flows. It records only commit/image identity, file IDs,
safe counts, and safe error codes. It never records credentials, object keys,
signed URLs, filenames, or raw provider responses.

During this window, rolling back to the pre-Release A backend remains supported
because migration `032` retains and synchronizes the legacy schema.

## Release B: Contract Cleanup

After Release A is stable, add forward-only migration
`033_remove_file_platform_compatibility_columns.sql`.

Before destructive DDL, one transactional guard checks:

- every file has a nonblank canonical display filename and purpose;
- every file has at least one `file_versions` row;
- every ready file points to a current version belonging to that file;
- no user has a nonblank `profile_image_url` without
  `profile_image_file_id`;
- no achievement has a nonblank `image_path` without `image_file_id`.

The guard raises fixed, count-free messages. It does not print row values or
locators. A tenant that fails the guard keeps its old schema unchanged and does
not receive traffic until the data is reviewed.

When the guard passes, migration `033`:

1. drops `active_files` and `generate_storage_path`;
2. drops the temporary compatibility triggers and functions;
3. drops `users.profile_image_url` and
   `staff_achievements.image_path`;
4. drops these legacy `files` columns:
   `user_id`, `school_id`, `filename`, `original_filename`, `file_size`,
   `mime_type`, `storage_path`, `file_type`, `width`, `height`,
   `has_thumbnail`, `thumbnail_path`, `is_temporary`, `is_public`,
   `checksum`, and `uploaded_by`;
5. retains and verifies the canonical owner and temporary-expiry indexes.

The migration is transactional and does not delete file, version, derivative,
operation, or domain-reference rows.

After Release B, rolling back to a binary older than Release A is unsupported
because that binary requires removed columns. The pinned Release A image is the
minimum supported backend rollback target. `docs/OPERATIONS.md` will state this
boundary and the required image/commit evidence.

## Security and Failure Behavior

- Authorization remains in the existing purpose registry and domain policies;
  schema cleanup does not weaken access checks.
- Public/private delivery behavior and signed-grant handling do not change.
- No API or log exposes provider locators.
- Migration audit failures are fail-closed and log only fixed safe messages.
- Applied migration files remain immutable; all work is in `032` and `033`.
- The cleanup does not touch national-ID data, encryption keys, permissions, or
  admin applications.

## Verification Matrix

Each release runs focused database and File Platform tests plus the repository
backend-school matrix:

```bash
cd backend-school
TEST_DATABASE_URL='provided-at-runtime' cargo test modules::files::schema_tests --bin backend-school
TEST_DATABASE_URL='provided-at-runtime' cargo test modules::files::repository::tests --bin backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

The database URL comes from the ignored test environment and is never printed.
The final review also runs:

```bash
git diff --check
git status --short
```

Production verification follows the File Platform smoke procedure in
`docs/TESTING.md` against the isolated school tenant. DB-004 is removed from
`TODO.md` only after Release B deploy, migration, readiness, and smoke evidence
all pass.
