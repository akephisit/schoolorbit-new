# File Platform Implementation Plan

> **Required execution skill:** Use `superpowers:subagent-driven-development` to execute this plan in an isolated worktree. Apply `superpowers:test-driven-development` to every behavior change and `superpowers:verification-before-completion` before reporting success.

**Goal:** Replace direct, path-based R2 use in `backend-school` and `frontend-school` with an authorized, provider-neutral File Platform that supports public/private objects, content inspection, malware scanning, immutable versions, retryable lifecycle operations, and future document-system attachments by file ID.

**Architecture:** `AppState` owns one `FilePlatform` application service. The service composes a code-owned purpose registry, domain access policies, a `StorageProvider` port, a `MalwareScanner` port, content inspection, and SQL repositories. Business modules pass tenant/actor/resource context and file IDs; only the R2 adapter knows bucket names or object keys. Existing file rows remain the logical identity while migration `030` adds lifecycle metadata and immutable version/operation tables.

**Tech stack:** Rust 2021, Axum 0.8, SQLx/PostgreSQL, async-trait, AWS S3 SDK for R2, Tokio TCP for clamd, image 0.25, Utoipa/OpenAPI, generated TypeScript API contracts, SvelteKit 5.

**Scope constraints:** Do not modify `backend-admin/` or `frontend-admin/`. Do not edit migrations `001`–`029`. Do not expose or log storage keys, bucket names, signed URLs, original filenames, file content, credentials, or national IDs.

---

## Task 1: Establish rollout preconditions and static boundaries

**Files:**

- Modify: `backend-school/tests/static_architecture.rs`

**Step 1: Audit active tenant file state**

Use the existing backend-admin internal school metadata and tenant database connection values from the runtime environment without printing credentials. For every active tenant, run read-only counts for:

```sql
SELECT
  (SELECT COUNT(*) FROM files) AS file_count,
  (SELECT COUNT(*) FROM files WHERE deleted_at IS NULL) AS active_file_count;
```

Also count references in `schools.logo_file_id`, `schools.banner_file_id`,
`admission_application_documents.file_id`, `admission_portal_documents.file_id`,
question-bank file relationship columns, `users.profile_image_url`, and
`achievements.image_path` after verifying their exact current table/column names
against migration `001`. Stop implementation if any object-bearing row exists; the
approved design requires an explicit inventory/copy procedure before delivery
behavior changes.

**Step 2: Write failing architecture guards**

Add tests that fail while any business module imports `services::r2_client::R2Client`, directly calls `R2Client::new`, or constructs a tenant object prefix. Allow the R2 symbol only in the provider adapter and provider-focused tests.

Add a guard that file API response structs do not contain fields named `storage_path`, `thumbnail_path`, `bucket`, `object_key`, or a persistent private provider URL.

**Step 3: Run the guards and confirm failure**

Run:

```bash
cd backend-school
cargo test --test static_architecture file_platform -- --nocapture
```

Expected: failure listing the current direct R2 consumers and legacy response fields.

**Step 4: Make the boundary executable during the compatibility window**

Replace the initial unconditional failure with an exact allowlist of the legacy
files reported by Step 3. The test must fail for any new direct R2 consumer or API
locator field that is not in that allowlist. Task 8 removes every allowlisted
entry and tightens the test to allow R2 only in the provider adapter. This keeps
the branch green without hiding the known migration work.

Rerun:

```bash
cd backend-school
cargo test --test static_architecture file_platform -- --nocapture
```

Expected: pass with only the exact compatibility allowlist.

**Step 5: Commit the executable boundary**

```bash
git add backend-school/tests/static_architecture.rs
git commit -m "test: define file platform boundaries"
```

## Task 2: Add the forward-only File Platform schema

**Files:**

- Create: `backend-school/migrations/030_file_platform.sql`
- Create: `backend-school/src/modules/files/schema_tests.rs`
- Modify: `backend-school/src/modules/files.rs`

**Step 1: Write failing migration assertions**

Add database-backed tests, skipped with an explicit message when `TEST_DATABASE_URL` is absent, that apply active migrations to isolated state and assert:

- `files` has `purpose_code`, `visibility`, `lifecycle_status`, `current_version_id`, `retention_class`, `delete_requested_at`, and `deleted_at`;
- `file_versions`, `file_derivatives`, and `file_operations` exist;
- `(file_id, version_number)` and `(provider_code, storage_class, object_key)` are unique;
- lifecycle/storage/scan/operation statuses are constrained;
- no existing migration is changed.

**Step 2: Run the focused test and confirm failure**

```bash
cd backend-school
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::files::schema_tests --bin backend-school -- --nocapture
```

**Step 3: Add migration `030`**

Create only `030_file_platform.sql`. Keep legacy columns for rollback compatibility. Add the new file columns nullable/defaulted where necessary for an additive rollout, create immutable version/derivative tables, create durable operation rows with attempts, lease fields, retry time, and bounded safe error codes, then add supporting indexes.

Do not store provider responses, signed URLs, filenames in operation errors, or a generic resource-type/resource-ID relationship.

**Step 4: Rerun focused migration tests**

Run the command from Step 2 and confirm all assertions pass.

**Step 5: Commit**

```bash
git add backend-school/migrations/030_file_platform.sql backend-school/src/modules/files.rs backend-school/src/modules/files/schema_tests.rs
git commit -m "feat: add file platform schema"
```

## Task 3: Implement purpose registry, stable keys, and provider-neutral types

**Files:**

- Create: `backend-school/src/modules/files/platform_types.rs`
- Create: `backend-school/src/modules/files/purpose_registry.rs`
- Modify: `backend-school/src/modules/files.rs`
- Modify: `backend-school/src/db/admin_client.rs`
- Modify: `backend-school/src/utils/tenant.rs`
- Modify: `backend-school/src/utils/request_context.rs`

**Step 1: Write failing unit tests**

Cover:

- every approved initial purpose resolves to server-owned domain, purpose segment, visibility, limits, scan requirement, derivative recipes, retention class, and policy key;
- unknown purposes fail;
- keys exactly follow `tenants/{tenant_uuid}/{domain}/{purpose}/{file_uuid}/v{version}/original.{ext}`;
- keys use the stable tenant UUID rather than subdomain;
- submitted filenames, person names, student/application identifiers, and national-ID-like values cannot enter generated keys;
- extensions come from detected content;
- private/public storage class cannot be supplied by an API request.

**Step 2: Run tests and confirm failure**

```bash
cd backend-school
cargo test modules::files::purpose_registry --bin backend-school
cargo test modules::files::platform_types --bin backend-school
```

**Step 3: Implement minimal types and registry**

Add typed enums/structs for purpose, visibility, lifecycle, storage class, detected content, derivative recipe, retention, provider object reference, and provider-neutral `DownloadGrant`.

Extend `SchoolDbInfo` parsing to retain the existing protected internal response `id`, and carry it as `tenant_id: Uuid` in `TenantContext`. Do not change backend-admin code or its response.

**Step 4: Rerun tests and request-context tests**

```bash
cd backend-school
cargo test modules::files:: --bin backend-school
cargo test utils::request_context::tests --bin backend-school
```

**Step 5: Commit**

```bash
git add backend-school/src/modules/files.rs backend-school/src/modules/files/platform_types.rs backend-school/src/modules/files/purpose_registry.rs backend-school/src/db/admin_client.rs backend-school/src/utils/tenant.rs backend-school/src/utils/request_context.rs
git commit -m "feat: define file purposes and stable tenant keys"
```

## Task 4: Introduce the StorageProvider port and dual-bucket R2 adapter

**Files:**

- Create: `backend-school/src/modules/files/storage_provider.rs`
- Create: `backend-school/src/modules/files/r2_storage_provider.rs`
- Modify: `backend-school/src/modules/files.rs`
- Delete after consumer migration: `backend-school/src/services/r2_client.rs`
- Modify: `backend-school/src/services.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`

**Step 1: Write fake-provider contract tests**

Define async operations:

```rust
async fn put(&self, object: &StoredObject, body: Bytes) -> Result<(), StorageError>;
async fn head(&self, object: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError>;
async fn delete(&self, object: &StoredObject) -> Result<(), StorageError>;
async fn private_download_grant(
    &self,
    object: &StoredObject,
    filename: &str,
    ttl: Duration,
) -> Result<DownloadGrant, StorageError>;
fn public_location(&self, object: &StoredObject) -> Result<Url, StorageError>;
```

Tests must prove public objects select only the public bucket, private objects select only the private bucket, delete is idempotent on not-found, grants have bounded expiry, and returned errors/log-safe variants contain no object key or signed URL.

**Step 2: Run tests and confirm failure**

```bash
cd backend-school
cargo test modules::files::storage_provider --bin backend-school
```

**Step 3: Implement the port and R2 adapter**

Build one S3-compatible client and route operations internally by `StorageClass`. Read `R2_PUBLIC_BUCKET_NAME` and `R2_PRIVATE_BUCKET_NAME`; never accept a bucket from callers. Use SDK presigning for private downloads and sanitized content disposition. Keep raw SDK errors behind safe error codes.

**Step 4: Rerun focused tests**

```bash
cd backend-school
cargo test modules::files::storage_provider --bin backend-school
cargo test modules::files::r2_storage_provider --bin backend-school
```

**Step 5: Commit**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock backend-school/src/modules/files.rs backend-school/src/modules/files/storage_provider.rs backend-school/src/modules/files/r2_storage_provider.rs backend-school/src/services.rs
git commit -m "feat: add provider-neutral dual-bucket storage"
```

## Task 5: Build content inspection and fail-closed malware scanning

**Files:**

- Create: `backend-school/src/modules/files/file_inspector.rs`
- Create: `backend-school/src/modules/files/malware_scanner.rs`
- Modify: `backend-school/src/modules/files.rs`
- Modify: `backend-school/src/utils/file_processor.rs`

**Step 1: Write failing inspector/scanner tests**

Use synthetic non-sensitive fixtures to verify:

- PNG, JPEG, WebP, and PDF signatures/structures are detected independent of submitted MIME/extension;
- spoofed MIME, unsupported data, oversize content, excessive dimensions, and excessive decoded pixels are rejected;
- safe canonical extensions are returned;
- clean, infected, unavailable, malformed-response, and timeout scanner results map to safe typed outcomes;
- all initial client-upload purposes require a clean scan before readiness.

**Step 2: Run tests and confirm failure**

```bash
cd backend-school
cargo test modules::files::file_inspector --bin backend-school
cargo test modules::files::malware_scanner --bin backend-school
```

**Step 3: Implement inspector and scanner port**

Decode supported images with explicit dimension and pixel ceilings before derivative work. Validate supported PDF header/trailer structure. Implement `MalwareScanner` plus a clamd `INSTREAM` adapter using bounded chunks, connect/read/write timeouts, and a maximum response size. Never log file data or scanner raw responses.

**Step 4: Rerun tests**

Run the commands from Step 2.

**Step 5: Commit**

```bash
git add backend-school/src/modules/files.rs backend-school/src/modules/files/file_inspector.rs backend-school/src/modules/files/malware_scanner.rs backend-school/src/utils/file_processor.rs
git commit -m "feat: inspect and scan file content"
```

## Task 6: Implement durable upload, delivery, deletion, and reconciliation

**Files:**

- Create: `backend-school/src/modules/files/repository.rs`
- Create: `backend-school/src/modules/files/platform_service.rs`
- Create: `backend-school/src/modules/files/reconciler.rs`
- Rewrite: `backend-school/src/modules/files/models.rs`
- Rewrite: `backend-school/src/modules/files/services.rs`
- Modify: `backend-school/src/modules/files.rs`
- Rewrite: `backend-school/src/services/cleaner.rs`
- Modify: `backend-school/src/main.rs`

**Step 1: Write failure-injection tests**

With fake provider/scanner/repository implementations, cover:

- successful upload transitions `processing -> ready`;
- scanner infected/unavailable/timeout never writes a public object and never reaches ready;
- provider put failure leaves durable failed metadata/repair work;
- metadata finalize failure leaves a deterministic operation for reconciliation;
- required derivative failure keeps the file non-ready;
- optional derivative failure queues only derivative retry;
- private/public delivery rejects non-ready files;
- delete immediately revokes delivery, is idempotent, retries provider failure, and reaches deleted only after all objects are absent;
- leases prevent duplicate workers and expired leases can be reclaimed;
- retry backoff is bounded and terminal failures remain observable.

**Step 2: Run tests and confirm failure**

```bash
cd backend-school
cargo test modules::files::platform_service --bin backend-school
cargo test modules::files::reconciler --bin backend-school
```

**Step 3: Implement repositories and orchestration**

`FilePlatform` receives `Arc<dyn StorageProvider>` and `Arc<dyn MalwareScanner>`. It accepts authoritative `TenantContext`, actor/resource policy context, purpose, filename, and bounded bytes. It owns metadata, scan, immutable key/version, derivatives, finalization, grants, and delete requests.

The repository uses transactions for metadata relationships and `file_operations`. Object-store calls occur outside open DB transactions. Use deterministic operation records so partial failures can be retried.

Replace cleaner behavior with operation reconciliation. Remove path logging and swallowed provider failures.

**Step 4: Integrate into AppState**

Construct production adapters once at startup. Add background reconciliation over cached tenant pools. Readiness validates required File Platform configuration and scanner connectivity without exposing secrets.

**Step 5: Rerun focused tests**

Run the commands from Step 2 plus:

```bash
cd backend-school
cargo test modules::system::handlers::health --bin backend-school
```

**Step 6: Commit**

```bash
git add backend-school/src/main.rs backend-school/src/modules/files.rs backend-school/src/modules/files/models.rs backend-school/src/modules/files/services.rs backend-school/src/modules/files/repository.rs backend-school/src/modules/files/platform_service.rs backend-school/src/modules/files/reconciler.rs backend-school/src/services/cleaner.rs
git commit -m "feat: add durable file lifecycle orchestration"
```

## Task 7: Enforce domain policies and publish typed file APIs

**Files:**

- Create: `backend-school/src/policies/file_access_policy.rs`
- Modify: `backend-school/src/policies.rs`
- Rewrite: `backend-school/src/modules/files/handlers.rs`
- Modify: `backend-school/src/modules/files/models.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify generated: `contracts/openapi/school-api.json`
- Modify generated: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/files.ts`
- Create: `frontend-school/tests/static/file-platform-contract.test.mjs`

**Step 1: Write failing access-policy and API tests**

Cover allowed/denied create/read/delete for own profile, school branding, admission staff/application sessions, and question-bank ownership/scope. Add cross-tenant, cross-user, and unrelated-resource denials.

Static/API contract tests require:

- `POST /api/files`, `GET /api/files/{id}`, `POST /api/files/{id}/download`, `DELETE /api/files/{id}`, and `GET /api/public/files/{id}/content`;
- typed `ApiResponse` envelopes except redirects/binary responses;
- no provider/object/storage-path fields;
- public content only when ready/public;
- private delivery requires authenticated policy and returns an ephemeral redirect/grant;
- frontend helper imports generated DTOs and treats file ID as identity.

**Step 2: Run tests and confirm failure**

```bash
cd backend-school
cargo test policies::file_access_policy --bin backend-school
cargo test api_contract::tests -- --nocapture
cd ../frontend-school
node --test tests/static/file-platform-contract.test.mjs
```

**Step 3: Implement policy and handlers**

Handlers perform request context, registered domain policy, `FilePlatform`, and typed response only. Stream multipart chunks into a purpose-bounded buffer. Clients may submit purpose and an allowed domain resource identifier but not visibility, owner, key, bucket, or lifecycle state.

Use existing generated domain permission constants where they express the approved authority. Add permission-contract entries only if no exact domain permission exists; if added, include migration data and run the full permission generation matrix.

**Step 4: Generate API contracts and consume generated DTOs**

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Update `files.ts` over the generated DTOs without `unknown`, response casts, storage paths, or permanent URLs.

**Step 5: Rerun focused tests and commit**

```bash
git add backend-school/src/policies.rs backend-school/src/policies/file_access_policy.rs backend-school/src/modules/files/handlers.rs backend-school/src/modules/files/models.rs backend-school/src/main.rs backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts frontend-school/src/lib/api/files.ts frontend-school/tests/static/file-platform-contract.test.mjs
git commit -m "feat: expose authorized typed file APIs"
```

## Task 8: Migrate all backend file consumers

**Files:**

- Modify: `backend-school/src/modules/school/handlers.rs`
- Modify: `backend-school/src/modules/school/services.rs`
- Modify: `backend-school/src/modules/admission/handlers/applications.rs`
- Modify: `backend-school/src/modules/admission/handlers/portal.rs`
- Modify: `backend-school/src/modules/admission/services/application_service.rs`
- Modify: `backend-school/src/modules/admission/services/portal_service.rs`
- Modify: `backend-school/src/modules/admission/services/round_service.rs`
- Modify: `backend-school/src/modules/question_bank/handlers.rs`
- Modify: `backend-school/src/modules/question_bank/services.rs`
- Modify: `backend-school/src/modules/school/models.rs`
- Delete: `backend-school/src/services/r2_client.rs`
- Modify: `backend-school/src/services.rs`

**Step 1: Add integration-focused tests**

Add or extend service tests that assert domain rows keep file IDs, domain transaction finalization owns the relationship, replacement requests deletion of the previous file, and download inherits the domain resource policy.

**Step 2: Run tests and confirm legacy failures**

```bash
cd backend-school
cargo test modules::school --bin backend-school
cargo test modules::admission --bin backend-school
cargo test modules::question_bank --bin backend-school
cargo test modules::achievement --bin backend-school
```

**Step 3: Migrate one domain at a time**

Replace every direct provider call, key builder, and provider URL/path with File Platform operations and file IDs. Preserve domain authorization and transaction boundaries. Do not swallow cleanup errors; persist retryable deletion work.

For public display, return `/api/public/files/{id}/content`. For private downloads, return the platform download endpoint or trigger it through the typed helper.

**Step 4: Prove direct coupling is gone**

```bash
rg -n "R2Client|storage_path|thumbnail_path|R2_BUCKET_NAME|build_.*file.*url" backend-school/src/modules backend-school/src/services
cd backend-school
cargo test --test static_architecture file_platform -- --nocapture
```

Expected: no business-module R2 use and no API-facing storage locator. Any compatibility-only DB column use must be confined to the File Platform repository with a documented removal path.

**Step 5: Commit**

```bash
git add backend-school/src
git commit -m "refactor: route file consumers through platform"
```

## Task 9: Migrate frontend-school consumers to file IDs

**Files:**

- Modify: `frontend-school/src/lib/components/forms/ProfileImageUpload.svelte`
- Modify: `frontend-school/src/lib/components/achievement/AchievementDialog.svelte`
- Modify: `frontend-school/src/lib/components/achievement/AchievementCard.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/achievements/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/school-settings/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/view/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/question-bank/+page.svelte`
- Modify: `frontend-school/tests/static/file-platform-contract.test.mjs`

**Step 1: Extend the failing frontend contract test**

Assert no frontend consumer reads `storage_path`/`thumbnail_path`, constructs `/api/files?path=`, or passes the old client-selected `file_type`. Assert public assets use file-ID content routes and private access uses the typed download helper.

**Step 2: Run the focused test and confirm failure**

```bash
cd frontend-school
node --test tests/static/file-platform-contract.test.mjs
```

**Step 3: Update Svelte consumers**

Use the required Svelte documentation/analyzer tooling for every edited `.svelte` file. Store and pass file IDs. Map approved UI actions to purpose codes inside typed helpers. Do not add provider/storage knowledge to components.

**Step 4: Verify Svelte and static behavior**

```bash
cd frontend-school
node --test tests/static/file-platform-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
```

Resolve every Svelte analyzer issue before continuing.

**Step 5: Commit**

```bash
git add frontend-school/src frontend-school/tests/static/file-platform-contract.test.mjs
git commit -m "refactor: use file IDs in school frontend"
```

## Task 10: Configure scanner, buckets, readiness, and operations

**Files:**

- Modify: `backend-school/.env.example`
- Modify: `backend-school/docker-compose.yml`
- Modify: `docker-compose.yml`
- Modify: `podman-compose.yml`
- Modify: `backend-school/src/modules/system/handlers/health.rs`
- Modify: `docs/OPERATIONS.md`
- Modify: `docs/TESTING.md`
- Modify: `.github/workflows/deploy-backend-school.yml`

**Step 1: Write failing configuration/readiness tests**

Test missing/placeholder public bucket, private bucket, public URL, scanner endpoint, invalid grant TTL, and invalid retry/lease values. Production configuration must fail closed; deterministic test adapters remain injectable.

**Step 2: Add executable topology**

Add a pinned clamd-compatible container with healthcheck, persistent signature volume, bounded resources, and internal-only networking. Add:

- `R2_PUBLIC_BUCKET_NAME`
- `R2_PRIVATE_BUCKET_NAME`
- `R2_PUBLIC_URL`
- `CLAMD_ENDPOINT`
- scanner timeouts/connection limits
- private grant TTL
- reconciliation retry/lease settings

Retain `R2_BUCKET_NAME` only as a temporary documented compatibility input if required for one safe rollout; the application must not silently use one public bucket for private files.

**Step 3: Provision and verify the private R2 bucket**

Use the configured Cloudflare credentials without printing them. Preserve the existing public bucket. Create the explicitly named private bucket only after confirming it does not exist; do not enable public access for it. Verify both buckets through read-only API calls.

**Step 4: Document durable operations**

Update `docs/OPERATIONS.md` with bucket/scanner rollout, readiness, reconciliation, failure recovery, and rollback. Update `docs/TESTING.md` with authenticated public/private upload/download/delete smoke commands and scanner failure tests. Do not create another long-lived Markdown file.

**Step 5: Commit**

```bash
git add backend-school/.env.example backend-school/docker-compose.yml docker-compose.yml podman-compose.yml backend-school/src/modules/system/handlers/health.rs docs/OPERATIONS.md docs/TESTING.md .github/workflows/deploy-backend-school.yml
git commit -m "ops: configure secure file platform runtime"
```

## Task 11: Full verification, rollout, and cleanup

**Files:**

- Modify: `TODO.md`
- Delete after implementation is recorded: `docs/superpowers/specs/2026-07-28-file-platform-design.md`
- Delete after implementation is recorded: `docs/superpowers/plans/2026-07-28-file-platform.md`

**Step 1: Run focused and contract checks**

```bash
cd backend-school
cargo test modules::files --bin backend-school
cargo test policies::file_access_policy --bin backend-school
cargo test api_contract::tests -- --nocapture
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check

cd ../frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run check:permissions
npm run test:permissions
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

If permissions were changed, regenerate and verify them before these checks.

**Step 2: Review boundary searches**

```bash
rg -n "R2Client|R2_BUCKET_NAME|/api/files\\?path=|storage_path|thumbnail_path" backend-school/src/modules frontend-school/src
rg -n "object_key|bucket_name|signed.*url" backend-school/src/modules/files/handlers.rs backend-school/src/modules/files/models.rs contracts/openapi/school-api.json frontend-school/src/lib/api/generated
git diff --check
git status --short
git diff --stat
git diff
```

Explain every remaining match; no business/UI coupling or API leak is allowed.

**Step 3: Apply migration and deploy in order**

1. Confirm the tenant audit still reports zero file rows.
2. Ensure the private bucket and healthy scanner are available.
3. Deploy backend-school with new secrets/configuration.
4. Wait for `/ready`.
5. Confirm migration `030` is applied to sandbox.
6. Deploy frontend-school only after backend readiness.

Do not update the production server compose destructively or restart unrelated services.

**Step 4: Run deployed smoke**

With credentials supplied only at runtime:

```bash
SMOKE_SUBDOMAIN=sandbox \
SMOKE_USERNAME="$SMOKE_USERNAME" \
SMOKE_PASSWORD="$SMOKE_PASSWORD" \
./scripts/smoke_test.sh
```

Then exercise one small public image and one small private PDF through upload, metadata, delivery, and idempotent delete. Confirm anonymous private access fails, public access works only while ready, deletion revokes both, logs contain no object locator/signed URL/filename/content, and `/api/auth/me` still passes.

**Step 5: Update the backlog**

Remove completed SEC-005/File Platform work from `TODO.md`. Keep only genuinely unfinished follow-up items, such as later legacy-column removal after a verified compatibility window.

**Step 6: Final workflow-artifact cleanup and commit**

After commits/PR history preserve the approved design and plan, remove the two Superpowers artifacts as required by `.rules`, then:

```bash
git add TODO.md docs/superpowers/specs/2026-07-28-file-platform-design.md docs/superpowers/plans/2026-07-28-file-platform.md
git commit -m "docs: close file platform backlog"
```

**Step 7: Final evidence**

Report exact commands and outcomes, any external check that was skipped and why, migration/deployment run links, the new configuration names (never values), and whether rollback remains additive.
