# File Platform Design

**Date:** 2026-07-28  
**Status:** Pending written-spec review  
**Backlog owner:** `SEC-005`

## Problem

SchoolOrbit currently treats file storage as a collection of R2 calls rather than
as a platform boundary. The generic file API accepts a client-selected file type,
stores every upload as public, and returns storage paths and public URLs. Admission,
question bank, school settings, and cleanup code also call R2 directly or depend on
storage paths.

This creates four risks:

1. authorization and visibility rules differ between consumers;
2. private records can be exposed through a public storage path;
3. database and object-storage state can diverge after partial failures;
4. adding another storage or document system would require changes throughout the
   product.

The current production data inventory is reported to contain no file records. The
rollout must verify that claim across every active tenant before relying on it.

## Goals

- Make private files inaccessible without an authoritative SchoolOrbit policy
  decision.
- Keep public assets fast and cacheable without mixing them with private objects.
- Give business modules one file-ID-based application interface.
- Isolate provider-specific behavior behind a storage port.
- Support immutable versions, derivatives, scanning, deletion retries, and audit.
- Make future document workflows and external providers consume the same platform.
- Preserve generated permission and API contracts.
- Use only forward tenant migrations.

## Non-goals

- Implement a document sending, approval, inbox, or e-signature workflow.
- Implement Google Drive, OneDrive, or another storage adapter in this release.
- Expose arbitrary user-created storage paths or provider folders.
- Create a universal string-based resource foreign key.
- Move unrelated media or export code that does not persist an object.
- Change `backend-admin` or `frontend-admin`.

## Considered Approaches

### Minimal patch to the current file module

Add permission checks and set `is_public` correctly while keeping direct R2 calls
and path-based responses.

This is the smallest change, but storage behavior would remain duplicated across
modules and future providers would still require product-wide changes.

### One private bucket with backend-proxied delivery

Keep every object private and stream public and private files through
`backend-school`.

This creates a simple security boundary, but it makes the application service a
bandwidth bottleneck and complicates stable CDN caching for logos and banners.

### File Platform with public/private storage classes

Create one application service, one storage provider port, and distinct public and
private R2 buckets. Business modules use file IDs and domain policies.

This is the selected approach. It keeps public delivery efficient, private delivery
authorized, and provider integration localized.

## Architecture

```text
Admission / Staff / School / Question Bank / future Document workflows
                              |
                            file ID
                              |
                     File Platform Service
              +---------------+----------------+
              |               |                |
         Access policy   Content pipeline   Lifecycle/retry
                              |
                       StorageProvider
                              |
                  R2 public / R2 private
                              |
                  future provider adapters
```

The platform has four boundaries:

1. **File application service** orchestrates metadata, validation, provider calls,
   versions, derivatives, and lifecycle state.
2. **Purpose registry** owns visibility, limits, allowed media, scan requirements,
   derivatives, retention, and the domain access-policy key.
3. **Domain access policy** answers whether an actor may create, read, or delete a
   file in a specific business resource.
4. **Storage provider port** handles bytes and short-lived delivery grants without
   making authorization decisions.

Business modules must not instantiate an R2 client, build a storage path, or return
a provider URL. They call the File Platform and persist a file ID in their own
resource table.

## Storage Topology

The initial provider is R2 with two buckets:

- `R2_PUBLIC_BUCKET_NAME` contains only assets whose purpose registry visibility
  is `public`;
- `R2_PRIVATE_BUCKET_NAME` contains private files and any object that has not
  reached the `ready` state.

`R2_PUBLIC_URL` applies only to the public bucket. Private delivery uses an R2
presigned request created by the provider adapter. Production readiness fails when
either required bucket configuration is absent.

Object storage folders are virtual prefixes. Keys use this layout:

```text
tenants/{tenant_uuid}/{domain}/{purpose}/{file_uuid}/v{version}/original.{ext}
tenants/{tenant_uuid}/{domain}/{purpose}/{file_uuid}/v{version}/derivatives/{variant}.{ext}
```

Examples:

```text
tenants/{tenant_uuid}/school/logo/{file_uuid}/v1/original.webp
tenants/{tenant_uuid}/identity/id-card/{file_uuid}/v1/original.pdf
tenants/{tenant_uuid}/admission/application-document/{file_uuid}/v1/original.pdf
tenants/{tenant_uuid}/question-bank/image/{file_uuid}/v1/derivatives/thumbnail-256.webp
```

Key rules:

- use the stable school UUID already returned by the protected internal school
  response, extending only `backend-school` tenant context to retain it; never use
  a mutable subdomain;
- use only registry-owned domain and purpose segments;
- use random file and version identifiers;
- derive the extension from detected content, not the submitted filename;
- never include a person's name, national ID, student code, application number,
  phone number, original filename, or user-created folder path.

User-created document folders, if added later, are database metadata. Renaming or
moving a logical folder must not move an object.

## Provider Port

`StorageProvider` is an async application port with operations equivalent to:

- put an immutable object;
- inspect whether an object exists and read safe metadata;
- delete an object idempotently;
- create a short-lived private download grant;
- obtain the delivery location for a public object.

The port receives a platform-generated object key and storage class. It does not
receive an actor, permission, business resource, or client-selected bucket.

The initial `R2StorageProvider` implements this port using the existing S3-compatible
client. Future adapters may return a redirect URL or an application stream through
a provider-neutral `DownloadGrant`; API consumers do not branch on provider type.

## Purpose Registry

Every upload has a purpose code defined in a code-owned registry. A purpose entry
contains:

- domain and purpose path segments;
- public or private storage class;
- allowed detected MIME types;
- maximum byte size and, for images, maximum dimensions and decoded pixels;
- whether malware scanning is mandatory;
- derivative recipes;
- retention class;
- domain access-policy key.

Every initial client-upload purpose requires malware scanning. The registry keeps
the requirement explicit so a future trusted, provider-generated artifact can have
a separately reviewed policy without weakening ordinary uploads.

Clients may request a purpose code, but unknown purposes are rejected and the
registry determines every security-relevant property. Clients cannot choose the
provider, bucket, visibility, object key, lifecycle status, resource owner, scan
policy, or retention policy.

Initial purposes cover the current persisted consumers:

- school logo and banner;
- profile image;
- admission application document;
- transcript, certificate, and ID card;
- question-bank image;
- course material and assignment attachment;
- generic private document only where an explicit owning resource policy exists.

## Data Model

The existing `files` table remains the logical file identity so current domain
foreign keys can migrate without changing identity.

### `files`

The forward migration adds or formalizes:

- `purpose_code`;
- `visibility`;
- `lifecycle_status`;
- sanitized display filename;
- `current_version_id`;
- retention and deletion timestamps;
- created and updated timestamps.

During the additive compatibility window, the existing `user_id` and `uploaded_by`
columns remain the physical owner and creator columns. File Platform domain types
name those concepts `owner_user_id` and `created_by`; a later verified cleanup
migration may rename the physical columns after legacy consumers are gone. This
avoids duplicate ownership fields and preserves application rollback.

Provider, bucket, and object-key details do not appear in API DTOs.

### `file_versions`

Each immutable version records:

- version ID and logical file ID;
- monotonically increasing version number;
- provider code and storage class;
- internal object key;
- detected MIME type and canonical extension;
- byte size and SHA-256 checksum;
- scan status and safe scanner result code;
- creator and timestamps.

The table has unique constraints for `(file_id, version_number)` and for the
provider/object-key locator.

### `file_derivatives`

Each row records a derivative kind such as `thumbnail-256` or `preview-1024`, its
source version, provider locator, media metadata, checksum, and lifecycle status.

### `file_operations`

Durable operations cover scan, derivative generation, object deletion, and
reconciliation. Rows contain operation type, target ID, status, attempts,
`next_retry_at`, lease information, and a bounded safe error code. They never store
credentials, signed URLs, request bodies, or raw provider responses.

### Domain relationships

Domain tables own their relationships:

```text
admission_application_documents(application_id, file_id, document_type)
question or rich-document references(file_id)
school_settings(logo_file_id)
future document_attachments(document_id, file_id, attachment_role)
```

The platform does not add a generic `(resource_type, resource_id)` relationship
that cannot enforce foreign keys or resource authorization.

## Authorization

The File Platform is not the source of business authorization. A purpose maps to a
domain policy registered in backend code.

For every create, read, download, version, or delete operation:

1. resolve the authoritative tenant and actor;
2. load file metadata and its domain relationship;
3. select the policy from the server-owned purpose registry;
4. enforce generated permission constants and resource scope;
5. perform the file operation only after the policy allows it.

Representative rules:

- school branding requires the existing school-settings update authority;
- a user can manage their own profile image, while staff management uses its exact
  people permission and resource policy;
- an applicant uses the authoritative application portal session for only their
  application;
- admission staff access follows the admission permission and resource scope;
- question-bank images inherit access from the referenced question;
- public files are unauthenticated only for read delivery after reaching `ready`;
  create, replace, and delete remain authorized.

Any missing permission codes are added to `contracts/permissions.json`, generated
registries, and a new forward permission migration together. Raw permission
strings do not enter feature handlers.

## API Contract

The primary authenticated routes are:

```text
POST   /api/files
GET    /api/files/{id}
POST   /api/files/{id}/download
DELETE /api/files/{id}
```

A public ready asset may be delivered through:

```text
GET /api/public/files/{id}/content
```

The public route resolves the tenant, verifies public visibility and ready state,
and redirects to the public CDN location with an appropriate cache policy.

Private download returns or redirects through a short-lived `DownloadGrant`.
Provider-specific URLs are ephemeral capabilities: they are never persisted,
logged, included in list metadata, or accepted back as file identity.

File DTOs expose only authorized metadata such as:

- file ID;
- sanitized original filename;
- detected MIME type;
- byte size;
- purpose, lifecycle status, and version;
- a platform public-content URL only for ready public assets.

The API never returns an object key, bucket, provider credential, permanent private
URL, checksum unless explicitly needed, or scanner details.

Domain services finalize attachment relationships inside their own database
transactions. There is no public endpoint that lets a client attach an arbitrary
file ID to an arbitrary resource.

Rust DTOs and OpenAPI remain authoritative. Frontend types and helpers are
generated or built over generated DTOs.

## Upload Lifecycle

```text
authorize purpose
  -> enforce streaming byte limit
  -> inspect signature, structure, dimensions, and decoded-pixel limit
  -> malware scan
  -> compute checksum and immutable key
  -> create processing metadata
  -> provider put
  -> create required derivatives
  -> mark ready
```

Submitted MIME type and extension are hints only. The content inspector identifies
the supported format by signature and validates its internal structure. Unsupported
or mismatched content is rejected.

`FileInspector` performs local signature, document-structure, and image safety
checks. `MalwareScanner` is a separate port. The initial production adapter uses a
configured clamd-compatible endpoint. All initial client uploads fail closed when
the scanner is unavailable or times out. Tests use a deterministic fake scanner.

No download path serves a file before `ready`. Public objects are not placed in the
public bucket until validation and scanning succeed.

## Download Lifecycle

```text
file ID
  -> ready-state check
  -> domain access policy
  -> audit decision
  -> public CDN redirect or short-lived private grant
```

Private grants have a short bounded lifetime and a sanitized content-disposition
filename. The service does not log the grant or its query parameters.

## Delete and Reconciliation Lifecycle

Delete is idempotent:

```text
authorize
  -> mark deleting and revoke delivery
  -> enqueue deletion for every version and derivative
  -> provider delete with retry
  -> mark deleted after all objects are absent
```

Failure rules:

- metadata creation followed by upload failure leaves a `failed` record that the
  reconciler can expire;
- object upload followed by metadata-finalization failure leaves deterministic
  pending metadata that can be inspected and finalized or deleted;
- optional derivative failure enqueues only that derivative for retry;
- required derivative failure keeps the file non-ready;
- provider deletion failure keeps the file in `deleting` and inaccessible;
- scanner outage keeps client uploads non-ready and reports a safe service
  availability error;
- repeated terminal failures become observable repair work, not swallowed warnings.

The reconciler uses bounded retries, leases, exponential backoff, and idempotent
provider operations. Logs identify the file and operation IDs but exclude original
filenames, object keys, signed URLs, credentials, and private payloads.

## Consumer Migration

The release migrates every persisted-file consumer:

1. generic file upload/list/delete;
2. school logo and banner;
3. admission application documents and portal documents;
4. question-bank images and downloads;
5. file cleanup jobs.

Direct `R2Client` use is removed from business modules. R2 remains only in the
provider adapter and provider-focused tests.

Frontend consumers use one typed helper for upload, metadata, download, and delete.
They retain file IDs rather than provider URLs or paths.

## External Integration Model

Future storage adapters implement the provider port and are registered by provider
code. Domain modules remain unchanged.

Future document systems reference file IDs in their attachment table. Folder trees,
recipients, delivery state, acknowledgements, approval, and e-signature belong to
the document domain, not the File Platform.

Future service-to-service access uses a scoped service identity and the same purpose
policy boundary. It does not receive R2 credentials or bypass file authorization.

## Migration and Rollout

1. Audit every active tenant for file-row count and referenced file IDs.
2. If any file row exists, stop this rollout and add an explicit object inventory
   and migration procedure before changing delivery behavior.
3. Create and configure the private R2 bucket while preserving the public bucket.
4. Add forward tenant migration `030`; do not edit the applied baseline or
   migrations `002` through `029`.
5. Deploy additive schema, platform services, provider/scanner adapters, and new
   routes.
6. Migrate every current consumer and remove its direct provider dependency.
7. Run permission and API generation and contract checks.
8. Verify readiness configuration before enabling uploads.
9. Run authenticated upload/download/delete smoke tests for both storage classes.
10. Remove legacy route behavior only after repository searches and tests show no
    consumer depends on a storage path or permanent file URL.

The migration remains additive during rollout so application rollback does not
require a database rollback. Legacy columns are retained until a later verified
cleanup migration.

## Configuration

Required production configuration includes:

- existing R2 endpoint credentials and region;
- `R2_PUBLIC_BUCKET_NAME`;
- `R2_PRIVATE_BUCKET_NAME`;
- `R2_PUBLIC_URL`;
- clamd-compatible scanner endpoint and connection limits;
- bounded private-download grant lifetime;
- worker retry and lease settings.

Startup/readiness validation rejects missing or placeholder security-critical
configuration. Tests inject fake provider and scanner implementations and do not
use production credentials.

## Observability and Audit

Structured metrics cover:

- upload, inspection, scan, provider, and finalize latency;
- ready, processing, failed, deleting, and retry counts;
- provider and scanner availability;
- reconciler queue depth, attempts, age, and terminal failures;
- allowed and denied download decisions by purpose without user PII.

Audit events include actor, tenant, file ID, purpose, business resource ID where
allowed, action, result, and timestamp. They exclude filenames when unnecessary,
all object locators, signed URLs, credentials, national IDs, and file content.

## Verification

Required focused tests include:

- object-key construction excludes PII and uses stable tenant UUIDs;
- purpose registry rejects unknown purposes and client-controlled visibility;
- MIME spoofing, unsupported structures, oversize payloads, excessive image
  dimensions, and decompression bombs are rejected;
- scanner clean, infected, unavailable, and timeout outcomes;
- public/private bucket selection;
- allowed and denied create/read/delete policy paths;
- cross-tenant, cross-user, and unrelated-resource denial;
- provider put failure, metadata finalize failure, derivative retry, delete retry,
  and reconciler idempotency;
- public delivery only for ready public files;
- private grants are short-lived and absent from logs and metadata;
- API and permission contracts remain generated and current;
- admission, school branding, and question-bank integration paths;
- authenticated sandbox upload, download, delete, and `/api/auth/me` smoke.

The applicable `.rules` verification matrix, `git diff --check`, final diff review,
and `git status --short` are mandatory before completion.

## Acceptance Criteria

- No business module calls R2 or builds an object key directly.
- Private files cannot be fetched anonymously or through a permanent public URL.
- Public assets are the only objects written to the public bucket.
- Every operation is authorized through a purpose-owned domain policy.
- API responses do not expose storage paths, bucket names, or persistent private
  provider URLs.
- File content is inspected and malware-scanned before becoming ready.
- Upload and deletion partial failures are durable, retryable, and observable.
- Existing file consumers use the central platform and typed frontend helper.
- A new storage provider can be added without changing domain modules or API DTOs.
- A future document workflow can attach files by ID without creating another
  storage subsystem.
