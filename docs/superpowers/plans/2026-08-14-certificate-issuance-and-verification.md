# Certificate Issuance and Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a school certificate system that lets exact organization units prepare campaigns, templates, and recipients; lets school-level issuers approve and atomically assign sequential numeric certificate numbers; links issued certificates to internal student/staff accounts; and lets the public verify and download valid certificates by QR proof or certificate number plus separated first/last names.

**Architecture:** Build a new `certificates` domain beside `staff_achievements`. PostgreSQL owns drafts, request locks, immutable issued snapshots, counters, revocation, and encrypted/hashed QR proof; reusable policies own exact-unit versus school authorization. The File Platform privately stores scanned PDF backgrounds, images, and fonts and persists server-inspected geometry. The backend returns typed render manifests and short-lived asset grants, while a lazily loaded browser renderer keeps the source PDF vector background and overlays Thai text, images, and QR content without storing one generated PDF per recipient.

**Tech Stack:** Rust 2021, Axum 0.8, SQLx/PostgreSQL, `lopdf`, `ttf-parser`, AES-256-GCM/HMAC-SHA256, SvelteKit 5/Svelte 5, TypeScript, Canvas, `pdfjs-dist`, `pdf-lib`, `qrcode`, SheetJS `xlsx`, Node test runner, and Playwright.

## Global Constraints

- The approved behavior is owned by `docs/superpowers/specs/2026-08-13-certificate-issuance-and-verification-design.md`; implementation must not silently narrow recipient types, template capabilities, organization ownership, approval, public verification, or download behavior.
- Application scope is `backend-school/`, `frontend-school/`, root generated contracts, and the relevant canonical testing documentation. Do not modify `backend-admin/` or `frontend-admin/`.
- Tasks produce reviewable commits but are not independently deployable. Do not enable menu discovery for normal users or deploy until Tasks 1–18 and the final evidence gate are green.
- Never edit migrations `001` through `034`. Add exactly `backend-school/migrations/035_certificate_issuance.sql` with the complete schema needed by this plan; later corrections require a new sequential migration.
- Never accept, persist, return, or log a plaintext national ID in this domain. Reject national-ID-like spreadsheet headers before row persistence and never log import bodies, recipient names, QR proofs, verification names, signed delivery URLs, or render tokens.
- Keep certificate issuance separate from `staff_achievements`. Staff UI may display the two sources in route-backed tabs, but records are never copied between the modules.
- Use generated permission constants from `contracts/permissions.json` and generated API DTOs from the Rust/OpenAPI contract. Raw permission strings are limited to the migration, permission contract, and contract tests.
- Organization-level access is exact-unit and matching-position. A permission granted in unit A must not authorize a campaign in unit B merely because the actor is also a member of B. Parent membership never implies child access.
- Only `certificate.issue.school` assigns numbers and only `certificate.revoke.school` revokes. Preparing, importing, designing, and submitting at organization scope never imply issuance or revocation.
- The displayed number is `YYYY-AAAA-NNNNNN-C`, where `YYYY` is the Buddhist academic year, `AAAA` is the campaign sequence, `NNNNNN` is the certificate sequence within the campaign, and `C` is the Luhn check digit. Allocate only in the backend transaction, never with `MAX()+1`, never reuse numbers, reject activity sequence 10,000 and certificate sequence 1,000,000.
- Generate a unique 256-bit random base64url QR proof for each issued certificate. Store an AES-GCM encrypted copy for rebuilding the QR and a domain-separated HMAC-SHA256 hash for lookup. Never use the certificate number as the QR secret.
- The QR URL is canonical `https://<subdomain>.<BASE_DOMAIN>/verify/certificate/<number>#proof=<secret>`. The fragment is removed from browser history immediately after it is copied into memory; proof and render tokens travel only in POST JSON bodies.
- Do not persist generated recipient PDFs. Editing the current template changes every later render, including issued certificates; already downloaded or printed files remain unchanged. Do not add template-version or historical-layout tables.
- A one-page, unencrypted PDF background is the source of truth for CropBox, MediaBox, and rotation. Store PDF points and reject a displayed side outside 25–600 mm or page area above 250,000 mm². Do not accept page dimensions from the browser as authoritative.
- Layout coordinates use displayed-page PDF points with a top-left origin after applying page rotation. Persist the source CropBox offset/size plus rotation so the renderer can normalize the original vector page deterministically.
- Built-in Sarabun Regular/Bold comes from `frontend-school/static/fonts/`. Uploaded fonts are private TTF/OTF files and require an explicit rights confirmation tied to the attaching user and time.
- Spreadsheet files are parsed lazily in the browser, are never uploaded or retained, and are sent as typed rows. The backend repeats header, type, account, template, length, duplicate, and variable validation. Limit one submission to 5,000 rows, 64 custom columns, 100 Unicode scalars per header/name, and 500 Unicode scalars per custom value.
- Issued recipient snapshots are immutable. Correct a recipient-specific error by revoking and creating a replacement candidate that must pass a new issue request and receive a new number.
- Public verification returns the same not-found response for wrong number, wrong first name, wrong last name, wrong proof, and wrong tenant. Use `Cache-Control: no-store`, no-referrer behavior, constant-time digest comparison, and in-memory limits of 20 attempts per tenant/IP per five minutes plus 6 failed attempts per tenant/IP/target per ten minutes.
- Public, own, organization, and school render paths use the same manifest schema and renderer. Revoked certificates remain visible as revoked but never receive a render manifest or download control.
- No certificate realtime event is added in v1. Mutations patch local typed state; explicit page refreshes reload request queues when needed.
- Before creating, editing, or analyzing any `.svelte`, `.svelte.ts`, or `.svelte.js` file, the implementing agent must invoke both `svelte:svelte-code-writer` and `svelte:svelte-core-bestpractices`; use `frontend-design` for the new editor/workspace visual system and resolve all Svelte tooling diagnostics.
- Run the exact change-type verification matrix in `.rules`. Database tests use `scripts/test_backend_school.sh`; browser credentials come only from `E2E_*` environment variables.
- Unless a command block begins with an explicit `cd`, run it from the repository root. Every later `cd` is relative to the directory established inside that same command block only.

---

## File Responsibility Map

- `backend-school/migrations/035_certificate_issuance.sql` owns all certificate tables, indexes, constraints, the File Platform inspection-metadata column, permission rows, and default grants.
- `backend-school/src/modules/certificates/models.rs` owns every JSON wire/domain enum and named JSONB shape; handlers never return ad-hoc JSON.
- `backend-school/src/modules/certificates/services/numbering.rs`, `import_validation.rs`, `layout.rs`, and `proof.rs` own pure deterministic logic.
- `backend-school/src/modules/certificates/services/campaign_service.rs`, `template_service.rs`, `candidate_service.rs`, `request_service.rs`, `issuance_service.rs`, `render_service.rs`, and `verification_service.rs` own SQL and orchestration.
- `backend-school/src/policies/certificate_access_policy.rs` owns certificate resource authorization; `resource_access_policy.rs` owns the reusable exact-unit grant/delegation queries.
- `backend-school/src/modules/files/file_inspector.rs` owns byte-derived PDF/image/font metadata. `purpose_registry.rs` owns private certificate purpose limits and storage identity.
- `frontend-school/src/lib/api/certificates.ts` is the only authenticated certificate API wrapper; `public-certificates.ts` is the allowlisted public wrapper.
- `frontend-school/src/lib/certificates/importer.ts`, `layout.ts`, `editor-state.ts`, `paper.ts`, and `interpolation.ts` are browser-independent tested helpers.
- `frontend-school/src/lib/certificates/renderer.ts` is the import boundary; `renderer.browser.ts` owns Canvas/PDF/QR rendering and `renderer.server.ts` is the lightweight SSR failure stub.
- `frontend-school/src/lib/components/certificates/` owns reusable campaign, template, editor, import-review, request, issued-list, verification, and download UI.
- `/staff/certificates/**` owns preparation; `/staff/certificate-requests/**` owns school issuance review; `/student/certificates` and `/staff/achievements/issued` own linked-account views; `/verify/certificate/**` owns anonymous verification.

## Stable Database Contract for Migration 035

Migration 035 must create the following complete shape before any service test applies it:

| Table | Required columns and invariants |
| --- | --- |
| `certificate_academic_year_counters` | `academic_year_id` PK/FK `academic_years RESTRICT`, `next_activity_sequence` in `1..10000`, `updated_at` |
| `certificate_campaigns` | UUID PK; academic year; nullable owner organization unit `RESTRICT`; trimmed name ≤200; required event date; status `draft/active/closed/archived`; nullable activity sequence `1..9999`; next certificate sequence `1..1000000`; creator/updater/timestamps; partial unique `(academic_year_id, activity_sequence)` |
| `certificate_templates` | UUID PK; campaign FK `CASCADE`; normalized unique name per campaign; nullable background file `RESTRICT`; nullable CropBox x/y/width/height, MediaBox x/y/width/height, rotation `0/90/180/270`, and paper label that are all absent or all present; safe margin points; safe-area visibility; non-empty allowed recipient type array limited to `student/staff/external`; layout object with `schemaVersion=1`; active flag; creator/updater/timestamps |
| `certificate_template_assets` | UUID PK; template FK `CASCADE`; unique private file FK `RESTRICT`; kind `image/font`; display name; nullable font family/weight; font rights confirmer/time required together for font; creator/time |
| `certificate_import_batches` | UUID PK; campaign FK `CASCADE`; source `xlsx/csv/manual/account_search/replacement`; row count; custom header array; ready/review/invalid counts; status `processed`; creator/time; no source bytes or raw rows |
| `certificate_candidates` | UUID PK; campaign, optional batch/template, recipient type, optional matched user; at most one lookup student ID/staff username retained through converted-external revalidation and cleared after issue; imported/account title/first/last; selected source `file/account`; activity item; award/role; string-map JSON; match status; validation status `ready/needs_review/invalid`; validation code array; duplicate confirmation; nullable unique replacement-for certificate link; nullable issued certificate link; soft-delete and timestamps |
| `certificate_issue_requests` | UUID PK; campaign; status `pending/reviewing/returned/withdrawn/issued`; submitter/reviewer and transition times; return note ≤500 and typed issue-code array; timestamps |
| `certificate_issue_request_items` | Composite PK `(request_id,candidate_id)` with both FKs `RESTRICT`; preserves immutable request history |
| `certificate_candidate_issue_locks` | Candidate PK plus request FK and created time; composite FK back to the request item; rows exist only for pending/reviewing and prevent a candidate entering two active requests without a fragile boolean |
| `certificate_issue_runs` | UUID PK; request FK unique; client UUID idempotency key; issuer; outcome `issued/returned`; issued count and nullable first/last sequence; typed issue codes; timestamp; unique `(request_id,idempotency_key)` |
| `certificates` | UUID PK; campaign/template/candidate/issue-run `RESTRICT`; unique candidate; year/activity/recipient/check components and globally unique formatted number; recipient type and optional user; immutable title/first/last/template/activity/award/custom/school/owner/date snapshots; status `issued/revoked`; encrypted proof and unique 64-hex proof hash; revocation actor/time/reason; self-links for replacement; timestamps |

Add `files.inspection_metadata JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb` with an object/`kind` check. Add every `updated_at` trigger, lookup/queue/index path used by the services, JSON-object checks, all-null/all-present geometry checks, and post-create circular FKs for candidate issuance and certificate replacement. No FK may cascade into `certificates`.

## Stable HTTP and Route Contract

Authenticated JSON routes use the `ApiResponse<T>` envelope:

- `/api/certificates/campaigns` and `/campaigns/{campaign_id}`: list/create/detail/update/delete and status transition.
- `/api/certificates/owner-options`: active exact-unit choices filtered by the backend for `create` capability; school-level is represented by `ownerOrganizationUnitId: null`, not a duplicate root unit.
- `/api/certificates/campaigns/{campaign_id}/templates`, `/templates/{template_id}`, `/background`, and `/assets`: template CRUD, background attach/replace, asset attach/delete, variable catalog, and preview manifest.
- `/api/certificates/campaigns/{campaign_id}/candidates`, `/import`, `/manual`, `/account-search`, `/bulk`, and `/candidates/{candidate_id}`: typed preparation and resolution.
- `/api/certificates/campaigns/{campaign_id}/issue-requests` and `/api/certificates/issue-requests/{request_id}/{withdraw|review|return|issue}`: request lifecycle and idempotent school issuance.
- `/api/certificates/campaigns/{campaign_id}/issued`, `/api/certificates/campaigns/{campaign_id}/render-manifests`, `/api/certificates/{certificate_id}`, `/revoke`, `/replacement`, and `/render-manifest`: administration, immutable correction, and single/batch rendering (maximum 200 selected IDs).
- `/api/me/certificates`, `/api/me/certificates/{certificate_id}`, and `/render-manifest`: current-user-only issued/revoked views without target user IDs.
- `/api/public/certificates/verify/manual`, `/verify/qr`, and `/render`: generic verification plus encrypted short-lived render receipt consumption.

Frontend deep links are `/staff/certificates`, `/staff/certificates/new`, `/staff/certificates/[campaignId]/{overview,templates,recipients,requests,issued}`, `/staff/certificates/[campaignId]/templates/[templateId]/editor`, `/staff/certificate-requests`, `/staff/certificate-requests/[requestId]`, `/staff/achievements/{issued,self-recorded}`, `/student/certificates`, `/verify/certificate`, and `/verify/certificate/[number]`.

---

### Task 1: Add the complete forward-only schema and permission contract

**Files:**
- Create: `backend-school/migrations/035_certificate_issuance.sql`
- Create: `backend-school/src/modules/certificates.rs`
- Create: `backend-school/src/modules/certificates/schema_tests.rs`
- Create: `frontend-school/tests/static/certificate-contract.test.mjs`
- Modify: `backend-school/src/modules.rs`
- Modify: `contracts/permissions.json`
- Generate: `contracts/permissions.lock.json`
- Generate: `backend-school/src/permissions/registry_generated.rs`
- Generate: `frontend-school/src/lib/permissions/registry.generated.ts`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: current migration timeline through `034_auth_sessions.sql`, existing `users`, `student_info`, `academic_years`, `organization_units`, `files`, `permissions`, `roles`, `role_permissions`, `organization_permission_grants`, and `update_updated_at_column()`.
- Produces: the stable database contract above and generated constants for all fifteen approved `certificate.*` permissions.

- [ ] **Step 1: Write failing migration and permission guards**

Register `#[cfg(test)] mod schema_tests;` and assert the next migration is present, complete, and contains no national-ID field:

```rust
#[test]
fn certificate_migration_is_forward_only_and_complete() {
    let migration = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/035_certificate_issuance.sql"),
    )
    .expect("migration 035 must exist");

    for required in [
        "CREATE TABLE certificate_academic_year_counters",
        "CREATE TABLE certificate_campaigns",
        "CREATE TABLE certificate_templates",
        "CREATE TABLE certificate_template_assets",
        "CREATE TABLE certificate_import_batches",
        "CREATE TABLE certificate_candidates",
        "CREATE TABLE certificate_issue_requests",
        "CREATE TABLE certificate_issue_request_items",
        "CREATE TABLE certificate_candidate_issue_locks",
        "CREATE TABLE certificate_issue_runs",
        "CREATE TABLE certificates",
        "ADD COLUMN inspection_metadata JSONB",
        "UNIQUE (certificate_number)",
        "UNIQUE (qr_proof_hash)",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }
    assert!(!migration.to_ascii_lowercase().contains("national_id"));
}
```

In `certificate-contract.test.mjs`, load `contracts/permissions.json` and compare the exact set:

```js
const expected = [
	'certificate.read.own',
	'certificate.read.organization_unit',
	'certificate.read.school',
	'certificate.create.organization_unit',
	'certificate.create.school',
	'certificate.update.organization_unit',
	'certificate.update.school',
	'certificate.delete.organization_unit',
	'certificate.delete.school',
	'certificate.submit.organization_unit',
	'certificate.submit.school',
	'certificate.issue.school',
	'certificate.revoke.school',
	'certificate.download.organization_unit',
	'certificate.download.school'
];
assert.deepEqual(actualCertificateCodes.sort(), expected.sort());
```

- [ ] **Step 2: Run the focused guards and confirm red**

Run:

```bash
cd backend-school
cargo test modules::certificates::schema_tests::certificate_migration_is_forward_only_and_complete --bin backend-school -- --exact
cd ../frontend-school
node --test tests/static/certificate-contract.test.mjs
```

Expected: both fail because migration 035 and certificate permission definitions do not exist.

- [ ] **Step 3: Add all fifteen permission definitions and the complete migration**

Use `module: "certificate"` with actions/scopes matching each code. In migration 035:

- upsert the same permission rows;
- grant `certificate.read.own` to `roles` where `is_active=true` and `user_type IN ('staff','student')`;
- grant every `.school` certificate permission with the exact `admin_roles` CTE predicate established in migrations 019 and 023 (`ADMIN`/`SUPER_ADMIN`/`SCHOOL_ADMIN` codes plus their established English-name fallbacks);
- add exact-unit read/create/update/delete/submit/download grants for active units and positions `head`, `deputy_head`, and `coordinator` only;
- create every table/constraint/index in the stable database contract;
- add `files.inspection_metadata` for server-derived metadata.

The active candidate lock must use a real relational row:

```sql
CREATE TABLE certificate_candidate_issue_locks (
    candidate_id UUID PRIMARY KEY REFERENCES certificate_candidates(id) ON DELETE RESTRICT,
    request_id UUID NOT NULL REFERENCES certificate_issue_requests(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (request_id, candidate_id)
        REFERENCES certificate_issue_request_items(request_id, candidate_id)
        ON DELETE RESTRICT
);
```

The number constraints must preserve fixed widths:

```sql
CONSTRAINT certificates_number_shape_check CHECK (
    certificate_number ~ '^[0-9]{4}-[0-9]{4}-[0-9]{6}-[0-9]$'
),
CONSTRAINT certificates_component_range_check CHECK (
    academic_year_value BETWEEN 0 AND 9999
    AND activity_sequence BETWEEN 1 AND 9999
    AND certificate_sequence BETWEEN 1 AND 999999
    AND check_digit BETWEEN 0 AND 9
)
```

- [ ] **Step 4: Generate registries and test the real migrated schema**

Run:

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
node --test tests/static/certificate-contract.test.mjs
cd ..
./scripts/test_backend_school.sh modules::certificates::schema_tests -- --nocapture
cd backend-school
cargo test --test static_architecture active_migrations_are_clean_sequential_timeline -- --exact --nocapture
```

The database test must insert invalid status, malformed JSON, incomplete geometry, duplicate active candidate lock, duplicate certificate number, and duplicate proof hash and assert each is rejected.

- [ ] **Step 5: Commit the schema slice**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add backend-school/migrations/035_certificate_issuance.sql backend-school/src/modules.rs backend-school/src/modules/certificates.rs backend-school/src/modules/certificates/schema_tests.rs backend-school/tests/static_architecture.rs contracts/permissions.json contracts/permissions.lock.json backend-school/src/permissions/registry_generated.rs frontend-school/src/lib/permissions/registry.generated.ts frontend-school/tests/static/certificate-contract.test.mjs
git commit -m "feat(certificates): add schema and permission contract"
```

### Task 2: Persist trusted PDF, image, and font inspection metadata

**Files:**
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `backend-school/src/modules/files/platform_types.rs`
- Modify: `backend-school/src/modules/files/purpose_registry.rs`
- Modify: `backend-school/src/modules/files/file_inspector.rs`
- Modify: `backend-school/src/modules/files/repository.rs`
- Modify: `backend-school/src/modules/files/platform_service.rs`
- Modify: `backend-school/src/modules/files/schema_tests.rs`
- Modify: `backend-school/src/policies/file_access_policy.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/tests/static/file-platform-contract.test.mjs`

**Interfaces:**
- Adds `FilePurpose::{CertificateTemplateBackground, CertificateTemplateImage, CertificateTemplateFont}` with codes `certificate_template_background`, `certificate_template_image`, and `certificate_template_font`.
- Adds `DetectedContent::{Ttf,Otf}` and persisted `FileInspectionMetadata::{Unknown,Image,Pdf,Font}`.
- The `Pdf` metadata variant includes page count, CropBox and MediaBox x/y/width/height in points, and normalized rotation.
- All three purposes are private, temporary until attached, malware-scanned, and use `PolicyKey::CertificateTemplate`. Background accepts only PDF up to 20 MiB; image accepts PNG/JPEG/WebP up to 10 MiB, 6,000 px per side, and 24,000,000 decoded pixels; font accepts TTF/OTF up to 5 MiB. None creates a derivative.

- [ ] **Step 1: Add failing inspector and registry tests**

Use a generated one-page fixture built with `lopdf` in the test, plus minimal known-good TTF bytes from `frontend-school/static/fonts/Sarabun-Regular.ttf` through `include_bytes!`:

```rust
#[test]
fn certificate_background_reads_one_page_crop_box_and_rotation() {
    let pdf = one_page_pdf([18.0, 24.0, 859.89, 619.28], [0.0, 0.0, 900.0, 650.0], 90);
    let inspected = inspect_file(FilePurpose::CertificateTemplateBackground, &pdf).unwrap();
    assert_eq!(
        inspected.metadata(),
        &FileInspectionMetadata::Pdf {
            page_count: 1,
            crop_box: PdfPageBox::new(18.0, 24.0, 841.89, 595.28),
            media_box: PdfPageBox::new(0.0, 0.0, 900.0, 650.0),
            rotation: 90,
        }
    );
}

#[test]
fn certificate_background_rejects_encrypted_or_multiple_pages() {
    assert_eq!(
        inspect_file(FilePurpose::CertificateTemplateBackground, &two_page_pdf()),
        Err(FileInspectionError::PageCountNotAllowed)
    );
    assert_eq!(
        inspect_file(FilePurpose::CertificateTemplateBackground, &encrypted_pdf()),
        Err(FileInspectionError::EncryptedPdfNotAllowed)
    );
}
```

Add tests that image metadata stores decoded dimensions, TTF/OTF signatures cannot be relabeled as images, malformed fonts fail `ttf_parser::Face::parse`, and every purpose has the exact MIME/size policy.

- [ ] **Step 2: Run focused tests and confirm they fail on missing variants**

```bash
cd backend-school
cargo test modules::files::file_inspector::tests --bin backend-school -- --nocapture
cargo test modules::files::purpose_registry::tests --bin backend-school -- --nocapture
```

- [ ] **Step 3: Implement typed inspection and persistence**

Add `ttf-parser` and expose metadata without exposing uploaded bytes:

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileInspectionMetadata {
    Unknown,
    Image { width_px: u32, height_px: u32 },
    Pdf {
        page_count: u32,
        crop_box: PdfPageBox,
        media_box: PdfPageBox,
        rotation: i16,
    },
    Font { family_name: Option<String>, units_per_em: u16 },
}
```

`NewUpload` carries `inspection_metadata`; `reserve_upload` binds it into `files.inspection_metadata`; certificate background inspection walks inherited page dictionaries for CropBox/MediaBox/Rotate, rejects encryption/multiple pages, normalizes rotation modulo 360, and checks finite positive dimensions. Generic PDF purposes retain their existing multi-page behavior.

Add exhaustive match arms in `file_access_policy`; until Task 6 attaches domain relationships, all three new purposes must take the explicit certificate-domain path and never fall through a simple permission.

- [ ] **Step 4: Verify platform behavior and OpenAPI enum generation**

```bash
cd backend-school
cargo fmt --all -- --check
cargo test modules::files --bin backend-school -- --nocapture
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/file-platform-contract.test.mjs
```

Expected: existing purpose object keys and delivery tests remain unchanged; the generated `FilePurpose` union contains the three new values; no inspection metadata or object key is exposed by ordinary `FileMetadata`.

- [ ] **Step 5: Commit the trusted-inspection slice**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock backend-school/src/modules/files backend-school/src/policies/file_access_policy.rs backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts frontend-school/tests/static/file-platform-contract.test.mjs
git commit -m "feat(files): inspect certificate template assets"
```

### Task 3: Add certificate pure types, numbering, import validation, layout transforms, and proof crypto

**Files:**
- Create: `backend-school/src/modules/certificates/models.rs`
- Create: `backend-school/src/modules/certificates/services.rs`
- Create: `backend-school/src/modules/certificates/services/numbering.rs`
- Create: `backend-school/src/modules/certificates/services/import_validation.rs`
- Create: `backend-school/src/modules/certificates/services/layout.rs`
- Create: `backend-school/src/modules/certificates/services/proof.rs`
- Modify: `backend-school/src/modules/certificates.rs`
- Modify: `backend-school/src/utils/field_encryption.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`

**Interfaces:**
- Produces all certificate enums, `CertificateLayoutV1`, tagged `CertificateElement::{Text,Image,Qr}`, `CertificateImportRequest`, `CertificateImportRowInput`, `CertificateNumber`, `CertificateProof`, and named validation outcomes.
- Adds `field_encryption::hash_for_search_with_domain(domain, value)` while preserving national-ID hash behavior.
- Renderable row variables are `{คำนำหน้า}`, `{ชื่อ}`, `{นามสกุล}`, `{รายการกิจกรรม}`, and `{รางวัลหรือบทบาท}`. Reserved render variables are `{ปีการศึกษา}`, `{ชื่อกิจกรรมหลัก}`, `{เลขเกียรติบัตร}`, `{วันที่จัดกิจกรรม}`, `{วันที่ออก}`, `{ชื่อโรงเรียนผู้ออก}`, `{ชื่อหน่วยงานเจ้าของกิจกรรม}`, and `{QR_CODE}`; the variable catalog also includes validated custom headers. Matching/control headers (`ประเภทผู้รับ`, `รหัสนักเรียน`, `ชื่อผู้ใช้บุคลากร`, `แบบเกียรติบัตร`) are never render variables.

- [ ] **Step 1: Write failing pure tests for the agreed edge cases**

```rust
#[test]
fn formats_the_approved_number_and_validates_luhn() {
    let number = CertificateNumber::new(2569, 42, 123).unwrap();
    assert_eq!(number.as_str(), "2569-0042-000123-4");
    assert_eq!(CertificateNumber::parse(number.as_str()).unwrap(), number);
    assert!(CertificateNumber::parse("2569-0042-000123-5").is_err());
    assert!(CertificateNumber::new(2569, 10_000, 1).is_err());
    assert!(CertificateNumber::new(2569, 1, 1_000_000).is_err());
}

#[test]
fn rejects_forbidden_and_reserved_headers_after_unicode_normalization() {
    assert_eq!(classify_header(" national_id "), HeaderClass::Forbidden);
    assert_eq!(classify_header("เลขประจำตัวประชาชน"), HeaderClass::Forbidden);
    assert_eq!(classify_header("ชื่อ"), HeaderClass::Standard(StandardColumn::FirstName));
    assert_eq!(classify_header("ชื่อโรงเรียนผู้ออก"), HeaderClass::ReservedSystemVariable);
    assert_eq!(classify_header("ครูผู้ควบคุม"), HeaderClass::Custom("ครูผู้ควบคุม".into()));
}
```

Add tests for all recipient/template compatibility combinations, missing/duplicate headers, 5,000-row and 64-custom-column limits, NFC/collapsed-whitespace/Latin-case name normalization, plain-text interpolation, unknown variables, safe margin, paper recognition within 1 mm, and layout scale/reset at rotations 0/90/180/270.

- [ ] **Step 2: Confirm red before implementing helpers**

```bash
cd backend-school
cargo test modules::certificates::services --bin backend-school -- --nocapture
```

- [ ] **Step 3: Implement named layout and import contracts**

Use `unicode-normalization`; deny unknown fields on requests; enforce finite values and page bounds. The stored layout starts as:

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertificateLayoutV1 {
    pub schema_version: u16,
    pub elements: Vec<CertificateElement>,
}

impl Default for CertificateLayoutV1 {
    fn default() -> Self {
        Self { schema_version: 1, elements: Vec::new() }
    }
}
```

Text elements include content, frame, rotation, font source/family/weight/size/min-size, color, alignment, line height, auto-shrink, and optional shadow. Image elements include stable IDs, frames, rotation, and attached asset IDs. QR elements include stable IDs, frames, and rotation; error correction is fixed at M in the shared renderer so preview and every download path cannot diverge. No layout field accepts HTML.

- [ ] **Step 4: Implement domain-separated proof handling**

```rust
pub fn generate_certificate_proof() -> Result<CertificateProof, AppError> {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let plaintext = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    Ok(CertificateProof {
        encrypted: field_encryption::encrypt(&plaintext).map_err(proof_crypto_error)?,
        hash: field_encryption::hash_for_search_with_domain(
            "certificate-qr-proof-v1",
            &plaintext,
        )
        .map_err(proof_crypto_error)?,
        plaintext: zeroize::Zeroizing::new(plaintext),
    })
}
```

Tests set only test keys, verify different domains produce different HMACs, verify encryption round-trips, verify debug output is redacted, and never use a national-ID-shaped fixture.

- [ ] **Step 5: Run pure tests and commit**

```bash
cd backend-school
cargo fmt --all -- --check
cargo test modules::certificates::services --bin backend-school -- --nocapture
cargo test utils::field_encryption::tests --bin backend-school -- --nocapture
cargo check
git add Cargo.toml Cargo.lock src/modules/certificates.rs src/modules/certificates src/utils/field_encryption.rs
git commit -m "feat(certificates): add validated domain primitives"
```

### Task 4: Enforce exact-unit access and implement campaign APIs

**Files:**
- Create: `backend-school/src/policies/certificate_access_policy.rs`
- Create: `backend-school/src/modules/certificates/handlers.rs`
- Create: `backend-school/src/modules/certificates/services/campaign_service.rs`
- Create: `backend-school/src/modules/certificates/services/audit_service.rs`
- Create: `backend-school/src/modules/certificates/services_tests.rs`
- Create: `frontend-school/src/lib/api/certificates.ts`
- Modify: `backend-school/src/policies.rs`
- Modify: `backend-school/src/policies/resource_access_policy.rs`
- Modify: `backend-school/src/modules/certificates.rs`
- Modify: `backend-school/src/app.rs`
- Modify: `backend-school/src/api_contract.rs`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/certificate-contract.test.mjs`

**Interfaces:**
- `accessible_exact_units_for_permission(pool, actor_user_id, permission_code) -> Result<Vec<Uuid>, AppError>` unions role-based organization permission over active memberships, matching-position grants on that same unit, and active delegation for that same unit.
- `require_campaign_action(pool, actor, campaign, CertificateAction) -> Result<CertificateAccessGrant, AppError>` gives school short-circuit only for the matching `.school` permission; null owner is school-only.
- Campaign endpoints return typed summaries/details and an existing `OrganizationUnitLookupItem` list for owner options.

- [ ] **Step 1: Write failing exact-unit policy tests**

Create actors with memberships in units A and B but a position grant only in A:

```rust
#[tokio::test]
async fn grant_in_unit_a_does_not_authorize_campaign_in_unit_b() {
    let fixture = CertificatePolicyFixture::new("certificate_exact_unit").await;
    let actor = fixture.member_of_two_units_with_grant_only_in_first(
        codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
    ).await;

    assert!(require_owner_action(
        &fixture.pool,
        &actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    ).await.is_ok());
    assert!(require_owner_action(
        &fixture.pool,
        &actor,
        Some(fixture.unit_b),
        CertificateAction::Update,
    ).await.is_err());
}
```

Also test matching position, expired/future membership, inactive unit, a unit-scoped role in A plus membership in B, exact delegation to a non-member, expired/future delegation, parent-without-child, school override, null owner, and list-scope union.

- [ ] **Step 2: Run the policy tests and confirm red**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::grant_in_unit_a_does_not_authorize_campaign_in_unit_b -- --exact --nocapture
```

- [ ] **Step 3: Implement reusable exact-unit queries and campaign lifecycle**

The generic SQL must bind both permission code and target unit; it must not infer authorization from `ActorContext::has_permission` alone. Use the same three-branch predicate for single-resource checks and list-unit discovery:

```sql
SELECT EXISTS (
    SELECT 1
    FROM organization_units target_unit
    WHERE target_unit.id = $2
      AND target_unit.is_active = true
      AND (
          EXISTS (
              SELECT 1
              FROM organization_members om
              JOIN user_roles ur ON ur.user_id = om.user_id
              JOIN roles r ON r.id = ur.role_id AND r.is_active = true
              JOIN role_permissions rp ON rp.role_id = r.id
              JOIN permissions p ON p.id = rp.permission_id
              WHERE om.user_id = $1
                AND om.organization_unit_id = $2
                AND om.started_at <= CURRENT_DATE
                AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
                AND ur.started_at <= CURRENT_DATE
                AND (ur.ended_at IS NULL OR ur.ended_at > CURRENT_DATE)
                AND (ur.organization_unit_id IS NULL OR ur.organization_unit_id = $2)
                AND p.code = $3
          )
          OR EXISTS (
              SELECT 1
              FROM organization_members om
              JOIN organization_permission_grants opg
                ON opg.organization_unit_id = om.organization_unit_id
              JOIN permissions p ON p.id = opg.permission_id
              WHERE om.user_id = $1
                AND om.organization_unit_id = $2
                AND om.started_at <= CURRENT_DATE
                AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
                AND p.code = $3
                AND (opg.position_code IS NULL OR opg.position_code = om.position_code)
          )
          OR EXISTS (
              SELECT 1 FROM organization_permission_delegations opd
              JOIN permissions p ON p.id = opd.permission_id
              WHERE opd.to_user_id = $1
                AND opd.organization_unit_id = $2
                AND p.code = $3
                AND opd.started_at <= NOW()
                AND opd.revoked_at IS NULL
                AND (opd.expires_at IS NULL OR opd.expires_at > NOW())
          )
      )
)
```

A scoped `user_roles.organization_unit_id` must equal the target; a null role scope still requires an active membership in the target. A matching organization grant also requires active target membership and position. An active delegation already names the exact target and does not require an additional membership. Neither `is_primary` nor parent/child traversal participates.

Campaign create/update/delete/status services enforce active owner selection, locks from pending/reviewing requests, immutable academic year/owner after first issue, and hard delete only for a never-issued draft. `draft` becomes `active` only inside the first successful issuance transaction; issuing is allowed only from `draft` or `active`. Manual transitions are `active -> closed`, `closed -> active`, `active|closed -> archived`, and `archived -> active`; a never-issued draft is deleted instead of closed/archived. Audit metadata contains only campaign/unit IDs, status transitions, and changed field names.

- [ ] **Step 4: Add thin handlers, typed OpenAPI, and generated frontend wrapper**

Use these principal DTOs:

```rust
pub struct CreateCertificateCampaignRequest {
    pub academic_year_id: Uuid,
    pub owner_organization_unit_id: Option<Uuid>,
    pub name: String,
    pub event_date: NaiveDate,
}

pub struct UpdateCertificateCampaignRequest {
    pub expected_updated_at: DateTime<Utc>,
    pub academic_year_id: Option<Uuid>,
    pub owner_organization_unit_id: Option<NullableUuidUpdate>,
    pub name: Option<String>,
    pub event_date: Option<NaiveDate>,
    pub confirm_affects_issued_certificates: bool,
}

pub struct ChangeCertificateCampaignStatusRequest {
    pub expected_updated_at: DateTime<Utc>,
    pub status: CertificateCampaignStatus,
}
```

`NullableUuidUpdate` is a named `{ "value": UUID | null }` contract so omission means unchanged and null means school-level. Handlers perform session context → service → `ApiResponse`; they contain no SQL.
After first issuance, changing `name` or `event_date` requires `confirm_affects_issued_certificates=true` because verification and newly rendered PDFs use the current shared campaign value. The flag is ignored when those fields are unchanged and never permits academic-year or owner changes.

Generate contracts, then make `frontend-school/src/lib/api/certificates.ts` alias only generated schemas and call `requireApiData`.

- [ ] **Step 5: Verify API, policy, and architecture boundaries**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests -- --nocapture
cd backend-school
cargo test api_contract::tests::certificate_contracts --bin backend-school -- --nocapture
cargo test --test static_architecture
cargo check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/certificate-contract.test.mjs
```

- [ ] **Step 6: Commit the campaign slice**

```bash
git add backend-school/src/policies.rs backend-school/src/policies/resource_access_policy.rs backend-school/src/policies/certificate_access_policy.rs backend-school/src/modules/certificates.rs backend-school/src/modules/certificates backend-school/src/app.rs backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/certificates.ts frontend-school/src/lib/api/generated/school-api.ts frontend-school/tests/static/certificate-contract.test.mjs
git commit -m "feat(certificates): add scoped campaign management"
```

### Task 5: Build the read-first campaign workspace and creation flow

**Files:**
- Create: `frontend-school/src/lib/components/certificates/CertificateCampaignList.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateCampaignForm.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateCampaignWorkspaceNav.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificates/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/+page.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificates/new/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/new/+page.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/+layout.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/+layout.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/overview/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/overview/+page.svelte`
- Create: `frontend-school/tests/static/certificate-workspace.test.mjs`
- Modify: `frontend-school/tests/runtime/menu-route-registration.test.mjs`

**Interfaces:**
- Consumes only generated `CertificateCampaign*`, `OrganizationUnitLookupItem`, `AcademicYearLookupItem`, and generated permission constants.
- Produces menu route `/staff/certificates`, creation route, and campaign overview with capability-specific controls. The management route is discoverable only with `certificate.read.organization_unit` or `certificate.read.school`; `certificate.read.own` belongs only to personal certificate pages.

- [ ] **Step 1: Invoke required Svelte/design skills and write a failing workspace contract test**

Invoke `svelte:svelte-code-writer`, `svelte:svelte-core-bestpractices`, and `frontend-design` before reading or creating the Svelte files. The static test must require generated constants, PageShell ownership, route-backed navigation, and no raw permission strings:

```js
test('certificate workspace is permission-derived and route-backed', async () => {
	const meta = await readProjectFile('src/routes/(app)/staff/certificates/+page.ts');
	const layout = await readProjectFile(
		'src/routes/(app)/staff/certificates/[campaignId]/+layout.svelte'
	);
	assert.match(meta, /PERMISSIONS\.CERTIFICATE_READ_ORGANIZATION_UNIT/);
	assert.match(meta, /PERMISSIONS\.CERTIFICATE_READ_SCHOOL/);
	assert.doesNotMatch(meta, /PERMISSION_MODULES\.CERTIFICATE/);
	assert.match(layout, /\/templates|\/recipients|\/requests|\/issued/);
	assert.doesNotMatch(layout, /certificate\.(read|create|update|delete)\./);
});
```

- [ ] **Step 2: Run the focused test and confirm route files are absent**

```bash
cd frontend-school
node --test tests/static/certificate-workspace.test.mjs
```

- [ ] **Step 3: Implement list, create, and overview behavior**

Use `PageShell`, compact filter surfaces, shared loading/empty/error states, and a full-width workspace. The list loads only with an organization-unit or school management-read scope; the create button is visible only for create scopes. The form loads academic years and owner options only after create capability passes, maps null to “กิจกรรมระดับโรงเรียน,” excludes root `SCHOOL`, and displays the active-unit hierarchy without treating hierarchy as inherited authorization.

Creation patches the list outcome by navigating to `/staff/certificates/<id>/overview`. Overview shows status, academic year, event date, owner, activity sequence only after issuance, template/candidate/issued counts, and only lifecycle actions the current permissions permit.

- [ ] **Step 4: Validate Svelte and menu registration**

Run the Svelte code-writer analyzer/fixer on every new component, then:

```bash
cd frontend-school
node --test tests/static/certificate-workspace.test.mjs
npm run test:menu-sync
npm run sync:menu-routes
npm run test:menu-sync
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
```

Expected: the route registers once under a real staff workspace; read-only actors do not trigger create-only owner-option calls.

- [ ] **Step 5: Commit the campaign UI slice**

```bash
git add frontend-school/src/lib/components/certificates frontend-school/src/routes/'(app)'/staff/certificates frontend-school/tests/static/certificate-workspace.test.mjs frontend-school/tests/runtime/menu-route-registration.test.mjs
git commit -m "feat(frontend-school): add certificate campaign workspace"
```

### Task 6: Implement template, background, asset, and preview-manifest APIs

**Files:**
- Create: `backend-school/src/modules/certificates/services/template_service.rs`
- Create: `backend-school/src/modules/certificates/services/render_service.rs`
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/modules/certificates/services.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `backend-school/src/modules/certificates.rs`
- Modify: `backend-school/src/policies/certificate_access_policy.rs`
- Modify: `backend-school/src/policies/file_access_policy.rs`
- Modify: `backend-school/src/modules/files/consumer_service.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/src/lib/api/certificates.ts`
- Modify: `frontend-school/src/lib/api/files.ts`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/certificate-contract.test.mjs`

**Interfaces:**
- Template create returns a shell with no background; it cannot be selected for a ready candidate until a valid background and layout are attached.
- `AttachCertificateBackgroundRequest` contains `file_id`, `geometry_action: preserve|scale|reset`, and `preview_confirmed`.
- `AttachCertificateAssetRequest` contains `file_id`, `kind`, display metadata, and font rights confirmation; all byte trust comes from `files.inspection_metadata`.
- `CertificateRenderManifest` has page geometry, current layout, current campaign values, recipient/sample values, QR payload, built-in or granted fonts, granted images, background grant, and suggested filename.

- [ ] **Step 1: Add failing database/service tests for template invariants**

```rust
#[tokio::test]
async fn background_geometry_comes_from_the_ready_file_not_the_request() {
    let fixture = CertificateServiceFixture::new("certificate_template_geometry").await;
    let template = fixture.create_template().await;
    let file_id = fixture.ready_background_with_geometry(841.89, 595.28, 0).await;

    let updated = template_service::attach_background(
        &fixture.pool,
        &fixture.actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    ).await.unwrap();

    assert_eq!(updated.page_geometry.unwrap().crop_box.width_points, 841.89);
}
```

Also test wrong purpose/not-ready/multiple-page metadata, 24.9 mm and 600.1 mm sides, excessive area, duplicate normalized name, recipient compatibility, same-geometry preserve, changed-geometry preserve rejection, deterministic scale/reset, active request lock, used-template delete-to-deactivate behavior, font without rights confirmation, referenced asset deletion, and cross-campaign file relation denial.

- [ ] **Step 2: Confirm red with the focused service suite**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::background_geometry_comes_from_the_ready_file_not_the_request -- --exact --nocapture
```

- [ ] **Step 3: Implement template lifecycle and file attachment transactions**

Read and deserialize `files.inspection_metadata` in the service; do not accept geometry in JSON. On attachment, update the file retention to `standard` in the same domain transaction. On replacement or unused deletion, commit the relationship change first, then call `request_deletions` for detached IDs.

For different displayed geometry, compute server-side transformed layout and require the matching action plus `preview_confirmed=true`. Never scale the PDF background itself. Used-template updates query all issued certificate custom maps, return `missingVariableCertificateCount`, and require explicit acknowledgement only when the saved layout introduces variables absent from issued snapshots.

- [ ] **Step 4: Wire explicit File Platform authorization**

For create, require `resource_id=template_id`, load the template's campaign owner, and require template update access. For existing files:

- background or linked asset read follows campaign read access;
- unlinked temporary file deletion requires the creating actor plus template update access;
- a currently referenced file returns conflict from the generic delete endpoint and must be detached through the template service;
- no certificate purpose is ever made public.

Add policy tests for allowed/denied/read-only/wrong-template/wrong-tenant-equivalent relationships and verify the fallback `ExplicitOwningResource` path cannot grant access.

- [ ] **Step 5: Add typed preview manifests and short-lived private grants**

`POST /api/certificates/templates/{template_id}/preview-manifest` accepts:

```rust
pub struct CertificatePreviewManifestRequest {
    pub preview_kind: CertificatePreviewKind,
    pub candidate_id: Option<Uuid>,
    pub sample_values: BTreeMap<String, String>,
}
```

Until candidates exist, `short/normal/long` produce deterministic Thai sample names; after Task 10, an authorized `candidate_id` uses real draft values. Preview certificate number and QR payload are explicitly marked `ตัวอย่าง` and never resemble an issued proof. Manifest asset grants expire using the existing File Platform TTL and are never written to audit metadata.

- [ ] **Step 6: Register/generate contracts and verify**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests -- --nocapture
cd backend-school
cargo test modules::files --bin backend-school -- --nocapture
cargo test api_contract::tests::certificate_contracts --bin backend-school -- --nocapture
cargo test --test static_architecture
cargo check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/certificate-contract.test.mjs tests/static/file-platform-contract.test.mjs
```

- [ ] **Step 7: Commit the template backend slice**

```bash
git add backend-school/src/modules/certificates backend-school/src/modules/files/consumer_service.rs backend-school/src/policies backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/certificates.ts frontend-school/src/lib/api/files.ts frontend-school/src/lib/api/generated/school-api.ts frontend-school/tests/static/certificate-contract.test.mjs
git commit -m "feat(certificates): manage templates and private assets"
```

### Task 7: Add template management and asset upload UI

**Files:**
- Create: `frontend-school/src/lib/components/certificates/CertificateTemplateList.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateTemplateForm.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateBackgroundUpload.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateAssetManager.svelte`
- Create: `frontend-school/src/lib/certificates/paper.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/templates/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/templates/+page.svelte`
- Create: `frontend-school/tests/static/certificate-template-ui.test.mjs`
- Create: `frontend-school/tests/static/certificate-paper.test.mjs`

**Interfaces:**
- Uses typed upload purposes and always sends the template ID as File Platform `resource_id`.
- `recognizePaper(geometry)` labels A4/A5/Letter within 1 mm after rotation, otherwise `ขนาดกำหนดเอง <width> × <height> มม.`.
- A template card displays allowed recipient types, current page/orientation, background readiness, active state, and editor link.

- [ ] **Step 1: Invoke Svelte/design skills and add failing paper/UI tests**

Invoke the three frontend skills listed in Global Constraints. Add pure tests:

```js
test('recognizes rotated A4 and preserves custom dimensions', async () => {
	const { describePaper } = await import('../../src/lib/certificates/paper.ts');
	assert.equal(describePaper({ widthPoints: 841.89, heightPoints: 595.28, rotation: 0 }), 'A4 แนวนอน');
	assert.equal(describePaper({ widthPoints: 595.28, heightPoints: 841.89, rotation: 90 }), 'A4 แนวนอน');
	assert.match(describePaper({ widthPoints: 720, heightPoints: 360, rotation: 0 }), /ขนาดกำหนดเอง/);
});
```

Static UI tests require `.pdf`, `.png,.jpg,.jpeg,.webp`, and `.ttf,.otf` accept filters, rights confirmation, generated permission constants, and no manual width/height input.

- [ ] **Step 2: Confirm focused tests fail**

```bash
cd frontend-school
node --test tests/static/certificate-paper.test.mjs tests/static/certificate-template-ui.test.mjs
```

- [ ] **Step 3: Implement template and attachment workflows**

Create the template shell first, upload the initial PDF with purpose `certificate_template_background` and its template ID, then attach the returned file ID. The UI shows backend-derived geometry only after attachment. Images and fonts follow the same two-step upload/attach workflow; font attachment cannot submit until the administrator checks the rights statement.

On failed attach, retain the typed error and offer deletion of the unattached temporary file; never mark a malformed file usable. Patch only the affected template/asset in local state.

- [ ] **Step 4: Analyze each Svelte component and run frontend gates**

```bash
cd frontend-school
node --test tests/static/certificate-paper.test.mjs tests/static/certificate-template-ui.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
```

- [ ] **Step 5: Commit the template UI slice**

```bash
git add frontend-school/src/lib/components/certificates frontend-school/src/lib/certificates/paper.ts frontend-school/src/routes/'(app)'/staff/certificates/'[campaignId]'/templates frontend-school/tests/static/certificate-paper.test.mjs frontend-school/tests/static/certificate-template-ui.test.mjs
git commit -m "feat(frontend-school): add certificate template assets"
```

### Task 8: Implement the lazy shared browser PDF renderer

**Files:**
- Create: `frontend-school/src/lib/certificates/layout.ts`
- Create: `frontend-school/src/lib/certificates/interpolation.ts`
- Create: `frontend-school/src/lib/certificates/renderer.ts`
- Create: `frontend-school/src/lib/certificates/renderer.browser.ts`
- Create: `frontend-school/src/lib/certificates/renderer.server.ts`
- Create: `frontend-school/src/lib/certificates/download.ts`
- Create: `frontend-school/tests/static/certificate-renderer.test.mjs`
- Create: `frontend-school/tests/e2e/certificate-renderer.spec.ts`
- Modify: `frontend-school/package.json`
- Modify: `frontend-school/package-lock.json`
- Modify: `frontend-school/vite.config.ts`
- Modify: `frontend-school/tests/static/browser-only-heavy-dependencies.test.mjs`

**Interfaces:**
- `loadCertificateRenderer(): Promise<CertificateRenderer>` is the only UI import boundary.
- `renderPreview(manifest, canvas, options)` and `buildCertificatePdf(manifests)` share interpolation, fonts, image loading, QR generation, text layout, and coordinate transforms.
- Batch output accepts at most 200 manifests and preserves each normalized page's own dimensions.

- [ ] **Step 1: Write failing pure and browser tests**

Pure tests cover escaped braces, missing-value reporting, points/mm, rotated coordinate matrices, auto-shrink bounds, filename sanitization, and batch limit. The Playwright test constructs vector PDFs in memory with each rotation, renders a Thai name/shadow/QR, and compares preview pixels with a `pdfjs-dist` raster of the downloaded result at the same viewport:

```ts
expect(pixelDifferenceRatio(previewPixels, exportedPixels)).toBeLessThan(0.015);
expect(exportedPageSizes).toEqual([
	{ width: 841.89, height: 595.28 },
	{ width: 419.53, height: 595.28 }
]);
```

- [ ] **Step 2: Confirm tests fail before dependencies and renderer exist**

```bash
cd frontend-school
node --test tests/static/certificate-renderer.test.mjs tests/static/browser-only-heavy-dependencies.test.mjs
npx playwright test --list tests/e2e/certificate-renderer.spec.ts
```

- [ ] **Step 3: Add browser-only dependencies and the SSR redirect boundary**

Install `pdf-lib`, `pdfjs-dist`, `qrcode`, and `@types/qrcode`. Extend the Vite pre-plugin so SSR resolution of the extensionless `src/lib/certificates/renderer` points to `renderer.server.ts`; the browser resolves `renderer.browser.ts`. Do not import those packages from route modules or ordinary components.

The server stub exports the same interface and rejects without touching `window`:

```ts
export async function loadCertificateRenderer(): Promise<never> {
	throw new Error('ตัวสร้างเกียรติบัตรใช้งานได้เฉพาะในเบราว์เซอร์');
}
```

- [ ] **Step 4: Implement one deterministic rendering pipeline**

Fetch every signed asset with `credentials: 'omit'` and `referrerPolicy: 'no-referrer'`. Use `pdfjs-dist` for editor background rasterization. Load fonts through `FontFace`; built-in Sarabun resolves from `/fonts/Sarabun-Regular.ttf` and `/fonts/Sarabun-Bold.ttf`.

At 300 DPI, render all text into one transparent Canvas layer using browser Thai shaping, wrap/align/line-height/auto-shrink/shadow rules, and fail the whole operation if any required font/image/QR fails. Generate QR with error correction M. Use `pdf-lib` to embed the original source page as a vector page into a new rotation-zero page with displayed dimensions, then overlay the transparent PNG. Use these tested background transforms:

```ts
const transforms = {
	0:   { x: 0, y: 0, rotation: 0 },
	90:  { x: sourceHeight, y: 0, rotation: 90 },
	180: { x: sourceWidth, y: sourceHeight, rotation: 180 },
	270: { x: 0, y: sourceWidth, rotation: 270 }
} as const;
```

CropBox offsets are passed to `embedPage`. Keep the transform table as the implementation contract and require the rotation fixtures to prove preview/export pixel equivalence before accepting any coordinate change. Mixed-size manifests each become their own output page.

- [ ] **Step 5: Verify renderer, SSR bundle boundary, and browser output**

```bash
cd frontend-school
node --test tests/static/certificate-renderer.test.mjs tests/static/browser-only-heavy-dependencies.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run build
npx playwright test tests/e2e/certificate-renderer.spec.ts
npm run lint
```

Expected: server output contains the stub but not `pdfjs-dist`, `pdf-lib`, `qrcode`, or `xlsx`; preview/export pixel comparison and mixed page sizes pass.

- [ ] **Step 6: Commit the renderer slice**

```bash
git add frontend-school/package.json frontend-school/package-lock.json frontend-school/vite.config.ts frontend-school/src/lib/certificates frontend-school/tests/static/certificate-renderer.test.mjs frontend-school/tests/static/browser-only-heavy-dependencies.test.mjs frontend-school/tests/e2e/certificate-renderer.spec.ts
git commit -m "feat(frontend-school): render certificate PDFs in browser"
```

### Task 9: Build the focused Canva-like certificate editor

**Files:**
- Create: `frontend-school/src/lib/certificates/editor-state.ts`
- Create: `frontend-school/src/lib/components/certificates/editor/CertificateEditor.svelte`
- Create: `frontend-school/src/lib/components/certificates/editor/CertificateCanvas.svelte`
- Create: `frontend-school/src/lib/components/certificates/editor/CertificateToolbar.svelte`
- Create: `frontend-school/src/lib/components/certificates/editor/CertificateElementPanel.svelte`
- Create: `frontend-school/src/lib/components/certificates/editor/CertificateLayersPanel.svelte`
- Create: `frontend-school/src/lib/components/certificates/editor/CertificateVariablePicker.svelte`
- Create: `frontend-school/src/lib/components/certificates/editor/CertificateBackgroundReplaceDialog.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/templates/[templateId]/editor/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/templates/[templateId]/editor/+page.svelte`
- Create: `frontend-school/tests/static/certificate-editor.test.mjs`
- Create: `frontend-school/tests/e2e/certificate-editor.spec.ts`

**Interfaces:**
- `CertificateEditorState` owns selection, undo-free current layout, drag/resize/rotate, duplicate, delete, z-order, alignment, snapping, zoom, safe-area visibility, and dirty/saving state.
- Save sends the full validated current layout with `expectedUpdatedAt`; there is no version history or rollback UI.

- [ ] **Step 1: Invoke required Svelte/design skills and write failing editor-state tests**

Test movement in page points independent of viewport scale, minimum frame size, rotation, duplication IDs, layer ordering, multi-element alignment, and deterministic scale/reset:

```js
test('dragging converts screen pixels to page points', async () => {
	const { moveElement } = await import('../../src/lib/certificates/editor-state.ts');
	const moved = moveElement(textElement({ x: 72, y: 72 }), { dxPixels: 40, dyPixels: -20 }, 2);
	assert.deepEqual(moved.frame, { x: 92, y: 62, width: 240, height: 60 });
});
```

The static test requires controls for text, QR, image, size, color, alignment, line height, auto-shrink, shadow, duplicate, delete, layers, safe area, short/normal/long/real preview, and background scale/reset.

- [ ] **Step 2: Confirm editor tests fail**

```bash
cd frontend-school
node --test tests/static/certificate-editor.test.mjs
npx playwright test --list tests/e2e/certificate-editor.spec.ts
```

- [ ] **Step 3: Implement pointer/keyboard editor interactions**

Use a real bordered page with full available workspace height and scroll only the side panels. The locked PDF background is never selectable. Convert pointer movement through the current zoom into page points; keep 8 resize handles, a rotation handle, arrow-key nudging, Delete, duplicate, layer up/down, center/middle alignment, and a 10 mm safe-area warning that can be adjusted or hidden.

Text insertion may combine static text and picker-inserted `{variable}` tokens. Treat content as plain text. QR is a single special element; image elements select only attached image assets. Property controls are typed and action-specific saving states use `LoadingButton`.

- [ ] **Step 4: Implement previews, save conflicts, and background replacement**

Load renderer code only when the editor asks for a preview/background. Preview modes request typed manifests for short/normal/long or a selected real candidate. A background with unchanged source geometry preserves layout; changed geometry opens the required scale/reset comparison and does not attach until the user previews and confirms.

If `expectedUpdatedAt` conflicts, retain local state, show that the server copy changed, and offer explicit reload; never silently overwrite another save.

- [ ] **Step 5: Analyze all Svelte files and verify editor workflows**

```bash
cd frontend-school
node --test tests/static/certificate-editor.test.mjs tests/static/certificate-renderer.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test tests/e2e/certificate-editor.spec.ts
```

- [ ] **Step 6: Commit the editor slice**

```bash
git add frontend-school/src/lib/certificates/editor-state.ts frontend-school/src/lib/components/certificates/editor frontend-school/src/routes/'(app)'/staff/certificates/'[campaignId]'/templates/'[templateId]'/editor frontend-school/tests/static/certificate-editor.test.mjs frontend-school/tests/e2e/certificate-editor.spec.ts
git commit -m "feat(frontend-school): add certificate layout editor"
```

### Task 10: Implement recipient creation, import validation, account matching, and bulk resolution APIs

**Files:**
- Create: `backend-school/src/modules/certificates/services/candidate_service.rs`
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/modules/certificates/services.rs`
- Modify: `backend-school/src/modules/certificates/services/render_service.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `backend-school/src/modules/certificates.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/src/lib/api/certificates.ts`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/certificate-contract.test.mjs`

**Interfaces:**
- `CertificateImportRequest { source, headers, rows }` accepts typed rows, not an uploaded workbook.
- `CertificateCandidateBulkRequest` is a tagged operation: assign template, choose account/file name, confirm external, confirm duplicate, or soft-delete.
- Candidate account search returns only user ID, recipient type, student ID or username, title, first name, and last name; it never exposes national ID/contact/medical/guardian fields.

- [ ] **Step 1: Add failing matching/import decision-table tests**

```rust
#[tokio::test]
async fn matched_accounts_cannot_become_external_in_single_or_bulk_flows() {
    let fixture = CertificateServiceFixture::new("candidate_external_guard").await;
    let user_id = fixture.active_student("S-0069", "กมล", "ใจดี").await;
    let candidate = fixture.import_student("S-0069", "กมล", "ใจดี").await;
    assert_eq!(candidate.matched_user_id, Some(user_id));

    assert!(candidate_service::confirm_external(
        &fixture.pool,
        &fixture.actor,
        candidate.id,
    ).await.is_err());
    assert!(candidate_service::bulk_update(
        &fixture.pool,
        &fixture.actor,
        CertificateCandidateBulkRequest::ConfirmExternal {
            candidate_ids: vec![candidate.id],
        },
    ).await.is_err());
}
```

Add tests for exact student ID, exact staff username, inactive account, name match/mismatch, system/file name choice, unmatched conversion, account created just before conversion, template recipient compatibility including external competition awards, duplicate warnings, all row statuses, header-level atomic rejection, and no forbidden values in audit.

- [ ] **Step 2: Run the matching test and confirm red**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::matched_accounts_cannot_become_external_in_single_or_bulk_flows -- --exact --nocapture
```

- [ ] **Step 3: Implement header-atomic and row-level validation**

Before opening the insert transaction, normalize all headers and reject the whole import for missing required headers, duplicates, reserved variables, national-ID-like names, too many rows/columns, or invalid source. Never include a cell value in an error or trace field.

Within the transaction, fetch all distinct student IDs and staff usernames in two bulk queries, map active/inactive/existing states, normalize names, resolve a sole compatible template by normalized name when possible, and insert all candidates with `QueryBuilder<Postgres>` in bounded chunks. Store invalid rows so the web can edit them; store only batch counts/custom headers, not workbook bytes or raw request JSON.

- [ ] **Step 4: Implement manual external, minimal account search, edit, and bulk resolution**

Manual external candidates have no lookup identifier. An imported internal-intent row that is confirmed external retains only its student ID or username on the draft candidate so issuance can re-query; that identifier is cleared when issued and is never copied into `certificates`.

Every edit recomputes status from authoritative template/account data. Confirmation to external re-runs an existence query including inactive users and rejects when any matching account now exists. Candidate delete uses `deleted_at`, preserving request history. Bulk operations validate the complete set before updating, so a mixed invalid selection makes no partial mutation.

- [ ] **Step 5: Extend actual-candidate preview and typed API contracts**

An authorized preview candidate resolves selected name, activity, award/role, custom map, current campaign fields, and template variables into the existing manifest. Add list filters for status/template/search and return summary counts in one typed response.

Register/generate every endpoint and update the frontend wrapper with concrete generated types only.

- [ ] **Step 6: Verify candidate behavior and privacy boundaries**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests -- --nocapture
cd backend-school
cargo test api_contract::tests::certificate_contracts --bin backend-school -- --nocapture
cargo test --test static_architecture
cargo check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/certificate-contract.test.mjs
```

- [ ] **Step 7: Commit the candidate backend slice**

```bash
git add backend-school/src/modules/certificates backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/certificates.ts frontend-school/src/lib/api/generated/school-api.ts frontend-school/tests/static/certificate-contract.test.mjs
git commit -m "feat(certificates): validate and match recipients"
```

### Task 11: Build browser spreadsheet import and recipient review UI

**Files:**
- Create: `frontend-school/src/lib/certificates/importer.ts`
- Create: `frontend-school/src/lib/certificates/import-template.ts`
- Create: `frontend-school/src/lib/components/certificates/CertificateRecipientWorkspace.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateImportDialog.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateCandidateTable.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateCandidateEditDialog.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateAccountSearchDialog.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateManualExternalDialog.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/recipients/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/recipients/+page.svelte`
- Create: `frontend-school/tests/static/certificate-importer.test.mjs`
- Create: `frontend-school/tests/static/certificate-recipient-ui.test.mjs`
- Create: `frontend-school/tests/e2e/certificate-import-review.spec.ts`

**Interfaces:**
- `parseCertificateImport(file): Promise<ParsedCertificateImport>` loads SheetJS only after file selection; CSV decoding is UTF-8 fatal and handles BOM.
- The source `File` never leaves the browser. Only `{ source, headers, rows }` goes to the backend.
- The fixed header order is exactly `ประเภทผู้รับ`, `รหัสนักเรียน`, `ชื่อผู้ใช้บุคลากร`, `คำนำหน้า`, `ชื่อ`, `นามสกุล`, `รายการกิจกรรม`, `รางวัลหรือบทบาท`, `แบบเกียรติบัตร`.

- [ ] **Step 1: Invoke Svelte/design skills and add failing parser/template tests**

Test quoted/multiline UTF-8 CSV, BOM, Thai XLSX displayed values, duplicate headers, empty trailing rows, formula text safety, and generated sample columns:

```js
test('parses UTF-8 CSV into typed rows without uploading the file', async () => {
	const { parseCertificateCsv } = await import('../../src/lib/certificates/importer.ts');
	const parsed = await parseCertificateCsv(
		new TextEncoder().encode('ประเภทผู้รับ,ชื่อ,นามสกุล,รางวัลหรือบทบาท\nบุคคลภายนอก,"กมล",ใจดี,"วิทยากร"')
	);
	assert.equal(parsed.rows[0].recipientType, 'external');
	assert.equal(parsed.rows[0].firstName, 'กมล');
});
```

The static UI test must require status counts, per-row edit, template bulk assignment, name-source resolution, single/bulk external confirmation, matched-account external prohibition messaging, and horizontally scrollable fixed-width columns.

- [ ] **Step 2: Confirm parser/UI tests fail**

```bash
cd frontend-school
node --test tests/static/certificate-importer.test.mjs tests/static/certificate-recipient-ui.test.mjs
```

- [ ] **Step 3: Implement lazy import and sample downloads**

Use `TextDecoder('utf-8', { fatal: true })` for CSV and lazy `import('xlsx')` for workbook parsing and XLSX template generation. Read displayed strings with `raw: false`, never execute formulas, normalize empty cells to `''`, and reject a second non-empty sheet to avoid silently importing the wrong one. Offer both `.xlsx` and UTF-8 BOM `.csv` examples with the fixed headers and one clearly fictional row.

- [ ] **Step 4: Implement the review workspace**

Show ready/review/invalid totals before submission, filter/search, and allow per-row/bulk operations. Disable “เปลี่ยนเป็นบุคคลภายนอก” for every matched or inactive existing account. If the backend reports that an account appeared during confirmation, patch the row back to review and display the typed reason.

Account search and manual external creation use their dedicated minimal APIs. Template choices display recipient compatibility; external students from other schools can select any template allowing `external`, including competition awards.

- [ ] **Step 5: Analyze Svelte files and run browser tests**

```bash
cd frontend-school
node --test tests/static/certificate-importer.test.mjs tests/static/certificate-recipient-ui.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test tests/e2e/certificate-import-review.spec.ts
```

- [ ] **Step 6: Commit the import/review UI slice**

```bash
git add frontend-school/src/lib/certificates/importer.ts frontend-school/src/lib/certificates/import-template.ts frontend-school/src/lib/components/certificates frontend-school/src/routes/'(app)'/staff/certificates/'[campaignId]'/recipients frontend-school/tests/static/certificate-importer.test.mjs frontend-school/tests/static/certificate-recipient-ui.test.mjs frontend-school/tests/e2e/certificate-import-review.spec.ts
git commit -m "feat(frontend-school): review certificate recipients"
```

### Task 12: Implement issue-request submission, locking, review, return, and withdrawal

**Files:**
- Create: `backend-school/src/modules/certificates/services/request_service.rs`
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/modules/certificates/services.rs`
- Modify: `backend-school/src/modules/certificates/services/campaign_service.rs`
- Modify: `backend-school/src/modules/certificates/services/template_service.rs`
- Modify: `backend-school/src/modules/certificates/services/candidate_service.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `backend-school/src/modules/certificates.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/src/lib/api/certificates.ts`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`

**Interfaces:**
- `submit_issue_request(pool, actor, campaign_id, Vec<Uuid>)` atomically validates all selected candidates, inserts history items and lock rows, and returns one pending request.
- Transition commands are `withdraw`, `start_review`, and `return_request`; returned requests remain immutable history and fixes require a new request.
- `CertificateResourceLocked` is a typed conflict with a request ID only when the actor can read that request.

- [ ] **Step 1: Write a failing transition and lock matrix**

```rust
#[tokio::test]
async fn active_request_locks_only_selected_candidates_and_referenced_templates() {
    let fixture = CertificateServiceFixture::new("request_resource_locks").await;
    let selected = fixture.ready_candidate("แบบรางวัล").await;
    let unselected = fixture.ready_candidate("แบบวิทยากร").await;
    let request = fixture.submit(vec![selected.id]).await;

    assert!(fixture.update_candidate(selected.id).await.is_err());
    assert!(fixture.update_template(selected.template_id.unwrap()).await.is_err());
    assert!(fixture.update_candidate(unselected.id).await.is_ok());
    assert!(fixture.update_template(unselected.template_id.unwrap()).await.is_ok());
    assert_eq!(request.status, CertificateIssueRequestStatus::Pending);
}
```

Test no candidate in two active requests, only ready/non-deleted candidates, owner-active submission, submitter-only pending withdrawal, issuer-only reviewing/return, invalid transitions, request list scoping, lock release on terminal state, and new-request-after-return.

- [ ] **Step 2: Confirm focused tests fail**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::active_request_locks_only_selected_candidates_and_referenced_templates -- --exact --nocapture
```

- [ ] **Step 3: Implement submission and relational locks**

Lock the campaign and candidate rows in deterministic UUID order. Recompute every candidate's readiness and template compatibility, validate exact-unit submit permission, insert request/items, then insert candidate lock rows. A uniqueness conflict becomes the typed locked response. Campaign shared-field updates check for any active lock in the campaign; template updates check only active locked candidates referencing that template.

- [ ] **Step 4: Implement transitions and safe return notes**

Withdrawal requires the original submitter, pending state, and current submit scope. Review/return requires `certificate.issue.school`. Return notes trim/collapse whitespace, max at 500 characters, reject a 13-digit run and national-ID header terms, and are never copied into audit metadata. Returning/withdrawing deletes lock rows in the same transaction and records typed issue codes separately from free text.

- [ ] **Step 5: Register/generate contracts and verify**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests -- --nocapture
cd backend-school
cargo test api_contract::tests::certificate_contracts --bin backend-school -- --nocapture
cargo test --test static_architecture
cargo check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

- [ ] **Step 6: Commit the request backend slice**

```bash
git add backend-school/src/modules/certificates backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/certificates.ts frontend-school/src/lib/api/generated/school-api.ts
git commit -m "feat(certificates): add issue request workflow"
```

### Task 13: Add preparation requests and the school review queue UI

**Files:**
- Create: `frontend-school/src/lib/components/certificates/CertificateSubmitRequestDialog.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateCampaignRequests.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateIssueQueue.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateIssueRequestReview.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/requests/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/requests/+page.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificate-requests/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificate-requests/+page.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificate-requests/[requestId]/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificate-requests/[requestId]/+page.svelte`
- Create: `frontend-school/tests/static/certificate-request-ui.test.mjs`
- Create: `frontend-school/tests/e2e/certificate-request-workflow.spec.ts`
- Modify: `frontend-school/tests/runtime/menu-route-registration.test.mjs`

**Interfaces:**
- Campaign request UI requires submit organization/school permissions; queue routes require `certificate.issue.school` and never become visible because of a preparation permission.
- Review uses request items, current revalidation summary, and sample preview manifests; the reviewer cannot edit candidates/template/campaign in the review screen.

- [ ] **Step 1: Invoke Svelte/design skills and add failing permission/workflow tests**

Require generated route constants and the state sequence pending → reviewing → returned/withdrawn; assert no candidate editing function is imported into review UI.

- [ ] **Step 2: Implement submit/history and school queue screens**

Recipients page selects ready rows and opens a summary grouped by template before submission. Campaign request history shows immutable states and permits withdrawal only to the submitter while pending. The school queue shows owner unit, submitter, count, templates, submitted time, and typed warnings without recipient names in the list.

Review detail loads recipient rows only after issue permission passes, allows preview samples, start review, and return with typed reasons/free note. The issue action is added in Task 15 after atomic issuance exists.

- [ ] **Step 3: Analyze Svelte files and verify non-issuance workflow**

```bash
cd frontend-school
node --test tests/static/certificate-request-ui.test.mjs
npm run test:menu-sync
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test tests/e2e/certificate-request-workflow.spec.ts --grep "submit|withdraw|return"
```

- [ ] **Step 4: Commit the request UI slice**

```bash
git add frontend-school/src/lib/components/certificates frontend-school/src/routes/'(app)'/staff/certificates/'[campaignId]'/requests frontend-school/src/routes/'(app)'/staff/certificate-requests frontend-school/tests/static/certificate-request-ui.test.mjs frontend-school/tests/e2e/certificate-request-workflow.spec.ts frontend-school/tests/runtime/menu-route-registration.test.mjs
git commit -m "feat(frontend-school): review certificate issue requests"
```

### Task 14: Implement atomic numbering, issuance, issued lists, revocation, and replacement drafts

**Files:**
- Create: `backend-school/src/modules/certificates/services/issuance_service.rs`
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/modules/certificates/services.rs`
- Modify: `backend-school/src/modules/certificates/services/candidate_service.rs`
- Modify: `backend-school/src/modules/certificates/services/render_service.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `backend-school/src/modules/certificates.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/src/lib/api/certificates.ts`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`

**Interfaces:**
- `IssueCertificateRequest { idempotency_key: Uuid }` is accepted only for a reviewing request.
- `IssueCertificateOutcome` is tagged `issued` with run/number range/certificates or `returned` with row IDs and typed codes.
- `RevokeCertificateRequest { reason, create_replacement_candidate }` returns the revoked detail and optional replacement candidate.
- Admin render manifests require the matching download scope and refuse revoked certificates.

- [ ] **Step 1: Write failing first-issue, concurrency, idempotency, and rollback tests**

```rust
#[tokio::test]
async fn concurrent_first_issue_allocates_distinct_campaign_and_certificate_ranges() {
    let fixture = CertificateServiceFixture::new_with_connections("certificate_concurrent_issue", 8).await;
    let request_a = fixture.reviewing_request_with_ready_candidates(2).await;
    let request_b = fixture.reviewing_request_in_another_campaign(3).await;

    let (a, b) = tokio::join!(
        fixture.issue(request_a, Uuid::new_v4()),
        fixture.issue(request_b, Uuid::new_v4()),
    );
    let numbers = a.unwrap().numbers().into_iter().chain(b.unwrap().numbers()).collect::<HashSet<_>>();
    assert_eq!(numbers.len(), 5);
}
```

Add tests that draft creation consumes no activity number; one campaign shares sequence across templates/runs; same idempotency key returns identical outcome; a different key cannot issue an issued request; one invalid row returns the whole request and consumes no number; converted-external lookup is rechecked; upper bounds reject; revoked numbers are not reused; replacement links only after replacement issuance; all snapshot fields are immutable.

- [ ] **Step 2: Confirm the focused transactional tests fail**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::concurrent_first_issue_allocates_distinct_campaign_and_certificate_ranges -- --exact --nocapture
```

- [ ] **Step 3: Implement the transaction in the approved lock order**

Resolve school name from the authoritative admin client before opening the transaction; fail without allocating if unavailable. Inside one transaction:

1. lock or return the issue-run idempotency outcome;
2. lock request, campaign, items, candidates, and templates in deterministic order;
3. revalidate request state, active owner, accounts, names, recipient/template compatibility, layout, background, and every referenced asset;
4. for the first campaign issue, upsert and lock the academic-year counter, take `next_activity_sequence`, and advance it;
5. lock the campaign counter, ensure the whole range fits, and advance it;
6. generate formatted numbers and independent encrypted/hashed proofs, insert immutable certificate snapshots, clear draft lookup identifiers, and link replacement certificates;
7. mark candidates/request/campaign, delete active locks, insert the issue-run result and PII-minimized audit rows;
8. commit all or none.

If revalidation fails, do not touch either counter: store a `returned` issue-run, return request/candidates to review with typed codes, release locks, and commit that terminal outcome.

- [ ] **Step 4: Implement revocation and replacement candidate creation**

Lock the issued certificate, require school revoke permission, reject repeated revocation, set reason/actor/time, and prevent all render manifests. If requested, create one `replacement` import batch and candidate from immutable snapshots, linked by `replacement_for_certificate_id`; never reuse the old number or proof. The replacement candidate still requires a new issue request.

- [ ] **Step 5: Add scoped issued list/detail/render APIs**

Organization/school list scope follows campaign ownership union. Detail read and download are separate capability checks. Render service decrypts proof only in memory, constructs the canonical fragment URL from trusted tenant subdomain plus configured base domain, interpolates only template-referenced snapshot/current values, and obtains only required asset grants.

- [ ] **Step 6: Run the database, crypto, API, and architecture suites**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests -- --nocapture
cd backend-school
cargo test modules::certificates::services::numbering --bin backend-school -- --nocapture
cargo test modules::certificates::services::proof --bin backend-school -- --nocapture
cargo test api_contract::tests::certificate_contracts --bin backend-school -- --nocapture
cargo test --test static_architecture
cargo check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

- [ ] **Step 7: Commit the issuance slice**

```bash
git add backend-school/src/modules/certificates backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/certificates.ts frontend-school/src/lib/api/generated/school-api.ts
git commit -m "feat(certificates): issue and revoke numbered certificates"
```

### Task 15: Complete school issuance, revocation, and single/batch download UI

**Files:**
- Create: `frontend-school/src/lib/components/certificates/CertificateIssuedTable.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateIssueConfirmationDialog.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateRevokeDialog.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateDownloadButton.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificateBatchDownloadDialog.svelte`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/issued/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/certificates/[campaignId]/issued/+page.svelte`
- Create: `frontend-school/tests/static/certificate-issued-ui.test.mjs`
- Create: `frontend-school/tests/e2e/certificate-issuance-download.spec.ts`
- Modify: `frontend-school/src/lib/components/certificates/CertificateIssueRequestReview.svelte`
- Modify: `frontend-school/tests/e2e/certificate-request-workflow.spec.ts`

**Interfaces:**
- One click attempt creates one `crypto.randomUUID()` idempotency key and reuses it for every retry until the backend returns a typed outcome.
- Single and batch download call the same lazy renderer; batch manifest requests preserve selected order and reject more than 200 IDs before network/render work.

- [ ] **Step 1: Invoke Svelte/design skills and add failing issue/download tests**

Add static assertions for school-only issue/revoke, stable idempotency-key reuse, revoked download absence, and generated constants. Add Playwright flows for issue confirmation, network retry with the same request body key, single download, mixed-size batch download, revoke, and replacement candidate link.

- [ ] **Step 2: Implement issue confirmation and returned-outcome handling**

The review screen shows authoritative revalidation, grouped template counts, sample renders, and a clear statement that numbers are assigned only after confirmation. Keep the same key during retry; on `issued`, patch request/campaign counts and show first/last number. On `returned`, show typed row issues and remove issue controls without pretending any number was allocated.

- [ ] **Step 3: Implement issued list, revoke, replacement, and downloads**

The issued route filters status/template/number/name without exposing lookup IDs. Revocation requires a reason and optional replacement draft; after success patch the row to revoked, remove download controls, and link to the new candidate. Single/batch download requests manifests only after the exact download capability passes, lazily loads the renderer, and reports asset/render failures without changing certificate state.

- [ ] **Step 4: Analyze Svelte files and run focused browser gates**

```bash
cd frontend-school
node --test tests/static/certificate-issued-ui.test.mjs tests/static/certificate-request-ui.test.mjs tests/static/certificate-renderer.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test tests/e2e/certificate-request-workflow.spec.ts tests/e2e/certificate-issuance-download.spec.ts
```

- [ ] **Step 5: Commit the issuance UI slice**

```bash
git add frontend-school/src/lib/components/certificates frontend-school/src/routes/'(app)'/staff/certificates/'[campaignId]'/issued frontend-school/tests/static/certificate-issued-ui.test.mjs frontend-school/tests/e2e/certificate-request-workflow.spec.ts frontend-school/tests/e2e/certificate-issuance-download.spec.ts
git commit -m "feat(frontend-school): issue and download certificates"
```

### Task 16: Add secure public QR/manual verification and download

**Files:**
- Create: `backend-school/src/modules/certificates/services/verification_service.rs`
- Create: `backend-school/src/modules/certificates/verification_limiter.rs`
- Create: `frontend-school/src/lib/api/public-certificates.ts`
- Create: `frontend-school/src/lib/components/certificates/PublicCertificateVerification.svelte`
- Create: `frontend-school/src/routes/(public)/verify/certificate/+page.ts`
- Create: `frontend-school/src/routes/(public)/verify/certificate/+page.svelte`
- Create: `frontend-school/src/routes/(public)/verify/certificate/[number]/+page.ts`
- Create: `frontend-school/src/routes/(public)/verify/certificate/[number]/+page.svelte`
- Create: `frontend-school/tests/static/certificate-public-verification.test.mjs`
- Create: `frontend-school/tests/e2e/certificate-public-verification.spec.ts`
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/modules/certificates/services.rs`
- Modify: `backend-school/src/modules/certificates/services/render_service.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `backend-school/src/modules/certificates.rs`
- Modify: `backend-school/src/app.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/api_contract.rs`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`

**Interfaces:**
- `ManualCertificateVerificationRequest { certificate_number, first_name, last_name }` and `QrCertificateVerificationRequest { certificate_number, proof }` are POST-only and deny unknown fields.
- Internal `CertificateVerificationAttempt::{Manual(ManualCertificateVerificationRequest),Qr(QrCertificateVerificationRequest)}` is the single input to `verification_service::verify`; both HTTP handlers wrap their request in this enum so failure normalization cannot diverge.
- Successful verification returns allowlisted `PublicCertificateVerificationData` plus an encrypted receipt for valid issued certificates. Revoked results have no receipt.
- `PublicCertificateRenderRequest { receipt }` returns the shared manifest only after receipt tenant/certificate/action/expiry and current issued status are revalidated.

- [ ] **Step 1: Write failing generic-outcome, receipt, rate-limit, and allowlist tests**

```rust
#[tokio::test]
async fn public_verification_failures_share_one_status_and_shape() {
    let fixture = CertificateServiceFixture::new("public_verification_generic").await;
    let issued = fixture.issued_certificate().await;
    let cases = [
        fixture.manual("0000-0000-000000-0", "กมล", "ใจดี"),
        fixture.manual(&issued.number, "ชื่อผิด", "ใจดี"),
        fixture.manual(&issued.number, "กมล", "นามสกุลผิด"),
        fixture.qr(&issued.number, "invalid-proof"),
    ];
    for request in cases {
        let error = verification_service::verify(&fixture.context, request).await.unwrap_err();
        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(error.public_message(), "ไม่พบข้อมูลที่ตรงกัน");
    }
}
```

Serialize success and assert the only detail fields are status, number, title/first/last, campaign/year/template, optional activity/award, issue date, issuer school name (never the issuing actor), optional replacement number, receipt, and receipt expiry. Test receipt expiry/wrong tenant/wrong action/tampering, revoked no-render, 20-per-IP and 6-failed-target limits, and cleanup of stale limiter entries.

- [ ] **Step 2: Confirm verification tests fail**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::public_verification_failures_share_one_status_and_shape -- --exact --nocapture
```

- [ ] **Step 3: Implement no-log verification and encrypted receipts**

Public handlers use `ConnectInfo<SocketAddr>`, trusted proxy CIDRs, and `tenant_context`; they never log request bodies, names, proof, target hash, receipt, or delivery grants. Normalize number/name, hash normalized names before constant-time digest comparison, and use the domain-separated proof hash for QR lookup.

Encode a receipt payload containing version, tenant UUID, certificate UUID, action `public_render`, and UTC expiry with the existing authenticated encryption utility. Set a five-minute expiry. Rendering decrypts and validates the receipt, reloads the certificate/template, denies revoked state, and issues fresh short-lived asset grants. Replays before expiry are harmless read-only renders.

- [ ] **Step 4: Implement bounded in-memory rate limiting in AppState**

`CertificateVerificationLimiter` uses `DashMap` entries keyed by tenant plus normalized IP and by tenant/IP plus SHA-256 target digest. It accepts a test clock, returns `AppError::RateLimited`, and lazily removes entries older than fifteen minutes. Add one `Arc<CertificateVerificationLimiter>` to `AppState`; do not add a new service, secret, database table, or scheduler.

- [ ] **Step 5: Invoke Svelte/design skills and build manual/QR public pages**

Manual UI has three separate inputs. QR detail reads `location.hash` once on mount, copies proof into a local variable, immediately calls `history.replaceState` with pathname/search only, then POSTs proof without logging it. Both render generic failure. Valid issued results offer download; revoked results show revoked status and optional replacement number with no renderer call.

The public API wrapper and renderer fetch paths use no-referrer/credential omission. Add `Cache-Control: no-store` and `Referrer-Policy: no-referrer` to verification/render responses and page metadata.

- [ ] **Step 6: Verify backend, contracts, fragment removal, and public browser flow**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests -- --nocapture
cd backend-school
cargo test api_contract::tests::certificate_contracts --bin backend-school -- --nocapture
cargo test --test static_architecture
cargo check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/certificate-public-verification.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test tests/e2e/certificate-public-verification.spec.ts
```

- [ ] **Step 7: Commit the public verification slice**

```bash
git add backend-school/src/modules/certificates backend-school/src/app.rs backend-school/src/main.rs backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/public-certificates.ts frontend-school/src/lib/api/generated/school-api.ts frontend-school/src/lib/components/certificates/PublicCertificateVerification.svelte frontend-school/src/routes/'(public)'/verify frontend-school/tests/static/certificate-public-verification.test.mjs frontend-school/tests/e2e/certificate-public-verification.spec.ts
git commit -m "feat(certificates): verify certificates publicly"
```

### Task 17: Link issued certificates to staff and student personal pages

**Files:**
- Create: `frontend-school/src/lib/components/certificates/MyCertificateList.svelte`
- Create: `frontend-school/src/lib/components/achievement/SelfRecordedAchievements.svelte`
- Create: `frontend-school/src/routes/(app)/staff/achievements/+layout.svelte`
- Create: `frontend-school/src/routes/(app)/staff/achievements/issued/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/achievements/issued/+page.svelte`
- Create: `frontend-school/src/routes/(app)/staff/achievements/self-recorded/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/achievements/self-recorded/+page.svelte`
- Create: `frontend-school/src/routes/(app)/student/certificates/+page.ts`
- Create: `frontend-school/src/routes/(app)/student/certificates/+page.svelte`
- Create: `frontend-school/tests/static/certificate-own-pages.test.mjs`
- Create: `frontend-school/tests/e2e/certificate-own-pages.spec.ts`
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/modules/certificates/services/render_service.rs`
- Modify: `backend-school/src/modules/certificates/services/issuance_service.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `backend-school/src/modules/certificates.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/src/lib/api/certificates.ts`
- Modify: `frontend-school/src/routes/(app)/staff/achievements/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/achievements/+page.svelte`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/runtime/menu-route-registration.test.mjs`

**Interfaces:**
- Every `/api/me/certificates/**` service derives `user_id` from `AuthenticatedSession`; no request/query/path accepts a target user ID.
- Own list includes issued and revoked certificates linked to the current user. Only issued state can request a manifest.
- Staff self-recorded achievements preserve their existing ownership and mutation APIs under the `self-recorded` route.

- [ ] **Step 1: Write failing own-scope backend and route tests**

```rust
#[tokio::test]
async fn own_certificate_routes_cannot_read_another_linked_user() {
    let fixture = CertificateServiceFixture::new("certificate_own_scope").await;
    let actor_certificate = fixture.issued_for_user(fixture.actor.user_id).await;
    let other_certificate = fixture.issued_for_user(fixture.other_user_id).await;
    let listed = issuance_service::list_own_certificates(&fixture.pool, fixture.actor.user_id).await.unwrap();
    assert!(listed.iter().any(|item| item.id == actor_certificate.id));
    assert!(!listed.iter().any(|item| item.id == other_certificate.id));
}
```

Static route tests require route-backed issued/self-recorded tabs, lazy per-tab APIs, student menu metadata using `CERTIFICATE_READ_OWN`, and revoked download absence.

- [ ] **Step 2: Implement current-user backend endpoints**

List/detail/render queries always bind the session user ID in SQL. Internal student/staff issuance links by exact matched `users.id`; external certificates never appear in account pages. Revoke remains visible. Own render repeats status/user/template/asset validation and returns the common manifest.

- [ ] **Step 3: Invoke Svelte/design skills and refactor staff achievements carefully**

Analyze the existing staff achievements component with the Svelte skills before changing it. Extract its existing self-recorded behavior without altering API ownership, permissions, dialogs, or image workflow. Make root achievements route redirect to an accessible route-backed tab, retain one menu record with OR metadata `[PERMISSION_MODULES.ACHIEVEMENT, PERMISSIONS.CERTIFICATE_READ_OWN]`, and load each source only when its exact permission passes. A staff user with only certificate-own access lands on `issued`; a user with achievement access but no certificate-own access lands on `self-recorded`; the tab they cannot access is hidden and route-guarded.

Add the shared read-only card/list for staff-issued and student certificates. Cards show number, campaign, template, award/role, issue date, status, public verification link, and download only for issued state.

- [ ] **Step 4: Verify own isolation and both portals**

```bash
./scripts/test_backend_school.sh modules::certificates::services_tests::own_certificate_routes_cannot_read_another_linked_user -- --exact --nocapture
cd backend-school
cargo test api_contract::tests::certificate_contracts --bin backend-school -- --nocapture
cargo check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/certificate-own-pages.test.mjs
npm run test:menu-sync
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test tests/e2e/certificate-own-pages.spec.ts
```

- [ ] **Step 5: Commit the linked-account slice**

```bash
git add backend-school/src/modules/certificates backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/certificates.ts frontend-school/src/lib/api/generated/school-api.ts frontend-school/src/lib/components/achievement frontend-school/src/lib/components/certificates/MyCertificateList.svelte frontend-school/src/routes/'(app)'/staff/achievements frontend-school/src/routes/'(app)'/student/certificates frontend-school/tests/static/certificate-own-pages.test.mjs frontend-school/tests/e2e/certificate-own-pages.spec.ts frontend-school/tests/runtime/menu-route-registration.test.mjs
git commit -m "feat(certificates): link staff and student certificates"
```

### Task 18: Run the complete lifecycle gate and document repeatable verification

**Files:**
- Create: `frontend-school/tests/e2e/certificate-lifecycle.spec.ts`
- Modify: `frontend-school/tests/static/certificate-contract.test.mjs`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `docs/TESTING.md`

**Interfaces:**
- The lifecycle fixture uses only runtime-provided dedicated preparer, issuer, and student accounts and removes its draft/test resources through supported APIs.
- `docs/TESTING.md` records commands and required variable names, never credential values, proofs, render receipts, delivery grants, or recipient data.

- [ ] **Step 1: Add one full browser lifecycle test**

With dedicated runtime credentials, exercise:

1. exact-unit preparer creates a campaign and cannot choose another unit or issue;
2. preparer creates two templates with different page sizes, uploads PDF/image/font assets, confirms font rights, and saves layouts;
3. preparer adds an internal student, internal staff member, manual external recipient, and imported external competition student; resolves name mismatch and missing account warnings;
4. preparer submits selected ready rows, withdraws one request, resubmits, and receives one returned request before a final request;
5. school issuer reviews and issues; retry returns the same range; a second batch continues the campaign sequence;
6. admin downloads single and mixed-size batch PDFs;
7. linked student/staff see their own certificates, while external has no account page;
8. manual and QR public verification download valid PDFs, QR fragment disappears, wrong fields are generic;
9. issuer revokes one certificate, creates/reissues a replacement, and old public/own/admin download disappears.

- [ ] **Step 2: Add durable static privacy/architecture guards**

The guards require thin certificate handlers, generated constants, public `tenant_context`, no SQL in handlers, no `MAX(` numbering, no plaintext proof field, no import/request-body logging, no permanent PDF column/purpose, no template-history table, no raw certificate permissions outside allowed contract/migration/tests, and no heavy renderer libraries in server output.

- [ ] **Step 3: Update canonical testing instructions only**

Add focused certificate backend/static/browser commands and these environment variable names to `docs/TESTING.md`:

```text
E2E_CERT_PREPARER_USERNAME
E2E_CERT_PREPARER_PASSWORD
E2E_CERT_ISSUER_USERNAME
E2E_CERT_ISSUER_PASSWORD
E2E_CERT_STUDENT_USERNAME
E2E_CERT_STUDENT_PASSWORD
```

Do not change `docs/OPERATIONS.md`: this design adds no service, secret, scheduler, proxy route, or runtime setting.

- [ ] **Step 4: Run focused database and browser lifecycle verification**

```bash
./scripts/test_backend_school.sh modules::certificates -- --nocapture
cd frontend-school
node --test tests/static/certificate-*.test.mjs
npx playwright test --list tests/e2e/certificate-lifecycle.spec.ts
E2E_CERT_PREPARER_USERNAME="$E2E_CERT_PREPARER_USERNAME" \
E2E_CERT_PREPARER_PASSWORD="$E2E_CERT_PREPARER_PASSWORD" \
E2E_CERT_ISSUER_USERNAME="$E2E_CERT_ISSUER_USERNAME" \
E2E_CERT_ISSUER_PASSWORD="$E2E_CERT_ISSUER_PASSWORD" \
E2E_CERT_STUDENT_USERNAME="$E2E_CERT_STUDENT_USERNAME" \
E2E_CERT_STUDENT_PASSWORD="$E2E_CERT_STUDENT_PASSWORD" \
npx playwright test tests/e2e/certificate-lifecycle.spec.ts
```

If runtime credentials are unavailable, the `--list` command must pass and the live lifecycle run must be reported as unrun, not passing.

- [ ] **Step 5: Run the full change-type matrix**

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run build
cd ../backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo test api_contract::tests --bin backend-school -- --nocapture
cargo check
cd ..
git diff --check
git status --short
```

Review the final diff for generated-artifact ownership, migration immutability, public allowlists, audit contents, signed URL/proof logging, exact-unit SQL, and no changes under admin applications.

- [ ] **Step 6: Commit the evidence/documentation slice**

```bash
git diff --name-only | rg '^(backend-admin|frontend-admin)/' && exit 1 || true
git add frontend-school/tests/e2e/certificate-lifecycle.spec.ts frontend-school/tests/static/certificate-contract.test.mjs backend-school/tests/static_architecture.rs docs/TESTING.md
git commit -m "test(certificates): cover complete issuance lifecycle"
```

After the implementation pull request records the completed outcome, remove this plan and its approved design spec in that pull request's final documentation cleanup, as required by `.rules`; do not delete them before reviewers and implementers have finished using them.
