# Certificate Campaign Permanent Purge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ให้ผู้มีสิทธิ์ลบกิจกรรมเกียรติบัตรทุกสถานะได้ถาวร โดยปิดการเข้าถึงทันที ลบ storage object แบบ retry ได้ แล้ว hard-delete domain, audit และ File Platform metadata เมื่อปลอดภัย

**Architecture:** ใช้ migration `039` เพิ่มสถานะ `purging`, durable purge job/file inventory และ database finalizer ที่มี transaction-local guard ระยะแรก lock campaign และ queue File Platform deletion ใน transaction เดียวกัน ส่วน immediate worker และ reconciler เดิมทำ provider deletion ก่อนเรียก finalizer แบบ idempotent Frontend ใช้ impact snapshot, exact-name confirmation และ polling progress ผ่าน generated API contract

**Tech Stack:** PostgreSQL migrations/triggers/functions, Rust/Axum/SQLx, File Platform reconciler, generated utoipa OpenAPI/TypeScript/permission contracts, SvelteKit 5 runes, shadcn-svelte, Node static tests และ Playwright

**Spec:** `docs/superpowers/specs/2026-08-22-certificate-campaign-hard-purge-design.md`

## Global Constraints

- อ่านและยึด `.rules`; ห้ามแก้ migration `001`–`038` ที่ใช้แล้ว
- ห้ามเก็บหรือ log plaintext national ID, recipient values, QR proof, object key, signed URL หรือ raw provider error
- ใช้ permission เดิม `certificate.delete.school` และ `certificate.delete.organization_unit`; organization-unit scope ต้องตรง exact owner เท่านั้น
- ลบ endpoint `DELETE /api/certificates/campaigns/{campaignId}` เดิมโดยไม่มี compatibility shim
- storage object ทุก version/derivative ต้องยืนยันว่า deleted ก่อน hard-delete File Platform metadata
- application Rust ห้ามมี `DELETE FROM files`; hard-delete metadata ทำใน guarded database finalizer เท่านั้น
- purge ไม่มี undo, grace period, durable completion tombstone หรือ durable purge audit
- campaign number/activity counters ไม่ลดและไม่ reuse
- ใช้ generated permission/API contracts และห้ามแก้ generated files ด้วยมือ
- ทุก Svelte file ที่แก้ต้องผ่าน `svelte-autofixer`
- รันทดสอบทีละคำสั่ง ใช้ `--test-threads=1`, `--test-concurrency=1` หรือ `--workers=1` เมื่อเครื่องมือรองรับ

---

## File Structure

### New files

- `backend-school/migrations/039_certificate_campaign_purge.sql` — schema, permission descriptions, purge guards และ atomic database finalizer
- `backend-school/src/modules/certificates/services/purge_service.rs` — impact, authorization, start/status/retry, provider completion orchestration และ reconciler finalization
- `frontend-school/src/lib/components/certificates/CertificateCampaignPurgeDialog.svelte` — impact confirmation, progress polling และ retry UI
- `frontend-school/tests/e2e/certificate-campaign-purge.spec.ts` — isolated component/browser coverage ด้วย API stubs

### Primary modified files

- `backend-school/src/modules/certificates/models.rs` — typed purge DTOs และ `purging` status
- `backend-school/src/modules/certificates/services.rs` — export purge service
- `backend-school/src/modules/certificates/schema_tests.rs` — migration/trigger/finalizer DB tests
- `backend-school/src/modules/certificates/services_tests.rs` — permission, impact, idempotency, retry และ concurrency tests
- `backend-school/src/modules/files/repository.rs` — finalize zero-object deletion in caller-owned transaction
- `backend-school/src/services/cleaner.rs` — advance certificate purge jobs after each reconciliation pass
- `backend-school/src/modules/certificates/services/{campaign,template,candidate,request,issuance,render,verification}_service.rs` — lock-order and `purging` visibility/mutation guards
- `backend-school/src/modules/files/consumer_service.rs` — serialize certificate upload ownership with campaign purge
- `backend-school/src/policies/{certificate_access_policy,file_access_policy}.rs` — hide/reject purging template resources and preserve campaign→file lock order
- `backend-school/src/modules/certificates/handlers.rs`, `backend-school/src/app.rs`, `backend-school/src/api_contract.rs` — four typed purge endpoints and removal of draft delete
- `contracts/permissions.json`, generated permission artifacts — permanent-delete descriptions
- `contracts/openapi/school-api.json`, `frontend-school/src/lib/api/generated/school-api.ts` — generated purge API contract
- `frontend-school/src/lib/api/certificates.ts` — concrete generated purge wrappers
- `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/overview/+page.svelte` — permanent danger zone
- `frontend-school/src/routes/(app)/staff/certificates/+page.svelte` และ `frontend-school/src/lib/components/certificates/CertificateCampaignList.svelte` — purging progress entry and completion patch
- `frontend-school/tests/static/certificate-workspace.test.mjs` — contract/capability guard
- `frontend-school/tests/e2e/certificate-lifecycle.spec.ts` — destructive cleanup through purge
- `backend-school/tests/static_architecture.rs` — guarded purge is the sole hard-delete exception
- `docs/TESTING.md`, `docs/OPERATIONS.md` — live test and recovery procedure

---

### Task 1: Forward-Only Purge Schema and Guarded Finalizer

**Files:**
- Create: `backend-school/migrations/039_certificate_campaign_purge.sql`
- Modify: `backend-school/src/modules/certificates/schema_tests.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Produces: `certificate_campaign_purge_jobs`, `certificate_campaign_purge_files`
- Produces: SQL function `finalize_certificate_campaign_purge(UUID) RETURNS BOOLEAN`
- Produces: status value `certificate_campaigns.status = 'purging'`
- Consumes later: service calls `SELECT finalize_certificate_campaign_purge($1)` only after file metadata reports deleted

- [ ] **Step 1: Write failing migration structure tests**

Add a static test that reads migration `039` and requires the two tables, the new status, guard functions, permission description updates and finalizer. Update the old “permanent” architecture assertion to preserve migration `035` unchanged while requiring a narrow guarded exception in `039`:

```rust
#[test]
fn certificate_campaign_purge_is_forward_only_guarded_and_file_complete() {
    let migration = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/039_certificate_campaign_purge.sql"),
    )
    .expect("migration 039 must exist");
    for required in [
        "CREATE TABLE certificate_campaign_purge_jobs",
        "CREATE TABLE certificate_campaign_purge_files",
        "'purging'",
        "finalize_certificate_campaign_purge",
        "certificate_campaign_purge_guard_allows",
        "certificate_file_purge_guard_allows",
        "file_versions_prevent_deletion",
        "file_derivatives_prevent_deletion",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }
    assert!(!migration.to_ascii_lowercase().contains("national_id"));
}
```

- [ ] **Step 2: Run the focused static test and verify RED**

Run:

```bash
./scripts/test_backend_school.sh modules::certificates::schema_tests::certificate_campaign_purge_is_forward_only_guarded_and_file_complete -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration `039` does not exist.

- [ ] **Step 3: Add migration schema and permission updates**

Create migration `039` with these exact state tables and constraints:

```sql
ALTER TABLE certificate_campaigns
    DROP CONSTRAINT certificate_campaigns_status_check,
    ADD CONSTRAINT certificate_campaigns_status_check
        CHECK (status IN ('draft', 'active', 'closed', 'archived', 'purging'));

CREATE TABLE certificate_campaign_purge_jobs (
    campaign_id UUID PRIMARY KEY REFERENCES certificate_campaigns(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('deleting_files', 'failed', 'finalizing')),
    requested_by UUID REFERENCES users(id) ON DELETE SET NULL,
    template_count BIGINT NOT NULL CHECK (template_count >= 0),
    candidate_count BIGINT NOT NULL CHECK (candidate_count >= 0),
    request_count BIGINT NOT NULL CHECK (request_count >= 0),
    open_request_count BIGINT NOT NULL CHECK (open_request_count >= 0),
    issued_certificate_count BIGINT NOT NULL CHECK (issued_certificate_count >= 0),
    revoked_certificate_count BIGINT NOT NULL CHECK (revoked_certificate_count >= 0),
    file_count BIGINT NOT NULL CHECK (file_count >= 0),
    total_file_bytes BIGINT NOT NULL CHECK (total_file_bytes >= 0),
    last_error_code VARCHAR(64) CHECK (
        last_error_code IS NULL OR last_error_code ~ '^[a-z0-9_]{1,64}$'
    ),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE certificate_campaign_purge_files (
    campaign_id UUID NOT NULL REFERENCES certificate_campaign_purge_jobs(campaign_id)
        ON DELETE CASCADE,
    file_id UUID NOT NULL UNIQUE REFERENCES files(id) ON DELETE CASCADE,
    object_count INTEGER NOT NULL CHECK (object_count >= 0),
    byte_size BIGINT NOT NULL CHECK (byte_size >= 0),
    PRIMARY KEY (campaign_id, file_id)
);
```

Update only the existing permission rows’ Thai `name`/`description`; do not change codes or grants.

- [ ] **Step 4: Implement narrow trigger guards**

Create `certificate_campaign_purge_guard_allows(UUID)` and `certificate_file_purge_guard_allows(UUID)`. Both require a matching transaction-local campaign ID, campaign status `purging`, and job status `finalizing`. Replace existing immutable trigger functions through `CREATE OR REPLACE FUNCTION` so only `DELETE` under that guard passes. Add guarded `BEFORE DELETE` triggers to `certificate_campaigns` and `files`.

For `enforce_certificate_snapshot_immutability`, permit only setting `replacement_for_certificate_id` and `replaced_by_certificate_id` to null under the guard; compare the remainder with `to_jsonb(NEW)`/`to_jsonb(OLD)` before returning.

- [ ] **Step 5: Implement the atomic finalizer**

`finalize_certificate_campaign_purge(p_campaign_id UUID)` must lock campaign/job/files, verify every inventory logical file and object metadata is `deleted`, set the local guard, delete campaign-scoped audit rows, clear candidate/certificate cycles, then delete rows in this order:

```text
certificate_issue_run_problems
certificate_candidate_issue_locks
certificate_issue_request_items
certificates
certificate_issue_runs
certificate_issue_requests
certificate_candidates
certificate_import_batches
certificate_template_assets
certificate_template_file_uploads
certificate_templates
file_operations
file_derivatives
file_versions
files
certificate_campaigns (cascades purge job last)
```

The function must set `files.current_version_id = NULL` before deleting versions and must raise an integrity exception before any delete if a file/object is not confirmed deleted.

- [ ] **Step 6: Add database behavior tests**

Add isolated DB tests that prove:

```rust
assert!(sqlx::query("DELETE FROM certificates WHERE id = $1")
    .bind(certificate_id).execute(&pool).await.is_err());
assert!(sqlx::query("DELETE FROM certificate_campaigns WHERE id = $1")
    .bind(campaign_id).execute(&pool).await.is_err());

let finalized: bool = sqlx::query_scalar(
    "SELECT finalize_certificate_campaign_purge($1)"
).bind(campaign_id).fetch_one(&pool).await.unwrap();
assert!(finalized);
```

The successful fixture must include audit rows, open request locks/items, an issue run problem, issued/revoked/replacement certificates, a background, image/font assets, versions, derivatives and operations. Assert all campaign/file/audit/purge rows are absent and `certificate_academic_year_counters.next_activity_sequence` is unchanged. Add a second fixture where storage status is not deleted and assert the whole finalizer rolls back.

- [ ] **Step 7: Run schema tests and verify GREEN**

```bash
./scripts/test_backend_school.sh modules::certificates::schema_tests -- --nocapture --test-threads=1
```

- [ ] **Step 8: Commit the schema slice**

```bash
git add backend-school/migrations/039_certificate_campaign_purge.sql backend-school/src/modules/certificates/schema_tests.rs backend-school/tests/static_architecture.rs
git commit -m "feat(certificates): add guarded campaign purge schema"
```

---

### Task 2: Purge Domain Service and Durable File Recovery

**Files:**
- Create: `backend-school/src/modules/certificates/services/purge_service.rs`
- Modify: `backend-school/src/modules/certificates/services.rs`
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/files/repository.rs`
- Modify: `backend-school/src/services/cleaner.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `backend-school/src/modules/files/schema_tests.rs`

**Interfaces:**
- Produces: `purge_service::impact`, `start`, `status`, `retry`, `reconcile_pending_purges`
- Consumes: `SqlFileRepository::request_delete_in_transaction`, `FilePlatform::complete_prepared_delete`
- Produces DTOs: `CertificateCampaignPurgeCounts`, `CertificateCampaignPurgeImpact`, `StartCertificateCampaignPurgeRequest`, `CertificateCampaignPurgePhase`, `CertificateCampaignPurgeStatus`

- [ ] **Step 1: Add failing DTO serialization/unit tests**

Define the wire shapes before service code:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignPurgeCounts {
    pub template_count: i64,
    pub candidate_count: i64,
    pub request_count: i64,
    pub open_request_count: i64,
    pub issued_certificate_count: i64,
    pub revoked_certificate_count: i64,
    pub file_count: i64,
    pub total_file_bytes: i64,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartCertificateCampaignPurgeRequest {
    pub confirmation_name: String,
    pub expected_updated_at: DateTime<Utc>,
    pub expected_impact: CertificateCampaignPurgeCounts,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignPurgeImpact {
    pub campaign_id: Uuid,
    pub campaign_name: String,
    pub updated_at: DateTime<Utc>,
    pub counts: CertificateCampaignPurgeCounts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CertificateCampaignPurgePhase {
    DeletingFiles,
    Failed,
    Finalizing,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignPurgeStatus {
    pub campaign_id: Uuid,
    pub phase: CertificateCampaignPurgePhase,
    pub file_count: i64,
    pub deleted_file_count: i64,
    #[schema(required = true)]
    pub last_error_code: Option<String>,
}
```

`CertificateCampaignPurgePhase` serializes `deleting_files`, `failed`, `finalizing`, `completed`; the impact keeps counts nested under `counts` consistently in GET and POST.

- [ ] **Step 2: Run the focused model test and verify RED**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::purge_contract_serializes_camel_case_and_rejects_unknown_fields -- --exact --nocapture --test-threads=1
```

- [ ] **Step 3: Fix zero-object File Platform deletion**

Call the existing `finalize_deleted_if_absent` inside `request_delete_in_transaction` after durable delete operations are inserted. Add a DB test that creates a logical file with no version, requests delete in a caller-owned transaction, commits, and asserts lifecycle `deleted` with empty work.

- [ ] **Step 4: Implement impact and frozen inventory queries**

`impact(pool, actor, campaign_id)` reads campaign/owner and all counts from one statement snapshot, then enforces `CertificateAction::Delete`. File IDs are the union of template background, template assets and `certificate_template_file_uploads`; total bytes count only non-deleted original/derivative objects.

The start transaction must lock campaign first, recheck exact owner permission, exact `confirmation_name`, `expected_updated_at`, counts, purpose codes, non-shared references and non-`legal_hold` retention. Insert job/inventory, set campaign `purging`, then call `request_delete_in_transaction` for sorted file IDs before commit.

- [ ] **Step 5: Implement start and immediate provider completion**

Use these exact service signatures:

```rust
pub async fn impact(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<CertificateCampaignPurgeImpact, AppError>;

pub async fn start(
    pool: &PgPool,
    actor: &ActorContext,
    platform: &FilePlatform,
    campaign_id: Uuid,
    request: StartCertificateCampaignPurgeRequest,
) -> Result<CertificateCampaignPurgeStatus, AppError>;
```

After commit call `complete_prepared_delete`; provider failures must remain durable and be logged only by campaign/file IDs plus `log_safe_code()`. Then call `advance_one` so zero/small inventories may finalize immediately. A duplicate start for a valid existing job returns its status without a second job.

- [ ] **Step 6: Implement status, retry and background advancement**

```rust
pub async fn status(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<CertificateCampaignPurgeStatus, AppError>;

pub async fn retry(
    pool: &PgPool,
    actor: &ActorContext,
    platform: &FilePlatform,
    campaign_id: Uuid,
) -> Result<CertificateCampaignPurgeStatus, AppError>;

pub async fn reconcile_pending_purges(pool: &PgPool) -> Result<usize, AppError>;
```

`advance_one` transitions to `finalizing` only when every inventory file is `deleted`, then calls the SQL finalizer. Terminal delete operations with no active retry mark the job `failed` using a bounded safe code. Retry calls `request_delete_in_transaction` again, which creates new operations only for failed targets; if all files are gone it calls finalizer directly.

Update `FileCleaner::reconcile_file_operations` to call `reconcile_pending_purges(self.repository.pool())` after the File Platform pass and log only aggregate count/error code.

- [ ] **Step 7: Add service and recovery tests**

Cover school/exact-unit allow/deny, confirmation mismatch, stale counts, open request inclusion, soft-deleted candidate inclusion, shared/legal-hold rejection, duplicate start, zero-file immediate completion, provider failure retained job, terminal failure→retry, and finalizer retry. Use test providers already present in File Platform tests; never put object keys in assertion messages.

- [ ] **Step 8: Run focused service/file tests and verify GREEN**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::purge -- --nocapture --test-threads=1
```

Then:

```bash
./scripts/test_backend_school.sh modules::files -- --nocapture --test-threads=1
```

- [ ] **Step 9: Commit the service slice**

```bash
git add backend-school/src/modules/certificates backend-school/src/modules/files/repository.rs backend-school/src/modules/files/schema_tests.rs backend-school/src/services/cleaner.rs
git commit -m "feat(certificates): orchestrate durable campaign purge"
```

---

### Task 3: Purging Visibility, Mutation Rejection, and Lock Order

**Files:**
- Modify: `backend-school/src/modules/certificates/services/campaign_service.rs`
- Modify: `backend-school/src/modules/certificates/services/template_service.rs`
- Modify: `backend-school/src/modules/certificates/services/candidate_service.rs`
- Modify: `backend-school/src/modules/certificates/services/request_service.rs`
- Modify: `backend-school/src/modules/certificates/services/issuance_service.rs`
- Modify: `backend-school/src/modules/certificates/services/render_service.rs`
- Modify: `backend-school/src/modules/certificates/services/verification_service.rs`
- Modify: `backend-school/src/policies/certificate_access_policy.rs`
- Modify: `backend-school/src/policies/file_access_policy.rs`
- Modify: `backend-school/src/modules/files/consumer_service.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`

**Interfaces:**
- Consumes: `CertificateCampaignStatus::Purging`
- Produces: every mutation uses campaign→child/file lock order and returns conflict/not-found for purging resources
- Produces: ordinary readers exclude purging; campaign list includes it only for a matching delete scope

- [ ] **Step 1: Add failing visibility and race tests**

Add DB service tests that set a campaign to `purging` with a valid job and assert using prepared fixture values `tenant_id`, `verification_attempt`, `template_id`, and `update_request`:

```rust
let verification_result = verification_service::verify(
    &pool,
    tenant_id,
    verification_attempt,
)
.await;
assert!(matches!(
    verification_result,
    Err(AppError::NotFound(_))
));

let template_result = template_service::update_template(
    &pool,
    &actor,
    template_id,
    update_request,
)
.await;
assert!(matches!(
    template_result,
    Err(AppError::NotFound(_)) | Err(AppError::Conflict(_))
));
```

Cover own certificates, public manual/QR, render manifests, template/candidate/request reads, revoke and issue. Add a two-connection test where issuance wins the campaign lock and changes impact, and one where purge wins so issuance creates no run/certificate.

- [ ] **Step 2: Run the purging visibility tests and verify RED**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::purging -- --nocapture --test-threads=1
```

- [ ] **Step 3: Update status parsing and campaign capabilities**

Add `Purging` to `CertificateCampaignStatus`. Manual status change must explicitly reject target `Purging`. Normal `get_campaign` hides purging. `list_campaigns` unions the normal read scope with a condition that purging rows are returned only when the actor also has matching delete scope. `can_delete` becomes true for every non-purging campaign in delete scope, regardless of open requests or issued history; all mutation/download capabilities become false for purging rows.

- [ ] **Step 4: Enforce campaign-first mutation locks**

Change template lock helper to return owner plus status and reject `purging`. Candidate paths already lock campaign; make their pre-read helper hide purging and keep locked status validation. Reorder request transitions and issuance/revocation so they identify campaign, lock campaign, reject purging, then lock request/certificate/items.

Use a single safe conflict code for post-lock mutation rejection:

```rust
fn require_campaign_not_purging(status: &str) -> Result<(), AppError> {
    if status == "purging" {
        Err(AppError::Conflict("certificate_campaign_purging".to_string()))
    } else {
        Ok(())
    }
}
```

- [ ] **Step 5: Serialize certificate uploads and generic deletes**

Make `record_certificate_template_upload` start a transaction, lock the campaign through template, reject `purging`, insert the relation, and commit. This guarantees uploads that lose the race are compensated by the existing file cleanup path.

In certificate template file delete guard lock campaign before template/file and reject purging. Add campaign status filters to certificate file read/grant policy so no new grant is issued after purge starts.

- [ ] **Step 6: Hide purging from all normal reads**

Add `campaign.status <> 'purging'` to common template/request/certificate access selects, candidate access, own/admin certificate lists, verification and all render manifest queries. Public paths return generic not-found; authenticated ordinary paths return not-found without exposing progress.

- [ ] **Step 7: Run focused race/visibility suites and verify GREEN**

```bash
./scripts/test_backend_school.sh modules::certificates -- --nocapture --test-threads=1
```

- [ ] **Step 8: Commit the lifecycle guard slice**

```bash
git add backend-school/src/modules/certificates backend-school/src/modules/files/consumer_service.rs backend-school/src/policies
git commit -m "fix(certificates): isolate campaigns while purging"
```

---

### Task 4: Typed HTTP API, Permission Contract, and Generated Artifacts

**Files:**
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/app.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `contracts/permissions.json`
- Generate: `contracts/permissions.lock.json`
- Generate: `backend-school/src/permissions/registry_generated.rs`
- Generate: `frontend-school/src/lib/permissions/registry.generated.ts`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Produces four endpoints: purge-impact, purge, purge-status, purge/retry
- Removes old DELETE campaign operation and generated client path

- [ ] **Step 1: Add failing OpenAPI and route assertions**

Update API contract tests to require operation IDs:

```text
getCertificateCampaignPurgeImpact
startCertificateCampaignPurge
getCertificateCampaignPurgeStatus
retryCertificateCampaignPurge
```

Assert `/api/certificates/campaigns/{campaign_id}` has `get` and `put` but no `delete`.

- [ ] **Step 2: Run the exact API contract test and verify RED**

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::certificate_contract_registers_campaign_purge -- --exact --nocapture --test-threads=1
```

- [ ] **Step 3: Add thin handlers and routes**

Handlers resolve actor tenant context and delegate only:

```rust
let result = purge_service::start(
    &context.tenant.pool,
    &context.actor,
    state.file_platform.as_ref(),
    campaign_id,
    payload,
).await?;
Ok((StatusCode::ACCEPTED, Json(ApiResponse::ok(result))).into_response())
```

Register the four routes in `app.rs`, remove `.delete(delete_certificate_campaign)`, delete the old handler/imports and `campaign_service::delete_campaign`, and register all paths/schemas with utoipa.

- [ ] **Step 4: Update permission descriptions and generate permissions**

Change only names/descriptions for the two existing delete codes in `contracts/permissions.json` to permanent campaign deletion. Run:

```bash
npm run generate:permissions
```

from `frontend-school`; do not hand-edit generated outputs.

- [ ] **Step 5: Generate API artifacts**

From `frontend-school` run:

```bash
npm run generate:api-contracts
```

Confirm the old DELETE path is absent and purge DTO fields are camelCase.

- [ ] **Step 6: Run contract checks sequentially**

```bash
npm run check:permissions
```

```bash
npm run test:permissions
```

```bash
npm run check:api-contracts
```

```bash
npm run test:api-contracts
```

- [ ] **Step 7: Commit the API/contract slice**

```bash
git add backend-school/src/app.rs backend-school/src/api_contract.rs backend-school/src/modules/certificates/handlers.rs contracts frontend-school/src/lib/api/generated frontend-school/src/lib/permissions backend-school/src/permissions
git commit -m "feat(certificates): expose permanent purge API"
```

---

### Task 5: Svelte Permanent-Delete UX and Progress Recovery

**Files:**
- Create: `frontend-school/src/lib/components/certificates/CertificateCampaignPurgeDialog.svelte`
- Modify: `frontend-school/src/lib/api/certificates.ts`
- Modify: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/overview/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/certificates/+page.svelte`
- Modify: `frontend-school/src/lib/components/certificates/CertificateCampaignList.svelte`
- Modify: `frontend-school/tests/static/certificate-workspace.test.mjs`
- Create: `frontend-school/tests/e2e/certificate-campaign-purge.spec.ts`

**Interfaces:**
- Consumes generated schemas and the four typed API endpoints
- `CertificateCampaignPurgeDialog` props: `campaign`, `onopenchange`, `oncompleted`
- Campaign list emits `onpurge(campaign)` for both start and status views

- [ ] **Step 1: Add typed API wrappers and failing static test**

Expose generated types and wrappers with optional abort signals:

```ts
export async function getCertificateCampaignPurgeImpact(
  campaignId: string,
  options: ApiRequestOptions = {}
): Promise<CertificateCampaignPurgeImpact>;

export async function startCertificateCampaignPurge(
  campaignId: string,
  payload: StartCertificateCampaignPurgeRequest
): Promise<CertificateCampaignPurgeStatus>;

export async function getCertificateCampaignPurgeStatus(
  campaignId: string,
  options: ApiRequestOptions = {}
): Promise<CertificateCampaignPurgeStatus>;

export async function retryCertificateCampaignPurge(
  campaignId: string
): Promise<CertificateCampaignPurgeStatus>;
```

Static test requires generated types, exact-name input, all impact labels, retry copy and absence of `deleteCertificateCampaign`.

- [ ] **Step 2: Run the static test and verify RED**

```bash
node --test tests/static/certificate-workspace.test.mjs --test-concurrency=1
```

- [ ] **Step 3: Build the purge dialog in Svelte 5 runes mode**

The component mounts only while open. On mount it loads impact for normal campaigns or status for `purging`; return cleanup that aborts fetch and clears polling timeout. Use `$state.raw` for API records and `$derived` for exact-name enablement/progress.

Dialog states are `loading-impact`, `confirm`, `deleting-files`, `finalizing`, `failed`, `completed`. Show template/candidate/request/open request/issued/revoked/file/byte counts, irreversible warnings, and an input labeled with the exact campaign name. Use `LoadingButton`, `Progress`, `Alert`, `Dialog`, `Input` and a spinning `LoaderCircle`; do not implement a custom modal.

Poll status no faster than 1.5 seconds. Treat status 404 as completion only after this component has observed a started/purging job. A 409 from start reloads impact, clears confirmation and tells the user counts changed. Closing after start invokes `onopenchange(false)` without cancelling backend work.

- [ ] **Step 4: Replace overview draft delete**

Remove `AlertDialog` and `deleteCertificateCampaign`. Show a destructive danger-zone card whenever `campaign.capabilities.canDelete`, with copy “ลบกิจกรรมถาวร”. Mount the new dialog and navigate to `/staff/certificates` on completion.

- [ ] **Step 5: Add purging rows to campaign list**

Add status label/classes/filter for `purging`. Normal rows keep “เปิดชุดออก”; purging rows show disabled mutation information and a button “ดูสถานะการลบ” that invokes `onpurge`. The list page owns selected campaign/dialog and removes the campaign from its local array on completion without a broad reload.

- [ ] **Step 6: Add isolated Playwright component harness**

Stub the four API functions and cover:

- impact spinner then counts
- button disabled until exact full name
- POST body includes `expectedUpdatedAt` and exact expected counts
- deleting-files progress `x / y`
- 409 reload resets typed name
- failed state exposes retry
- 404 after started invokes completion

- [ ] **Step 7: Run Svelte autofixer one file at a time**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificateCampaignPurgeDialog.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/certificates/[campaignId]/overview/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificateCampaignList.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/certificates/+page.svelte' --svelte-version 5
```

Resolve every issue/suggestion rather than suppressing it.

- [ ] **Step 8: Run focused frontend tests sequentially**

```bash
node --test tests/static/certificate-workspace.test.mjs --test-concurrency=1
```

```bash
npx playwright test tests/e2e/certificate-campaign-purge.spec.ts --workers=1
```

- [ ] **Step 9: Commit the frontend slice**

```bash
git add frontend-school/src/lib/api/certificates.ts frontend-school/src/lib/components/certificates frontend-school/src/routes/'(app)'/staff/certificates frontend-school/tests/static/certificate-workspace.test.mjs frontend-school/tests/e2e/certificate-campaign-purge.spec.ts
git commit -m "feat(certificates): add permanent purge workflow UI"
```

---

### Task 6: Destructive Lifecycle Cleanup and Operations Documentation

**Files:**
- Modify: `frontend-school/tests/e2e/certificate-lifecycle.spec.ts`
- Modify: `docs/TESTING.md`
- Modify: `docs/OPERATIONS.md`

**Interfaces:**
- Live E2E consumes generated purge impact/start/status DTOs
- Operations retain the rule that manual metadata SQL deletion is forbidden

- [ ] **Step 1: Replace draft-only cleanup with purge cleanup**

Store `campaignName` in lifecycle state. Add a helper that gets impact, starts purge with exact confirmation, polls status until 404/completed, and scrubs all error details. It runs in `finally` for partial fixtures and as an asserted final lifecycle phase after replacement verification.

- [ ] **Step 2: Assert live post-purge behavior**

After successful purge assert old and replacement manual verification return generic 404, the student own list contains neither certificate, and every uploaded file metadata endpoint is unavailable. Set `state.campaignId = null` only after those assertions.

- [ ] **Step 3: Update canonical documentation**

`docs/TESTING.md` must state the preparer needs exact-unit delete permission, the lifecycle purges all created certificate data, and the test is restricted to an isolated tenant. Remove the statement that issued rows remain audit history.

`docs/OPERATIONS.md` must document certificate purge as the only controlled hard-delete exception: inspect job/file counts and safe codes, repair provider access, use retry API, never run manual `DELETE`, and never print provider keys/grants.

- [ ] **Step 4: Run browser discovery only**

```bash
npx playwright test --list tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

Do not run the live destructive lifecycle until compatible backend/frontend code is deployed to the configured isolated tenant.

- [ ] **Step 5: Commit the lifecycle/docs slice**

```bash
git add frontend-school/tests/e2e/certificate-lifecycle.spec.ts docs/TESTING.md docs/OPERATIONS.md
git commit -m "test(certificates): purge live lifecycle fixtures"
```

---

### Task 7: Full Sequential Verification and Integration

**Files:**
- Review all changed files
- Remove workflow artifacts only if the final integration policy requires it; otherwise retain until the implementation change is recorded

**Interfaces:**
- Consumes every prior task
- Produces a clean feature branch ready to merge to `main`

- [ ] **Step 1: Run focused certificate backend tests**

```bash
./scripts/test_backend_school.sh modules::certificates -- --nocapture --test-threads=1
```

- [ ] **Step 2: Run backend formatting**

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
```

- [ ] **Step 3: Run backend architecture tests**

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture -- --nocapture --test-threads=1
```

- [ ] **Step 4: Run backend compile check with one build job**

```bash
CARGO_BUILD_JOBS=1 cargo check --manifest-path backend-school/Cargo.toml
```

- [ ] **Step 5: Recheck generated contracts**

From `frontend-school`, run one command at a time:

```bash
npm run check:permissions
npm run test:permissions
npm run check:api-contracts
npm run test:api-contracts
```

- [ ] **Step 6: Run frontend verification one command at a time**

```bash
npm run lint
```

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

```bash
node --test --test-concurrency=1 tests/static/*.test.mjs
```

```bash
npx playwright test tests/e2e/certificate-campaign-purge.spec.ts --workers=1
```

- [ ] **Step 7: Review repository state**

```bash
git diff --check
git status --short
git log --oneline --decorate -8
```

Review the complete diff against every acceptance criterion in the spec. Confirm no credentials, grants, object keys, recipient data, plaintext national IDs, debug output, compatibility endpoint or generated-file hand edits appear.

- [ ] **Step 8: Request code review and address findings**

Use `superpowers:requesting-code-review`, validate each finding against the current diff, and apply accepted fixes through the same focused RED→GREEN loop. Re-run only the impacted focused suite, then repeat Steps 1–7 once.

- [ ] **Step 9: Finish and integrate**

Use `superpowers:verification-before-completion`, then `superpowers:finishing-a-development-branch`. Merge the verified worktree branch into `main` without rewriting unrelated user work. Do not push unless explicitly authorized.
