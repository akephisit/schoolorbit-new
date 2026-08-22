# School Font Library Design

## Goal

Create one tenant-scoped school font library that can be reused by certificate templates and later
by other SchoolOrbit modules. Certificate designers can upload a font while working on an exact
template, and a separately authorized font librarian can upload and remove fonts without gaining
certificate access.

The certificate module is the first consumer. The library itself remains independent from
certificate campaigns, templates, issuance, and campaign purge ownership.

## Confirmed Product Decisions

- The library is school-wide inside each tenant database; it is not shared across schools.
- A certificate designer who may update an exact template may upload fonts into the shared library.
- A user with `font.manage.school` may manage the central library without certificate permissions.
- All certificate templates use the same uploaded-font catalog.
- Shared fonts are deleted only from the central library and only while unreferenced.
- Certificate campaign or template deletion removes only certificate reference rows, never shared
  font files.
- Existing logo and certificate-image upload behavior remains unchanged.
- Legacy template-owned fonts have been removed by the user. Migration `040` performs no backfill,
  compatibility parsing, or automatic legacy-data deletion.

## Chosen Architecture

Use a durable `school_fonts` aggregate that owns one private File Platform logical file per static
font variant. Each consuming module owns a strongly typed reference table with a real foreign key
to `school_fonts`. The certificate module therefore owns
`certificate_template_font_references`; another module must add its own typed reference relation
when it becomes a real consumer.

This keeps file ownership generic without weakening consumer integrity. The design does not use a
polymorphic `(consumer_type, consumer_id)` table because PostgreSQL could not enforce that each
consumer ID exists. It also does not retain template-owned font assets, because that would keep
campaign purge and shared-library ownership entangled.

## Scope

### In scope

- School-wide font schema, upload staging relation, inspection metadata, variant uniqueness, usage
  counts, rights confirmation, and reference-safe deletion.
- A private `school_font` File Platform purpose using the existing scanner, inspection, immutable
  object keys, temporary-upload retention, promotion, grants, and durable deletion mechanisms.
- A standalone school-font API and a settings-workspace management page.
- The dedicated permission `font.manage.school` and its generated registries.
- Certificate-context list, inspect, and batch-attach APIs authorized against an exact template.
- Certificate layout validation, save-time reference synchronization, preview, issuance, render
  manifests, browser rendering, campaign purge, and tests using `schoolFontId`.
- Source-first OpenAPI and generated TypeScript contract updates.
- Canonical operations and testing documentation updates for rollout and recovery.

### Out of scope

- Cross-tenant or global font sharing.
- Font conversion, editing, renaming, replacement, version history, or byte-level deduplication.
- Variable fonts or arbitrary variation axes.
- Public font delivery or permanent public URLs.
- Automatic integration with document, report, or design systems that do not yet consume custom
  fonts.
- A polymorphic consumer registry.
- Realtime font-library broadcasts. Other open pages observe additions on their next fetch.
- Logo, background, or image behavior changes.
- Legacy font backfill, compatibility DTOs, dual-read paths, or automatic legacy cleanup.

## Data Model

Migration `backend-school/migrations/040_school_font_library.sql` is the only schema migration.
Applied migrations `001` through `039` remain byte-for-byte unchanged.

### `school_font_file_uploads`

This relation records authorization context between a successful private upload and its atomic
promotion into `school_fonts`:

- `file_id UUID PRIMARY KEY` references `files(id)` with cascade on File Platform metadata cleanup.
- `purpose_code VARCHAR(100) NOT NULL DEFAULT 'school_font'`, constrained to `school_font`, with a
  composite foreign key to `files(id, purpose_code)`.
- `uploaded_by UUID NOT NULL` references `users(id)` with restrict.
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`.
- A composite foreign key proves the referenced file has purpose `school_font`.

This table belongs only to standalone central-manager uploads and therefore has no consumer ID.
The staging row is removed in the same transaction that creates the durable library row. Failed or
abandoned uploads keep temporary retention and use the existing authorized File Platform deletion
path.

### `certificate_school_font_file_uploads`

Certificate-context uploads use a separate typed staging relation:

- `file_id UUID PRIMARY KEY` and constant `purpose_code = 'school_font'` use a composite foreign
  key to `files(id, purpose_code)`.
- `template_id UUID NOT NULL` references `certificate_templates(id)` with cascade.
- `uploaded_by UUID NOT NULL` references `users(id)` with restrict.
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`.

This prevents the generic library relation from accumulating nullable consumer columns. A future
consumer that supports inline uploads adds its own typed staging relation and authorization path.

### `school_fonts`

Each row represents one inspected static font face:

- `id UUID PRIMARY KEY DEFAULT gen_random_uuid()`.
- `file_id UUID NOT NULL UNIQUE` and `purpose_code VARCHAR(100) NOT NULL DEFAULT 'school_font'` use
  a composite foreign key to `files(id, purpose_code)` with restrict, proving durable ownership of
  the correct File Platform purpose.
- `display_name VARCHAR(200) NOT NULL` with a trimmed non-empty check.
- `font_family VARCHAR(200) NOT NULL` with a trimmed non-empty check.
- `normalized_family VARCHAR(200) NOT NULL`, computed by the application from Unicode NFKC,
  trimming, and lowercase normalization.
- `font_weight SMALLINT NOT NULL`, constrained to 100–900 in increments of 100.
- `font_style TEXT NOT NULL`, constrained to `normal` or `italic`; static oblique is normalized to
  `italic` by the trusted inspector.
- `rights_confirmed_by UUID NOT NULL` and `rights_confirmed_at TIMESTAMPTZ NOT NULL`.
- `created_by UUID NULL`, `created_at`, and `updated_at`.
- A unique index on `(normalized_family, font_weight, font_style)`.

Variable fonts, missing families, unsupported weights, malformed files, non-font contents,
non-ready files, and duplicate variants are rejected before insertion. The database unique index
is the race-safe final duplicate guard.

### `certificate_template_font_references`

- `template_id UUID NOT NULL` references `certificate_templates(id)` with cascade.
- `font_id UUID NOT NULL` references `school_fonts(id)` with restrict.
- Primary key `(template_id, font_id)`.

The certificate service derives the exact referenced font-ID set from layout JSON, validates the
font metadata, and replaces this relation set inside the same transaction that saves the layout.
Template deletion and campaign purge cascade only these reference rows. Deleting a library row is
blocked by the foreign key if a save or delete races the service-level usage check.

### Legacy cutover

Migration `040` starts with a fail-closed preflight. It raises a descriptive integrity error if it
finds any of the following:

- `certificate_template_assets.kind = 'font'`;
- `certificate_template_file_uploads.purpose_code = 'certificate_template_font'`;
- a certificate text element whose `fontSource.type = 'asset'`.

After the empty-state proof, migration `040` removes the legacy font-only index, font-specific
columns and constraints from `certificate_template_assets`, restricts template assets to images,
removes `certificate_template_font` from template-upload constraints, and installs the new school
font schema. Runtime `FilePurpose::CertificateTemplateFont` and its upload policy are removed in
the same release. The migration does not delete, transform, or retain a compatibility path for
legacy rows.

## File Platform Ownership and Lifecycle

Add `FilePurpose::SchoolFont` with wire code `school_font`:

- domain segment `school`;
- purpose segment `font`;
- private visibility;
- TTF and OTF content only;
- 5 MiB maximum per file;
- required clean malware scan;
- trusted font inspection metadata;
- temporary retention until attached to `school_fonts`.

`POST /api/files` accepts an optional `resource_id` for this purpose:

- no resource ID requires `font.manage.school`;
- a template ID requires exact `certificate.update.organization_unit` or
  `certificate.update.school` resource authorization.

After upload, `consumer_service` records `school_font_file_uploads` for central management or
`certificate_school_font_file_uploads` for an exact template. Inspection and batch attach
re-authorize and lock the matching typed staging rows. Batch attach validates every selected file,
inserts all font rows, promotes all logical files to standard retention, removes staging rows, and
records a safe audit event in one transaction. Any validation or database failure attaches none of
the batch.

Render services resolve a `school_fonts.id` to its logical file ID server-side and request the
existing short-lived private grant. APIs and logs never expose bucket names, object keys, provider
URLs, raw provider errors, font bytes, raw font tables, or persisted signed URLs.

Central deletion locks the font row, obtains a safe reference count, and returns conflict while
any consumer row exists. If unreferenced, it removes the library row and requests durable File
Platform deletion after the domain transaction commits. A provider failure remains retryable File
Platform work; the font is no longer selectable after its domain row is removed.

## Permission and Authorization Model

Add the permission contract entry:

```text
font.manage.school
```

Its module is `font`, action is the already permitted contract action `manage`, and scope is
`school`. It authorizes only the standalone list, inspect, attach, delete, and temporary-file
cleanup paths for the school font library. It does not imply certificate, settings, file-platform,
or other module access.

The permission is not bundled into certificate permissions. Existing role/permission
administration assigns it explicitly. Certificate-context font endpoints use the existing
certificate resource policy instead:

- exact template read permits listing available font metadata;
- exact template update permits upload, inspection, and batch attach;
- certificate-context endpoints never permit central deletion.

Render and issuance operations resolve referenced fonts under the already-authorized certificate
operation and do not require `font.manage.school`.

## Typed API Contract

### Standalone school-font endpoints

- `GET /api/school-fonts`
- `POST /api/school-fonts/inspect`
- `POST /api/school-fonts/batch`
- `DELETE /api/school-fonts/{font_id}`

All require `font.manage.school`. The list response may contain a safe `referenceCount` but never
consumer IDs or names.

### Certificate-context endpoints

- `GET /api/certificates/templates/{template_id}/fonts`
- `POST /api/certificates/templates/{template_id}/fonts/inspect`
- `POST /api/certificates/templates/{template_id}/fonts/batch`

The two inspect/batch surfaces delegate to the same school-font service after their respective
authorization checks. Multi-file batches keep the existing limit of 40 static TTF/OTF files and
the existing one-time rights confirmation.

### Wire models

The common contract includes:

- `SchoolFontSummary` with ID, display name, family, weight, style, creation timestamp, and safe
  reference count;
- `SchoolFontListResponse`;
- `InspectSchoolFontUploadsRequest`;
- `AttachSchoolFontBatchRequest`;
- `SchoolFontUploadInspection` and per-file status;
- `SchoolFontDeleteConflict` with `referenceCount`.

`SchoolFontStyle` is the shared `normal`/`italic` enum. Certificate text elements, built-in font
descriptors, school-font summaries, and render grants reuse this type rather than defining a
certificate-owned duplicate.

Certificate text layout replaces the uploaded source variant with:

```json
{
  "type": "school_font",
  "font_id": "00000000-0000-4000-8000-000000000000"
}
```

`CertificateRenderFontGrant` replaces `assetId` with `schoolFontId`. Image grants retain
`assetId` and remain otherwise unchanged.

All JSON endpoints use the standard `ApiResponse` envelope. Rust DTOs and `utoipa` registration
own the contract; OpenAPI JSON and TypeScript schemas are generated artifacts.

## Error Contract

- `403` for missing `font.manage.school` or exact-template authorization.
- Tenant-safe `404` for unknown font or template relationships.
- `409 school_font_in_use` with only a safe `referenceCount`.
- `409 school_font_variant_conflict` for a normalized family/weight/style collision.
- `422 school_font_invalid` for malformed, variable, unsupported, wrong-purpose, or incomplete
  metadata.
- `422 school_font_unavailable` for a file that is no longer ready or no longer belongs to the
  authorized staging context.
- Existing safe `503` behavior for scanner or storage unavailability.

Frontend disabled states are advisory. The backend rechecks all permissions, ownership,
readiness, duplicate, and usage conditions under lock.

## Certificate Consumer Integration

The certificate editor loads the shared school-font list separately from
`CertificateTemplateDetail.assets`. Template assets continue to contain images only. Built-in
Sarabun 400 and 700 normal faces are merged with `SchoolFontSummary` in the frontend font-variant
helper.

Saving a layout validates that each `school_font` source matches the row's family, weight, and
style. Preview and issuance reject missing or mismatched references rather than silently falling
back. Render manifests grant only the school fonts actually referenced by the current layout.

The template asset panel retains the current batch-upload interaction, but successful uploads
patch a local shared-font list rather than adding template assets. It offers no shared-font delete
control. The central page owns deletion.

Campaign purge inventory includes backgrounds, template images, and their temporary uploads. It
explicitly excludes `school_font` files. Purge validation treats a shared font reference as a
normal external dependency that disappears through template cascade without acquiring file
ownership.

## Standalone User Experience

Create `/staff/school-fonts` in the settings workspace with menu and route access guarded only by
`font.manage.school`. The page must not fetch school settings, certificate campaigns, templates,
or recipient data.

The page provides:

- a compact multi-file TTF/OTF uploader;
- detected family, weight, and style review;
- a single rights-confirmation control for attachable rows;
- retry and explicit cleanup for temporary uploads;
- a library table grouped deterministically by family and sorted by style/weight;
- safe usage counts;
- deletion with confirmation and authoritative conflict feedback.

Users without certificate access see no campaign or template names. Users with certificate update
rights continue to upload from the template workflow without needing the central permission.

## Security and Privacy

Font files contain no expected PII, but they remain untrusted uploads and private assets. The
existing scanner, trusted parser, size/type checks, immutable object versions, and short-lived
grants remain mandatory. Do not log request bodies, filenames in provider errors, font bytes,
object keys, signed URLs, session data, credentials, or recipient values.

No national-ID fields or behavior are changed. No new realtime identity, session, or public-file
surface is introduced.

## Deployment and Recovery

The backend-school centralized migration runner applies migration `040` to every active tenant.
Deployment must stop if any tenant fails the legacy-empty preflight; operators must not edit the
migration, patch `_sqlx_migrations`, or manually delete rows. The tenant must be returned to the
supported application workflow for investigation and cleanup before retrying migration.

File Platform readiness still requires both buckets and clamd. Abandoned temporary school-font
uploads use normal temporary retention and authorized cleanup. Failed durable object deletion uses
the existing reconciler; operators inspect only safe file IDs, purpose aggregates, lifecycle
states, and error codes.

## Testing Strategy

### Backend and database

- Migration continuity and fail-closed legacy-empty preflight.
- Valid school-font rows and rejection of invalid/duplicate variants.
- Central manager allowed without certificate permissions; ordinary user denied.
- Exact-template reader may list, updater may upload/attach, and unrelated updater is denied.
- Batch attach is atomic and promotes every logical file only after revalidation.
- Temporary cleanup respects central or exact-template upload authority.
- Layout save synchronizes font references in the same transaction.
- Deletion rejects referenced fonts and remains safe under save/delete races.
- Preview, issuance, and render manifests resolve only exact school-font variants.
- Campaign purge excludes school-font logical files and deletes only certificate reference rows.
- File Platform purpose, scan, inspection, private visibility, and grant invariants.

### Frontend

- Permission and route metadata expose the library without certificate access.
- API wrappers consume generated DTOs without casts or `unknown`.
- Upload review, rights confirmation, retry, cleanup, duplicate, and in-use errors.
- Editor font-family/weight/style selection uses school-font IDs.
- Browser renderer registers and reuses school-font grants without affecting image grants.
- Certificate editor, renderer, and lifecycle browser coverage remains serial and deterministic.
- Every changed Svelte file passes the project Svelte analyzer.

### Contract and repository gates

- Permission contract generation/check/tests.
- API contract generation/check/tests.
- Backend formatting, static architecture, focused database tests, and `cargo check`.
- Frontend lint, Svelte check, static tests, menu synchronization, and Playwright discovery/run.
- Documentation-policy tests after canonical operations/testing updates.
- `git diff --check`, complete base-to-head diff review, generated-source provenance, secret and
  national-ID audit, and `git status --short`.

The documented destructive certificate lifecycle requires a separate authorization and isolated
tenant credentials. Production permission assignment, migration rollout, deployment, smoke,
push, pull request, and merge are not authorized by this design approval.

## Success Criteria

- One uploaded static font variant is selectable from every certificate activity in the same
  school without re-uploading.
- A certificate designer can add a font from an exact template and the new font enters the shared
  library.
- A dedicated librarian can manage fonts without gaining certificate access.
- A shared font cannot be removed while any typed consumer reference exists.
- Deleting or purging certificate activities never deletes shared font files.
- Preview, issuance, and downloaded certificates render the exact selected school font.
- Other modules can become consumers by adding their own typed reference table and authorized API
  integration without changing school-font file ownership.
- Logo and certificate-image behavior remains unchanged.
