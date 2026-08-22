# School Font Library Implementation Plan

> **For agentic workers:** REQUIRED WORKFLOW: execute through `schoolorbit-workflow` with isolated
> worktrees, validated waves, test-driven development, independent review, and fresh integrated
> verification. Do not use generic `superpowers:subagent-driven-development` as the parallel
> execution engine; the SchoolOrbit worktree-wave controller owns delegation. Steps use checkbox
> (`- [ ]`) syntax for tracking.

**Goal:** Build one tenant-scoped school font library, integrate certificate templates as its first
typed consumer, and let dedicated font librarians manage it without certificate permissions.

**Architecture:** `school_fonts` owns scanned private logical files, while each real consumer owns a
foreign-key reference table. Certificate layout sources use `school_font` IDs; certificate purge
deletes only its reference rows. Central and certificate-context upload endpoints share one atomic
inspection/attach service after applying different authorization policies.

**Tech Stack:** PostgreSQL/SQLx migrations, Rust/Axum/utoipa, SchoolOrbit File Platform, generated
permission/OpenAPI/TypeScript contracts, SvelteKit 5 runes, shadcn-svelte, Node static tests, and
Playwright.

**Spec:** `docs/superpowers/specs/2026-08-22-school-font-library-design.md`

## Global Constraints

- Read and follow `.rules`; migrations `001`–`039` are immutable.
- Create only `backend-school/migrations/040_school_font_library.sql`; no legacy backfill,
  compatibility DTO, dual-read path, or automatic legacy deletion.
- Migration `040` fails closed unless legacy template-font assets, uploads, and layout sources are
  absent.
- Use permission `font.manage.school`; its action is the existing allowed action `manage`.
- `font.manage.school` never grants certificate, settings, or unrelated File Platform access.
- Certificate template read may list shared fonts; exact template update may upload/attach; only
  central management may delete.
- Shared font files are private, scanned, typed TTF/OTF uploads delivered only with short-lived
  grants.
- Never expose or log font bytes, raw font tables, object keys, bucket/provider URLs, signed URLs,
  request bodies, credentials, recipient data, or plaintext national IDs.
- Keep school logo, certificate background, and certificate image behavior unchanged.
- Author Rust DTOs and permission JSON first; regenerate permission and API artifacts, never edit
  generated artifacts directly.
- Every changed Svelte file must use Svelte 5 runes and pass `svelte-autofixer` before completion.
- Campaign purge owns no `school_font` logical file and deletes only certificate reference rows.
- No production migration, permission assignment, deployment, destructive live lifecycle, push,
  pull request, or merge is authorized by implementation approval.

---

## File Structure

### New files

- `backend-school/migrations/040_school_font_library.sql` — fail-closed cutover, shared schema,
  typed certificate reference relation, and permission row.
- `backend-school/src/modules/school_fonts.rs` — module exports.
- `backend-school/src/modules/school_fonts/models.rs` — shared typed request/response DTOs.
- `backend-school/src/modules/school_fonts/services.rs` — inspection, atomic attach, list, usage,
  and reference-safe deletion.
- `backend-school/src/modules/school_fonts/handlers.rs` — standalone central HTTP handlers.
- `frontend-school/src/lib/api/school-fonts.ts` — generated-schema aliases and typed wrappers.
- `frontend-school/src/lib/components/school-fonts/SchoolFontBatchUpload.svelte` — reusable batch
  upload/review/cleanup UI.
- `frontend-school/src/lib/components/school-fonts/SchoolFontLibrary.svelte` — central table and
  delete workflow.
- `frontend-school/src/routes/(app)/staff/school-fonts/+page.ts` — settings-workspace metadata and
  `font.manage.school` access.
- `frontend-school/src/routes/(app)/staff/school-fonts/+page.svelte` — central page shell and data
  orchestration.
- `frontend-school/tests/static/school-font-library.test.mjs` — permission, route, contract, and
  consumer boundary guard.
- `frontend-school/tests/e2e/school-font-library.spec.ts` — upload/delete component behavior with
  controlled API stubs.

### Primary modified files

- `contracts/permissions.json` and generated permission artifacts.
- `backend-school/src/modules.rs`, `backend-school/src/app.rs`, and
  `backend-school/src/api_contract.rs`.
- `backend-school/src/modules/files/{platform_types,purpose_registry,file_inspector,handlers,consumer_service}.rs`.
- `backend-school/src/policies/{file_access_policy,certificate_access_policy}.rs`.
- `backend-school/src/modules/{files,certificates}/{schema_tests,services_tests}.rs`.
- `backend-school/src/modules/certificates/{models,handlers}.rs` and certificate services for
  templates, layout, rendering, issuance, and purge.
- `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/` through the API
  generator.
- `frontend-school/src/lib/api/{files,certificates}.ts`.
- `frontend-school/src/lib/components/certificates/CertificateAssetManager.svelte` and
  `CertificateFontBatchUpload.svelte`.
- `frontend-school/src/lib/components/certificates/editor/{CertificateEditor,CertificateElementPanel,CertificateCanvas}.svelte`.
- `frontend-school/src/lib/certificates/{font-variants,editor-state,renderer.browser}.ts`.
- Certificate static and Playwright tests under `frontend-school/tests/`.
- `docs/OPERATIONS.md` and `docs/TESTING.md`.

## Work Graph and Ownership

All tasks are high risk. Protected-resource writers run serially. After the second approval, write
`.superpowers/schoolorbit-workflow/work-graph.json` and validate it before starting a writer.

| Wave | Task | Dependency | Profile | Protected resources |
|---|---|---|---|---|
| 1 | Schema, permission, File Platform boundary | approved plan | `schoolorbit_high_risk_implementer` | migration timeline, permission contract, file purpose, authorization |
| 2 | Shared backend module | Wave 1 integrated | `schoolorbit_high_risk_implementer` | private-file ownership, central service and handler boundary |
| 3 | Certificate consumer | Wave 2 integrated | `schoolorbit_high_risk_implementer` | layout, issuance, rendering, campaign purge |
| 4 | API contract generation | Wave 3 integrated | `schoolorbit_high_risk_implementer` | OpenAPI source and generated API artifacts |
| 5 | Central frontend | Wave 4 integrated | `schoolorbit_high_risk_implementer` | guarded route/menu and generated DTO consumption |
| 6 | Certificate frontend | Wave 5 integrated | `schoolorbit_high_risk_implementer` | certificate editor and browser renderer |
| 7 | Canonical docs and focused browser coverage | Wave 6 integrated | `schoolorbit_high_risk_implementer` | operational rollout and destructive-test procedure |

The controller audits and integrates Wave 1 through Wave 7 in order. It then starts an independent
`schoolorbit_reviewer` at `gpt-5.6-sol`/`max`, returns findings to the original owners, re-reviews
fixes, and starts `schoolorbit_verifier` at `gpt-5.6-terra`/`high` only after review passes.

---

### Task 1: Forward-Only Schema, Permission, and File Purpose Boundary

**Files:**

- Create: `backend-school/migrations/040_school_font_library.sql`
- Modify: `contracts/permissions.json`
- Generate: `contracts/permissions.lock.json`
- Generate: `backend-school/src/permissions/registry_generated.rs`
- Generate: `frontend-school/src/lib/permissions/registry.generated.ts`
- Modify: `backend-school/src/modules/files/platform_types.rs`
- Modify: `backend-school/src/modules/files/purpose_registry.rs`
- Modify: `backend-school/src/modules/files/file_inspector.rs`
- Modify: `backend-school/src/modules/files/consumer_service.rs`
- Modify: `backend-school/src/modules/files/handlers.rs`
- Modify: `backend-school/src/policies/file_access_policy.rs`
- Modify: `backend-school/src/policies/certificate_access_policy.rs`
- Test: `backend-school/src/modules/files/schema_tests.rs`
- Test: `backend-school/src/modules/certificates/schema_tests.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Produces permission constant `codes::FONT_MANAGE_SCHOOL` and frontend constant
  `PERMISSIONS.FONT_MANAGE_SCHOOL`.
- Produces `FilePurpose::SchoolFont` with code `school_font`.
- Produces tables `school_font_file_uploads`, `school_fonts`,
  `certificate_school_font_file_uploads`, and `certificate_template_font_references`.
- Produces `consumer_service::{record_school_font_upload,record_certificate_school_font_upload}`.
- Consumed by Task 2 central service and Task 3 certificate reference synchronization.

- [ ] **Step 1: Add failing migration and purpose guards**

Add static/schema and inspector tests requiring migration 040, the new permission, the four tables,
the legacy preflight, the private purpose, and removal of the legacy runtime purpose:

```rust
#[test]
fn school_font_library_is_forward_only_private_and_reference_safe() {
    let migration = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/040_school_font_library.sql"),
    )
    .expect("migration 040 must exist");
    for required in [
        "certificate_template_font",
        "CREATE TABLE school_font_file_uploads",
        "CREATE TABLE certificate_school_font_file_uploads",
        "CREATE TABLE school_fonts",
        "CREATE TABLE certificate_template_font_references",
        "font.manage.school",
        "REFERENCES school_fonts(id) ON DELETE RESTRICT",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }
    assert!(!migration.to_ascii_lowercase().contains("national_id"));
}
```

Extend File Platform registry tests to require `SchoolFont`, private visibility, TTF/OTF-only
inspection, 5 MiB limit, clean scan, and temporary retention.

- [ ] **Step 2: Run the focused tests and verify RED**

```bash
./scripts/test_backend_school.sh modules::certificates::schema_tests::school_font_library_is_forward_only_private_and_reference_safe -- --exact --nocapture --test-threads=1
cd backend-school
cargo test modules::files::purpose_registry --bin backend-school -- --nocapture
```

Expected: FAIL because migration 040 and `FilePurpose::SchoolFont` do not exist.

- [ ] **Step 3: Add the source permission entry**

Add exactly this permission object to `contracts/permissions.json`:

```json
{
  "module": "font",
  "action": "manage",
  "scope": "school",
  "name": "จัดการคลังฟอนต์โรงเรียน",
  "description": "ดู อัปโหลด และลบฟอนต์กลางของโรงเรียนโดยไม่ให้สิทธิ์แก้ไขระบบที่นำฟอนต์ไปใช้"
}
```

Do not change `contracts/permissions.schema.json` and do not grant certificate permissions through
this entry.

- [ ] **Step 4: Create migration 040 with fail-closed cutover**

The migration starts with this semantic preflight and raises before any DDL when legacy data
exists:

```sql
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM certificate_template_assets WHERE kind = 'font'
        UNION ALL
        SELECT 1 FROM certificate_template_file_uploads
        WHERE purpose_code = 'certificate_template_font'
        UNION ALL
        SELECT 1
        FROM certificate_templates AS template
        CROSS JOIN LATERAL jsonb_array_elements(template.layout -> 'elements') AS element
        WHERE element ->> 'type' = 'text'
          AND element -> 'fontSource' ->> 'type' = 'asset'
    ) THEN
        RAISE EXCEPTION
            'legacy certificate template fonts must be empty before migration 040'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
END;
$$;
```

Then remove the old font-only template-asset index/columns/constraint paths, constrain template
assets to `image`, remove `certificate_template_font` from the template-upload purpose check, and
create the shared tables. The core constraints are:

```sql
CREATE TABLE school_fonts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL UNIQUE,
    purpose_code VARCHAR(100) NOT NULL DEFAULT 'school_font'
        CHECK (purpose_code = 'school_font'),
    display_name VARCHAR(200) NOT NULL CHECK (btrim(display_name) <> ''),
    font_family VARCHAR(200) NOT NULL CHECK (btrim(font_family) <> ''),
    normalized_family VARCHAR(200) NOT NULL CHECK (btrim(normalized_family) <> ''),
    font_weight SMALLINT NOT NULL
        CHECK (font_weight BETWEEN 100 AND 900 AND font_weight % 100 = 0),
    font_style TEXT NOT NULL CHECK (font_style IN ('normal', 'italic')),
    rights_confirmed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    rights_confirmed_at TIMESTAMPTZ NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (normalized_family, font_weight, font_style),
    FOREIGN KEY (file_id, purpose_code)
        REFERENCES files(id, purpose_code) ON DELETE RESTRICT
);

CREATE TABLE school_font_file_uploads (
    file_id UUID PRIMARY KEY,
    purpose_code VARCHAR(100) NOT NULL DEFAULT 'school_font'
        CHECK (purpose_code = 'school_font'),
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (file_id, purpose_code)
        REFERENCES files(id, purpose_code) ON DELETE CASCADE
);

CREATE TABLE certificate_school_font_file_uploads (
    file_id UUID PRIMARY KEY,
    purpose_code VARCHAR(100) NOT NULL DEFAULT 'school_font'
        CHECK (purpose_code = 'school_font'),
    template_id UUID NOT NULL
        REFERENCES certificate_templates(id) ON DELETE CASCADE,
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (file_id, purpose_code)
        REFERENCES files(id, purpose_code) ON DELETE CASCADE
);

CREATE TABLE certificate_template_font_references (
    template_id UUID NOT NULL REFERENCES certificate_templates(id) ON DELETE CASCADE,
    font_id UUID NOT NULL REFERENCES school_fonts(id) ON DELETE RESTRICT,
    PRIMARY KEY (template_id, font_id)
);
```

Insert or update permission data for `font.manage.school` from the same source values. Do not add a
default certificate-role grant.

- [ ] **Step 5: Add the File Platform purpose and upload authority**

Add `SchoolFont` to `FilePurpose::ALL`, wire code mapping, purpose registry, API schema tests, and
file-purpose static counts. Remove `CertificateTemplateFont` from the runtime enum, registry,
template upload relation handling, and purpose counts after the migration preflight proves the
legacy state empty. Define the replacement as:

```rust
FilePurpose::SchoolFont => PurposeDefinition {
    domain_segment: "school",
    purpose_segment: "font",
    visibility: FileVisibility::Private,
    allowed_content: FONT_CONTENT,
    limits: document_limits(5 * 1024 * 1024),
    scan_requirement: ScanRequirement::RequiredClean,
    derivatives: &[],
    retention_class: RetentionClass::Temporary,
    policy_key: PolicyKey::SchoolFont,
}
```

In `authorize_create`, accept no resource ID only with `codes::FONT_MANAGE_SCHOOL`; accept a
template ID only after `require_template_action(..., CertificateAction::Update)`. Record every
successful central upload through `record_school_font_upload` and every template-context upload
through `record_certificate_school_font_upload`. On relation-write failure, request normal File
Platform compensation exactly like certificate temporary uploads.

- [ ] **Step 6: Generate and verify permission artifacts**

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
```

Expected: all commands exit 0 and generated constants include `FONT_MANAGE_SCHOOL`.

- [ ] **Step 7: Run focused schema/File Platform tests and verify GREEN**

```bash
./scripts/test_backend_school.sh modules::certificates::schema_tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::files::schema_tests -- --nocapture --test-threads=1
cd backend-school
cargo test modules::files::purpose_registry --bin backend-school -- --nocapture
cargo test modules::files::file_inspector --bin backend-school -- --nocapture
cargo test --test static_architecture
```

- [ ] **Step 8: Commit the protected foundation lane**

```bash
git add backend-school/migrations/040_school_font_library.sql \
  contracts/permissions.json contracts/permissions.lock.json \
  backend-school/src/permissions/registry_generated.rs \
  frontend-school/src/lib/permissions/registry.generated.ts \
  backend-school/src/modules/files/platform_types.rs \
  backend-school/src/modules/files/purpose_registry.rs \
  backend-school/src/modules/files/file_inspector.rs \
  backend-school/src/modules/files/consumer_service.rs \
  backend-school/src/modules/files/handlers.rs \
  backend-school/src/modules/files/schema_tests.rs \
  backend-school/src/policies/file_access_policy.rs \
  backend-school/src/policies/certificate_access_policy.rs \
  backend-school/src/modules/certificates/schema_tests.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(fonts): add school font library foundation"
```

---

### Task 2: Shared School-Font Backend Module

**Files:**

- Create: `backend-school/src/modules/school_fonts.rs`
- Create: `backend-school/src/modules/school_fonts/models.rs`
- Create: `backend-school/src/modules/school_fonts/services.rs`
- Create: `backend-school/src/modules/school_fonts/handlers.rs`
- Create: `backend-school/src/modules/school_fonts/services_tests.rs`
- Modify: `backend-school/src/modules.rs`
- Test: `backend-school/src/modules/school_fonts/services_tests.rs`

**Interfaces:**

- Produces `SchoolFontSummary`, `SchoolFontListResponse`, inspection/batch DTOs, and
  `SchoolFontDeleteConflict`.
- Produces `school_fonts::services::{list_for_manager,inspect_for_manager,
  attach_for_manager,delete,list_authorized}` plus shared inspection/attach primitives callable
  only after a consumer policy authorizes its typed staging relation.
- Produces standalone central handlers for `/api/school-fonts`; Task 4 registers the routes.
- Task 3 consumes `list_authorized`, font lookup/lock helpers, shared attach primitives, and the
  school-font DTOs.

- [ ] **Step 1: Write failing authorization and service tests**

Add database-backed tests proving:

```rust
assert!(school_fonts::services::list_for_manager(&pool, &manager).await.is_ok());
assert!(school_fonts::services::list_for_manager(&pool, &template_designer).await.is_err());
assert!(school_fonts::services::attach_for_manager(&pool, &ordinary_user, request)
    .await
    .is_err());
```

Add atomic-batch cases for ready fonts, duplicates in one selection, duplicate existing variants,
variable fonts, wrong purpose, cross-context staging rows, missing rights confirmation, and a
database error that leaves zero library rows promoted.

- [ ] **Step 2: Run the service test filter and verify RED**

```bash
./scripts/test_backend_school.sh modules::school_fonts -- --nocapture --test-threads=1
```

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Define exact shared DTOs**

Use typed camelCase DTOs and the existing enum macro style:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchoolFontSummary {
    pub id: Uuid,
    pub display_name: String,
    pub font_family: String,
    pub font_weight: u16,
    pub font_style: SchoolFontStyle,
    pub reference_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SchoolFontStyle {
    Normal,
    Italic,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachSchoolFontBatchRequest {
    pub file_ids: Vec<Uuid>,
    #[serde(default)]
    pub rights_confirmed: bool,
}
```

Keep the batch maximum at 40. Normalize family in one pure helper using Unicode NFKC, trim, and
lowercase, and unit-test Thai/Latin whitespace and case behavior.

- [ ] **Step 4: Implement one service with two authority entry points**

Central entry points call `require_font_manager(actor)`. `list_authorized` and the shared
inspection/attach primitives are module-internal building blocks that do not make permission
decisions; Task 3 calls them only after exact certificate policy and typed-staging validation.

Atomic attach must lock all selected staging rows, preserve caller order for inspection output,
re-read File Platform readiness and inspection metadata, reject every non-ready row before insert,
insert all `school_fonts`, promote all files, delete all staging rows, and record a safe audit event
inside one transaction.

Deletion returns a typed conflict before deleting when this query is nonzero, then relies on the
FK as the final race guard:

```sql
SELECT COUNT(*)
FROM certificate_template_font_references
WHERE font_id = $1;
```

After the domain transaction commits, call `consumer_service::request_deletions` for the logical
file ID.

- [ ] **Step 5: Add standalone handlers**

Implement typed handlers for these paths; Task 4 owns their `app.rs` and OpenAPI registration:

```text
GET    /api/school-fonts
POST   /api/school-fonts/inspect
POST   /api/school-fonts/batch
DELETE /api/school-fonts/{font_id}
```

Handlers perform context extraction, call the service, and return typed `ApiResponse` envelopes.
They contain no SQL and never accept a template ID.

- [ ] **Step 6: Add temporary-file metadata/delete policy tests**

Prove central managers can inspect/delete their unattached central staging uploads, ordinary users
cannot use those staging rows, and no generic file endpoint can directly delete a durable
`school_fonts.file_id`. Task 3 owns exact-template staging coverage.

- [ ] **Step 7: Run focused backend tests and verify GREEN**

```bash
./scripts/test_backend_school.sh modules::school_fonts -- --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::files -- --nocapture --test-threads=1
cd backend-school
cargo test --test static_architecture
```

- [ ] **Step 8: Commit the shared backend lane**

```bash
git add backend-school/src/modules.rs backend-school/src/modules/school_fonts.rs \
  backend-school/src/modules/school_fonts
git commit -m "feat(fonts): manage shared school fonts"
```

---

### Task 3: Certificate Consumer, Rendering, and Purge Boundary

**Files:**

- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/modules/certificates/services/template_service.rs`
- Modify: `backend-school/src/modules/certificates/services/layout.rs`
- Modify: `backend-school/src/modules/certificates/services/render_service.rs`
- Modify: `backend-school/src/modules/certificates/services/issuance_service.rs`
- Modify: `backend-school/src/modules/certificates/services/purge_service.rs`
- Test: `backend-school/src/modules/certificates/services_tests.rs`

**Interfaces:**

- Replaces `CertificateFontSource::Asset { asset_id }` with
  `CertificateFontSource::SchoolFont { font_id }`.
- Replaces `CertificateRenderFontGrant.asset_id` with `school_font_id`.
- Produces certificate-context list/inspect/batch endpoints.
- Maintains `certificate_template_font_references` transactionally on every layout save.

- [ ] **Step 1: Write failing consumer and race tests**

Add focused tests for:

- layout JSON round-trip with `{ "type": "school_font", "font_id": ... }`;
- rejection of the removed `{ "type": "asset" }` source;
- family/weight/style mismatch;
- missing library row;
- exact-template list/read and upload/update authorization;
- exact-template staging rows cannot be inspected, attached, or cleaned up through another
  template;
- save inserts only referenced IDs and removes stale reference rows;
- delete/save race cannot remove an in-use font;
- preview and issuance include only referenced grants;
- campaign purge inventory excludes `school_font` and cascades template references.

- [ ] **Step 2: Run focused certificate tests and verify RED**

```bash
./scripts/test_backend_school.sh modules::certificates -- --nocapture --test-threads=1
```

Expected: new school-font source/reference assertions fail.

- [ ] **Step 3: Replace the layout and grant source types**

Define:

```rust
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CertificateFontSource {
    #[default]
    BuiltIn,
    SchoolFont { font_id: Uuid },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateRenderFontGrant {
    pub school_font_id: Uuid,
    pub file_id: Uuid,
    pub family: String,
    pub weight: u16,
    pub style: SchoolFontStyle,
    pub url: String,
    pub expires_at: DateTime<Utc>,
}
```

Change certificate text elements and built-in font descriptors to reuse `SchoolFontStyle`; remove
the certificate-owned duplicate style enum so generated contracts expose one shared style type.

Template assets expose only images and image dimensions. Remove font upload/attach/delete behavior
from the legacy template-asset service.

- [ ] **Step 4: Synchronize references during layout saves**

Add pure collection plus transactional synchronization:

```rust
fn referenced_school_font_ids(layout: &CertificateLayoutV1) -> BTreeSet<Uuid>;

async fn sync_school_font_references(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
    layout: &CertificateLayoutV1,
) -> Result<(), AppError>;
```

The helper loads every referenced `school_fonts` row under the template-save transaction, validates
exact family/weight/style, inserts missing relation rows, deletes stale relation rows, and runs
before the layout update commits.

- [ ] **Step 5: Add certificate-context handlers**

Implement typed handlers for these paths; Task 4 owns route and OpenAPI registration:

```text
GET  /api/certificates/templates/{template_id}/fonts
POST /api/certificates/templates/{template_id}/fonts/inspect
POST /api/certificates/templates/{template_id}/fonts/batch
```

List requires exact template read. Inspect and batch require exact template update and call the
Task 2 shared service with the template authority context.

- [ ] **Step 6: Update preview, issuance, render, and purge**

Render services resolve school-font IDs directly from `school_fonts`, verify ready private files,
and emit short-lived grants keyed by `schoolFontId`. Issuance validates the same metadata tuple and
never synthesizes or substitutes a face.

Purge inventory keeps only `certificate_template_background` and `certificate_template_image`
template files. Add assertions that no SQL or purpose switch treats `school_font` as campaign
owned, while template cascade removes `certificate_template_font_references`.

- [ ] **Step 7: Run focused backend tests and verify GREEN**

```bash
./scripts/test_backend_school.sh modules::certificates -- --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  --test static_architecture \
  certificate_runtime_keeps_handlers_thin_proofs_private_and_renders_ephemeral \
  -- --exact --test-threads=1
```

- [ ] **Step 8: Commit the certificate consumer lane**

```bash
git add backend-school/src/modules/certificates
git commit -m "feat(certificates): consume shared school fonts"
```

---

### Task 4: Source-First API Contract and Generated Types

**Files:**

- Modify: `backend-school/src/app.rs`
- Modify: `backend-school/src/api_contract.rs`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/`
- Test: `backend-school/src/api_contract.rs`
- Test: `frontend-school/tests/static/certificate-contract.test.mjs`
- Test: `frontend-school/tests/static/file-platform-contract.test.mjs`

**Interfaces:**

- Registers all Task 2/3 paths and schemas.
- Generates `SchoolFontSummary`, list/inspection/batch/delete models, `school_font` FilePurpose,
  `CertificateFontSource.school_font`, and `schoolFontId` render grants for Tasks 5/6.

- [ ] **Step 1: Add failing API contract assertions**

Require the four central paths, three certificate-context paths, `fontId` layout source,
`schoolFontId` grant, and the absence of old certificate-template font endpoints and purpose.

- [ ] **Step 2: Run contract tests and verify RED**

```bash
cd backend-school
cargo test api_contract::tests -- --nocapture
cd ../frontend-school
npm run test:api-contracts
```

- [ ] **Step 3: Register application routes, OpenAPI paths, and schemas**

Add all Task 2/3 handlers to `backend-school/src/app.rs`. Add the annotated handlers to OpenAPI
`paths(...)`, every named DTO to `components(schemas(...))`, and contract invariants proving all
browser mutations carry CSRF metadata. Remove the legacy template-font inspect/batch operations
from both route registrations.

- [ ] **Step 4: Generate API artifacts from Rust**

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Expected: all commands exit 0; inspect the diff and reject unrelated generated drift.

- [ ] **Step 5: Run backend contract tests and verify GREEN**

```bash
cd backend-school
cargo test api_contract::tests -- --nocapture
```

- [ ] **Step 6: Commit the serialized contract lane**

```bash
git add backend-school/src/app.rs backend-school/src/api_contract.rs \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated \
  frontend-school/tests/static/certificate-contract.test.mjs \
  frontend-school/tests/static/file-platform-contract.test.mjs
git commit -m "feat(api): publish school font contracts"
```

---

### Task 5: Central Font-Library Frontend

**Files:**

- Create: `frontend-school/src/lib/api/school-fonts.ts`
- Create: `frontend-school/src/lib/components/school-fonts/SchoolFontBatchUpload.svelte`
- Create: `frontend-school/src/lib/components/school-fonts/SchoolFontLibrary.svelte`
- Create: `frontend-school/src/routes/(app)/staff/school-fonts/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/school-fonts/+page.svelte`
- Modify: `frontend-school/src/lib/api/files.ts`
- Test: `frontend-school/tests/static/school-font-library.test.mjs`
- Test: `frontend-school/tests/e2e/school-font-library.spec.ts`

**Interfaces:**

- Produces typed wrappers `listSchoolFonts`, `inspectSchoolFontUploads`,
  `attachSchoolFontBatch`, and `deleteSchoolFont`.
- Produces reusable `SchoolFontBatchUpload` with central or exact-template context.
- Task 6 consumes the reusable uploader and `SchoolFontSummary` aliases.

- [ ] **Step 1: Write failing route/API/static tests**

Assert route metadata contains:

```ts
permission: PERMISSIONS.FONT_MANAGE_SCHOOL,
group: 'settings',
workspace: 'settings'
```

Assert the central page imports no certificate campaign/template API and wrappers use concrete
generated schema types.

- [ ] **Step 2: Write failing browser component cases**

Cover sequential multi-file upload, inspection review, one rights confirmation, atomic success,
duplicate conflict, retry, temporary cleanup, in-use delete conflict, and successful unreferenced
delete. API stubs must assert central uploads omit `resource_id`.

- [ ] **Step 3: Run focused frontend tests and verify RED**

```bash
cd frontend-school
node --test tests/static/school-font-library.test.mjs --test-concurrency=1
npx playwright test tests/e2e/school-font-library.spec.ts --workers=1
```

- [ ] **Step 4: Implement typed API wrappers and reusable uploader**

Alias generated schemas only:

```ts
type Schemas = components['schemas'];
export type SchoolFontSummary = Schemas['SchoolFontSummary'];
export type SchoolFontListResponse = Schemas['SchoolFontListResponse'];
export type InspectSchoolFontUploadsRequest = Schemas['InspectSchoolFontUploadsRequest'];
export type AttachSchoolFontBatchRequest = Schemas['AttachSchoolFontBatchRequest'];
```

The uploader accepts a discriminated context:

```ts
type SchoolFontUploadContext =
    | { type: 'central' }
    | { type: 'certificate_template'; templateId: string };
```

Central upload calls `uploadFile(file, 'school_font')`; template upload calls
`uploadFile(file, 'school_font', templateId)`. Cleanup passes the same resource context.

- [ ] **Step 5: Implement the guarded settings page**

Use `PageShell`, compact card conventions, shared loading/error states, action-specific pending
state, and local list patching after attach/delete. Show only safe usage counts. On conflict, keep
the row and display the authoritative reference count.

- [ ] **Step 6: Run Svelte analysis on every new component**

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/school-fonts/SchoolFontBatchUpload.svelte
npx @sveltejs/mcp svelte-autofixer src/lib/components/school-fonts/SchoolFontLibrary.svelte
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/school-fonts/+page.svelte'
```

Resolve every reported issue without auto-editing unrelated files.

- [ ] **Step 7: Run focused tests and verify GREEN**

```bash
cd frontend-school
node --test tests/static/school-font-library.test.mjs --test-concurrency=1
npx playwright test tests/e2e/school-font-library.spec.ts --workers=1
npm run test:menu-sync
```

- [ ] **Step 8: Commit the central frontend lane**

```bash
git add frontend-school/src/lib/api/school-fonts.ts frontend-school/src/lib/api/files.ts \
  frontend-school/src/lib/components/school-fonts \
  'frontend-school/src/routes/(app)/staff/school-fonts' \
  frontend-school/tests/static/school-font-library.test.mjs \
  frontend-school/tests/e2e/school-font-library.spec.ts
git commit -m "feat(fonts): add school font library workspace"
```

---

### Task 6: Certificate Editor and Browser Renderer Frontend

**Files:**

- Modify: `frontend-school/src/lib/api/certificates.ts`
- Modify: `frontend-school/src/lib/components/certificates/CertificateAssetManager.svelte`
- Modify: `frontend-school/src/lib/components/certificates/CertificateFontBatchUpload.svelte`
- Modify: `frontend-school/src/lib/components/certificates/editor/CertificateEditor.svelte`
- Modify: `frontend-school/src/lib/components/certificates/editor/CertificateElementPanel.svelte`
- Modify: `frontend-school/src/lib/components/certificates/editor/CertificateCanvas.svelte`
- Modify: `frontend-school/src/lib/certificates/font-variants.ts`
- Modify: `frontend-school/src/lib/certificates/editor-state.ts`
- Modify: `frontend-school/src/lib/certificates/renderer.browser.ts`
- Modify: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/templates/[templateId]/editor/+page.svelte`
- Test: `frontend-school/tests/static/certificate-editor.test.mjs`
- Test: `frontend-school/tests/static/certificate-template-ui.test.mjs`
- Test: `frontend-school/tests/static/certificate-renderer.test.mjs`
- Test: `frontend-school/tests/e2e/certificate-editor.spec.ts`
- Test: `frontend-school/tests/e2e/certificate-renderer.spec.ts`

**Interfaces:**

- Consumes generated Task 4 DTOs and Task 5 reusable uploader.
- Produces school-font variants keyed by `font_id` and renderer lookup keyed by `schoolFontId`.
- Preserves image assets and image grants keyed by `assetId`.

- [ ] **Step 1: Replace test fixtures first**

Change uploaded font fixtures to:

```ts
fontSource: { type: 'school_font', font_id: '50000000-0000-4000-8000-000000000003' },
fontFamily: 'Uploaded Thai',
fontWeight: 400,
fontStyle: 'normal'
```

Add assertions that the editor receives school fonts separately from `template.assets`, upload
success patches the shared list, no editor delete control exists, and image assets remain
unchanged.

- [ ] **Step 2: Run certificate frontend tests and verify RED**

```bash
cd frontend-school
node --test tests/static/certificate-*.test.mjs --test-concurrency=1
npx playwright test tests/e2e/certificate-editor.spec.ts tests/e2e/certificate-renderer.spec.ts --workers=1
```

- [ ] **Step 3: Update typed wrappers and editor data loading**

Add template-context wrappers for list/inspect/batch. The editor route loads template, variables,
preview manifest, and school fonts without requesting manager-only endpoints. It patches only the
font list returned by a successful template-context upload.

- [ ] **Step 4: Convert variant and renderer keys**

Update the source helpers to use:

```ts
function sourceKey(source: TextCertificateElement['fontSource']): string {
    return source.type === 'school_font'
        ? `school_font:${source.font_id}`
        : 'built_in';
}
```

`renderer.browser.ts`, `editor-state.ts`, and `CertificateCanvas.svelte` resolve uploaded font
grants by `schoolFontId`. Keep image-grant lookup by `assetId` exactly as before.

- [ ] **Step 5: Replace template-owned font UI**

Keep image upload/list/delete in `CertificateAssetManager`. Replace the old font-asset list and
delete controls with Task 5 `SchoolFontBatchUpload` in certificate-template context and text that
explains the upload enters the school-wide library.

- [ ] **Step 6: Run Svelte analysis on every changed component**

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificateAssetManager.svelte
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificateFontBatchUpload.svelte
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateEditor.svelte
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateElementPanel.svelte
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateCanvas.svelte
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/certificates/[campaignId]/templates/[templateId]/editor/+page.svelte'
```

- [ ] **Step 7: Run focused frontend tests and verify GREEN**

```bash
cd frontend-school
node --test tests/static/certificate-*.test.mjs --test-concurrency=1
npx playwright test tests/e2e/certificate-editor.spec.ts tests/e2e/certificate-renderer.spec.ts --workers=1
```

- [ ] **Step 8: Commit the certificate frontend lane**

```bash
git add frontend-school/src/lib/api/certificates.ts \
  frontend-school/src/lib/components/certificates \
  frontend-school/src/lib/certificates \
  'frontend-school/src/routes/(app)/staff/certificates/[campaignId]/templates/[templateId]/editor/+page.svelte' \
  frontend-school/tests/static/certificate-editor.test.mjs \
  frontend-school/tests/static/certificate-template-ui.test.mjs \
  frontend-school/tests/static/certificate-renderer.test.mjs \
  frontend-school/tests/e2e/certificate-editor.spec.ts \
  frontend-school/tests/e2e/certificate-renderer.spec.ts
git commit -m "feat(certificates): use shared school fonts"
```

---

### Task 7: Canonical Operations, Testing, and Lifecycle Coverage

**Files:**

- Modify: `docs/OPERATIONS.md`
- Modify: `docs/TESTING.md`
- Modify: `frontend-school/tests/e2e/certificate-lifecycle.spec.ts`
- Test: `frontend-school/tests/static/documentation-policy.test.mjs`
- Test: `frontend-school/tests/static/school-font-library.test.mjs`

**Interfaces:**

- Documents the legacy-empty migration prerequisite, school-font File Platform purpose, deletion
  recovery, and isolated live checks.
- Extends the existing certificate lifecycle to prove one school font survives campaign purge.

- [ ] **Step 1: Add failing lifecycle/static assertions**

The lifecycle must upload one `school_font` through a certificate template, attach it to the shared
library, render with it, purge the campaign, and then prove the central manager can still list the
font. Cleanup of that font occurs through the central delete API only after the campaign reference
has disappeared.

- [ ] **Step 2: Update canonical documentation**

In `docs/OPERATIONS.md`, add the `school_font` private-purpose ownership, migration-040 empty-state
preflight, fail-forward recovery, reference-safe delete, and reconciler guidance. In
`docs/TESTING.md`, add focused central authorization/upload/delete checks and the certificate
lifecycle survival assertion. Do not add a new Markdown entry point or completion report.

- [ ] **Step 3: Run documentation and browser discovery checks**

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs \
  frontend-school/tests/static/schoolorbit-agent-workflow.test.mjs
cd frontend-school
npx playwright test --list tests/e2e/school-font-library.spec.ts \
  tests/e2e/certificate-editor.spec.ts \
  tests/e2e/certificate-renderer.spec.ts \
  tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

- [ ] **Step 4: Run non-live focused browser coverage**

```bash
cd frontend-school
npx playwright test tests/e2e/school-font-library.spec.ts \
  tests/e2e/certificate-editor.spec.ts \
  tests/e2e/certificate-renderer.spec.ts --workers=1
```

- [ ] **Step 5: Commit the canonical documentation lane**

```bash
git add docs/OPERATIONS.md docs/TESTING.md \
  frontend-school/tests/e2e/certificate-lifecycle.spec.ts
git commit -m "test(fonts): document shared font lifecycle"
```

---

## Integrated Review and Verification

After serial integration, give the independent reviewer the approved spec/plan, base/head commits,
validated work graph, complete diff, generated-source provenance, and all lane test evidence.
Resolve Critical and Important findings through the original owner and re-review every fix round.

Then re-read `.rules` and run these commands fresh against the integrated worktree.

### Work graph

```bash
node .agents/skills/schoolorbit-workflow/scripts/validate-work-graph.mjs \
  .superpowers/schoolorbit-workflow/work-graph.json
```

### Backend and File Platform

```bash
./scripts/test_backend_school.sh modules::school_fonts -- --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::certificates -- --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  --test static_architecture \
  certificate_runtime_keeps_handlers_thin_proofs_private_and_renders_ephemeral \
  -- --exact --test-threads=1
./scripts/test_backend_school.sh
```

From `backend-school`:

```bash
cargo test modules::files::runtime_config --bin backend-school -- --nocapture
cargo test modules::files::malware_scanner --bin backend-school -- --nocapture
cargo test modules::files::r2_storage_provider --bin backend-school -- --nocapture
cargo test modules::files::platform_service --bin backend-school -- --nocapture
cargo test modules::files::reconciler --bin backend-school -- --nocapture
cargo test api_contract::tests -- --nocapture
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

### Permission, API, and frontend

From `frontend-school`:

```bash
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/school-font-library.test.mjs \
  tests/static/certificate-*.test.mjs --test-concurrency=1
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
npx playwright test tests/e2e/school-font-library.spec.ts \
  tests/e2e/certificate-editor.spec.ts \
  tests/e2e/certificate-renderer.spec.ts --workers=1
npx playwright test --list tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

Run the live certificate lifecycle only after separate authorization and with the documented
distinct `E2E_CERT_*` accounts on an isolated tenant:

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

If credentials or authorization are absent, report this command as unrun, not passing or skipped
coverage.

### Documentation and final repository audit

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs \
  frontend-school/tests/static/schoolorbit-agent-workflow.test.mjs
git diff --check
git status --short
```

Inspect the complete base-to-head diff, migration sequence, permission/API source-to-generated
chain, changed-file ownership, private-file grants, purge exclusions, unchanged logo/image paths,
and absence of secrets or plaintext national IDs.

## Separately Authorized Actions

Implementation approval does not authorize:

- manual deletion of tenant rows or File Platform objects;
- production permission assignment;
- migration rollout or `/internal/migrate-all`;
- the destructive live certificate lifecycle;
- production smoke, deployment, push, pull request, merge, or workflow-artifact cleanup.

Each requires an explicit later authorization tied to the exact target and environment.
