# File Platform Contract Cutover Design

**Date:** 2026-07-29
**Backlog item:** DB-004
**Scope:** `backend-school`, tenant migrations, File Platform tests, deployment workflow, and operational guidance
**Out of scope:** `backend-admin`, `frontend-admin`, new File Platform features, and API contract changes

## Context

Migrations `030_file_platform.sql` and
`031_file_platform_domain_references.sql` introduced the provider-neutral File
Platform without removing the original path-based schema. The current backend
therefore still duplicates metadata and several services still consume the old
columns:

- upload reservation writes object locators, MIME type, size, checksum,
  visibility, and retention state to both `files` and `file_versions`;
- delivery and admission documents read legacy filename, size, and MIME fields;
- question-bank ownership uses `user_id`;
- question-bank, domain attachment flows, and the expiry cleaner use
  `is_temporary`;
- `active_files` and `generate_storage_path` preserve the old path model;
- `users.profile_image_url` and `staff_achievements.image_path` remain beside
  their file-ID replacements.

There are no retained user files requiring a bespoke conversion. Smoke-test,
replaced-image, or soft-deleted File Platform rows may still exist, so the
cutover must validate actual rows and must preserve canonical file/version
records.

The requested outcome is a clean cutover. The new backend will use only the new
schema; it will not dual-write, install compatibility triggers, or support a
backend binary that still expects the old columns.

## Decision

Perform one coordinated forward-only contract cutover.

A new migration renames the three legacy columns that still represent useful
logical concepts, drops path-based and duplicated metadata, and tightens the
final File Platform schema. The same release changes every runtime consumer to
the final names.

The migration fails closed before destructive DDL if any row depends only on
legacy metadata. Because the migration is transactional per tenant, a failed
tenant keeps its pre-cutover schema unchanged and receives no application
traffic until the cause is resolved.

This intentionally gives up rollback to a pre-cutover backend. Recovery after a
tenant applies the migration is fix-forward with the cutover image or a newer
image.

The previously proposed two-release compatibility window is rejected because
the owner does not require old-binary support and there are no retained files
that justify temporary dual schemas and synchronization triggers. Dropping
columns without a data guard is also rejected because operational expectations
must not substitute for database evidence.

## Final Data Ownership

### `files`: logical identity only

After migration, `files` owns:

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

The migration preserves data by renaming:

- `user_id` to `owner_user_id`;
- `filename` to `display_filename`;
- `uploaded_by` to `created_by`.

`owner_user_id` remains nullable and its renamed foreign key changes to
`ON DELETE SET NULL`; deleting a user must not cascade into immutable file
versions or retention records. `created_by` keeps `ON DELETE SET NULL`.
`display_filename` remains required, and `purpose_code` becomes required after
the preflight guard passes.

The per-tenant `school_id` column is redundant and is removed.

### `file_versions`: original object metadata

`file_versions` is the sole owner of:

- provider and storage class;
- internal object key;
- detected MIME type and canonical extension;
- byte size and checksum;
- scan and storage state;
- version creator and timestamp.

### `file_derivatives`: generated object metadata

`file_derivatives` is the sole owner of derivative locators, MIME type, size,
checksum, and lifecycle state. Legacy thumbnail flags and paths are removed.

The old `width` and `height` fields have no API or current consumer and are
removed. Image dimensions remain an upload-inspection safety input. A future
consumer that needs persisted dimensions will add them to immutable version or
derivative rows with an explicit contract.

### Domain relationships

Business modules keep only opaque file IDs, including:

- `users.profile_image_file_id`;
- `staff_achievements.image_file_id`;
- school-settings branding file IDs;
- admission document relationships;
- question-bank rich-content file IDs;
- future document or attachment relationships.

No domain table or API stores an R2 key, bucket, provider URL, signed URL, or
another path. A future document system will reference `files(id)` and own its
authorization relationship.

## Forward Migration

Add `032_file_platform_contract_cutover.sql`. Migrations `001` through `031`
remain byte-for-byte unchanged.

Before any rename or drop, a transactional guard verifies:

- every `files` row has a nonblank filename and `purpose_code`;
- every `files` row has at least one `file_versions` row;
- every ready file points to a current version that belongs to that file;
- no user has a nonblank `profile_image_url` without
  `profile_image_file_id`;
- no achievement has a nonblank `image_path` without `image_file_id`.

The guard raises fixed messages without row values, counts, filenames, user
identifiers, object keys, or provider details.

When the guard passes, migration `032`:

1. drops `active_files` and `generate_storage_path`;
2. drops the old owner foreign key, renames `user_id` to `owner_user_id`, and
   creates `files_owner_user_id_fkey` with `ON DELETE SET NULL`;
3. renames `filename` to `display_filename` and `uploaded_by` to
   `created_by`, including their useful index/constraint names;
4. makes `purpose_code` non-null and adds a nonblank
   `display_filename` check;
5. replaces the temporary expiry index with one filtered by
   `retention_class = 'temporary'`;
6. drops `users.profile_image_url` and
   `staff_achievements.image_path`;
7. drops these duplicated or path-based `files` columns:
   `school_id`, `original_filename`, `file_size`, `mime_type`,
   `storage_path`, `file_type`, `width`, `height`, `has_thumbnail`,
   `thumbnail_path`, `is_temporary`, `is_public`, and `checksum`.

The migration does not delete logical files, versions, derivatives, operations,
or domain-reference rows.

## Application Cutover

The same release updates all backend-school consumers:

- upload reservation inserts only logical fields into `files` and immutable
  object metadata into `file_versions`/`file_derivatives`;
- delivery loads owner and display name from `files`, then MIME type and byte
  size from the current `file_versions` row;
- admission document queries join the current version and expose the existing
  response fields from canonical sources;
- question-bank ownership uses `owner_user_id`;
- temporary-file validation, finalization, and expiry use
  `retention_class = 'temporary'`;
- profile, school branding, achievement, admission, question-bank, and staff
  attachment flows update only `retention_class` and `expires_at`;
- the application-side `legacy_file_type` mapping is removed;
- test fixtures insert final-schema rows.

HTTP routes, authorization policies, JSON envelopes, and generated API
contracts do not change. No frontend change is required.

## Deployment and Recovery

This is a schema contract boundary and uses a maintenance cutover:

1. verify the new image and migration against the isolated test database;
2. audit safe aggregate File Platform row/object state and confirm no retained
   legacy-only data;
3. prevent tenant traffic from racing the cutover;
4. start the new backend image and migrate every active tenant to `032`;
5. confirm every active tenant reports migration success before restoring
   traffic;
6. run readiness, authentication, and temporary public/private
   upload-download-delete smoke checks.

The deployment path must not automatically restore the pre-cutover image after
any tenant has applied `032`. The recovery floor is the cutover image itself.
Failure recovery is:

- before any tenant applies `032`: the old image remains usable;
- after the first tenant applies `032`: keep the cutover schema and fix
  forward;
- if one tenant's preflight guard fails: leave that tenant unavailable, review
  safe aggregates, correct the data through an explicit forward migration, and
  retry.

`docs/OPERATIONS.md` will record the minimum compatible image commit and the
no-old-binary rollback boundary. Credentials, database URLs, object keys,
signed URLs, filenames, and raw provider responses are never recorded.

## Testing

Implementation follows test-driven development with real migrated schema
behavior:

- a pre-cutover fixture with canonical File Platform rows migrates successfully
  and retains logical IDs and version relationships;
- a legacy-only file row is rejected before any column is dropped;
- a ready file with a cross-file or missing current version is rejected;
- legacy path-only profile and achievement references are rejected;
- the final schema contains the renamed logical columns and none of the removed
  columns, view, or function;
- repository reserve/finalize/load/delete behavior works against the final
  schema;
- admission, question-bank, attachment finalization, and expiry paths work
  against canonical fields.

Focused and required verification:

```bash
cd backend-school
TEST_DATABASE_URL='provided-at-runtime' cargo test modules::files::schema_tests --bin backend-school
TEST_DATABASE_URL='provided-at-runtime' cargo test modules::files::repository::tests --bin backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

The final review also runs:

```bash
git diff --check
git status --short
```

Production verification follows the File Platform smoke procedure in
`docs/TESTING.md`. DB-004 is removed from `TODO.md` only after every active
tenant has applied `032` and readiness plus smoke verification pass.
