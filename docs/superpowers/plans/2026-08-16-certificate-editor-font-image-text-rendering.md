# Certificate Editor Font, Image, and Thai Text Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make certificate text render complete Thai marks, preserve image aspect ratios, support reviewed multi-file static font families, and expose exact-preview loading and retry states.

**Architecture:** Extend trusted File Platform inspection and the certificate asset contract with exact font variants and image dimensions. Keep layout manipulation in pure TypeScript/Rust helpers, keep font batch authorization and atomic persistence in the certificate template service, and keep browser-only font/PDF work behind the lazy renderer. Svelte components consume generated DTOs and small pure helpers instead of duplicating variant or geometry rules.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL, `ttf-parser`, Utoipa/OpenAPI, Svelte 5 runes, TypeScript, Canvas 2D, `FontFace`, pdf-lib, pdfjs-dist, Node test runner, Playwright.

## Global Constraints

- Read and follow `.rules`; never edit applied migrations `001` through `037`.
- Never store or log plaintext national IDs, credentials, signed file grants, raw font bytes, request bodies, or recipient values.
- Use generated API DTOs and regenerate OpenAPI/TypeScript after Rust route or schema changes.
- Use the existing template update permission and resource policy for inspection and attachment.
- Static `.ttf` and `.otf` only; reject variable fonts and never synthesize bold or italic.
- Limit one reviewed selection to 40 files and upload them sequentially.
- Preserve explicit retry and cleanup for every unattached private upload.
- Run tests one command at a time; Playwright always uses `--workers=1`.
- Run `npm`, `node`, and `npx` commands from `frontend-school`; run repository scripts and Cargo commands from the repository root unless a step says otherwise.
- Run Svelte autofixer on every edited `.svelte` file before final verification.
- Add `038_certificate_font_variants.sql`; do not modify migrations `035`–`037`.

---

### Task 1: Trusted font metadata and forward-only asset schema

**Files:**
- Create: `backend-school/migrations/038_certificate_font_variants.sql`
- Modify: `backend-school/src/modules/files/platform_types.rs`
- Modify: `backend-school/src/modules/files/file_inspector.rs`
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/schema_tests.rs`

**Interfaces:**
- Produces `FontInspectionStyle::{Normal, Italic}`.
- Extends `FileInspectionMetadata::Font` with `weight`, `style`, and `is_variable`, all backward-compatible with historical JSON.
- Produces API enum `CertificateFontStyle::{Normal, Italic}` and persisted `font_style`.

- [ ] **Step 1: Write failing inspector tests**

Extend the included Sarabun tests:

```rust
assert_eq!(weight, 400);
assert_eq!(style, FontInspectionStyle::Normal);
assert!(!is_variable);

let bold = inspect_file(
    FilePurpose::CertificateTemplateFont,
    include_bytes!("../../../../frontend-school/static/fonts/Sarabun-Bold.ttf"),
).unwrap();
assert_eq!(font_weight(&bold), 700);
```

Use test-only byte helpers to set static italic metadata and append a minimal valid `fvar` table; do not add a third-party font binary.

- [ ] **Step 2: Run RED inspector test**

```bash
cargo test --manifest-path backend-school/Cargo.toml modules::files::file_inspector::tests::certificate_font_requires_a_valid_font_and_cannot_be_relabeled -- --exact --test-threads=1
```

Expected: compile/assertion failure because the fields do not exist.

- [ ] **Step 3: Implement metadata extraction**

```rust
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontInspectionStyle {
    #[default]
    Normal,
    Italic,
}
```

Populate weight with `face.weight().to_number()`, style with `face.is_italic() || face.is_oblique()`, and variability with `face.is_variable()`. Serde defaults are 400, normal, and false.

- [ ] **Step 4: Write failing migration guard and add migration 038**

The schema test must require `font_style`, the `normal` backfill, a style-aware kind-fields check, and the partial unique variant index. The migration must run a duplicate precondition, then:

```sql
ALTER TABLE certificate_template_assets ADD COLUMN font_style TEXT;
UPDATE certificate_template_assets SET font_style = 'normal' WHERE kind = 'font';
```

Replace `certificate_template_assets_kind_fields_check`, require `normal|italic` for fonts and null for images, and index `(template_id, lower(btrim(font_family)), font_weight, font_style)` where kind is font.

- [ ] **Step 5: Run GREEN focused checks**

Run the inspector command again, then separately:

```bash
./scripts/test_backend_school.sh certificate_font_variant_migration_is_forward_only -- --exact --test-threads=1
```

- [ ] **Step 6: Commit**

```bash
git add backend-school/migrations/038_certificate_font_variants.sql backend-school/src/modules/files/platform_types.rs backend-school/src/modules/files/file_inspector.rs backend-school/src/modules/certificates/models.rs backend-school/src/modules/certificates/schema_tests.rs
git commit -m "feat(certificates): inspect static font variants"
```

### Task 2: Backward-compatible text style and image aspect layout

**Files:**
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/services/layout.rs`
- Modify: `backend-school/src/modules/certificates/services/template_service.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`

**Interfaces:**
- Adds `TextElement.font_style: CertificateFontStyle` defaulting to normal.
- Adds `ImageElement.lock_aspect_ratio: bool` defaulting true and `aspect_ratio: Option<f64>`.
- Produces `normalize_layout_compatibility(&mut CertificateLayoutV1)` and `ImageElement::effective_aspect_ratio()`.

- [ ] **Step 1: Write failing serde/validation tests**

```rust
let mut layout: CertificateLayoutV1 = serde_json::from_value(legacy_layout).unwrap();
normalize_layout_compatibility(&mut layout);
let CertificateElement::Image(image) = &layout.elements[0] else { panic!() };
assert!(image.lock_aspect_ratio);
assert_eq!(image.aspect_ratio, Some(image.frame.width / image.frame.height));
```

Also prove missing `fontStyle` becomes normal, invalid ratios fail, and a locked frame/ratio mismatch fails.

- [ ] **Step 2: Run RED layout tests**

```bash
cargo test --manifest-path backend-school/Cargo.toml modules::certificates::services::layout::tests -- --test-threads=1
```

- [ ] **Step 3: Implement defaults, normalization, and validation**

```rust
#[serde(default = "default_true")]
pub lock_aspect_ratio: bool,
#[serde(default)]
#[schema(required = false)]
pub aspect_ratio: Option<f64>,
```

Normalize missing ratios from the current frame before detail responses and saves. Validate a finite positive ratio and compare locked frame ratio with an explicit tolerance. Extend expected uploaded fonts to include style.

- [ ] **Step 4: Run GREEN layout tests and focused DB service test separately**

```bash
cargo test --manifest-path backend-school/Cargo.toml modules::certificates::services::layout::tests -- --test-threads=1
```

```bash
./scripts/test_backend_school.sh certificate_layout_compatibility -- --nocapture --exact --test-threads=1
```

- [ ] **Step 5: Commit**

```bash
git add backend-school/src/modules/certificates/models.rs backend-school/src/modules/certificates/services/layout.rs backend-school/src/modules/certificates/services/template_service.rs backend-school/src/modules/certificates/services_tests.rs
git commit -m "feat(certificates): model font style and image ratios"
```

### Task 3: Font inspection and atomic batch attach API

**Files:**
- Modify: `backend-school/src/modules/certificates/models.rs`
- Modify: `backend-school/src/modules/certificates/services/template_service.rs`
- Modify: `backend-school/src/modules/certificates/handlers.rs`
- Modify: `backend-school/src/app.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: `frontend-school/src/lib/api/certificates.ts`
- Modify: `frontend-school/tests/static/certificate-contract.test.mjs`
- Generated: `contracts/openapi/school-api.json`
- Generated: `frontend-school/src/lib/api/generated/school-api.ts`

**Interfaces:**
- Produces `inspectCertificateFontUploads(templateId, { fileIds })`.
- Produces `attachCertificateFontBatch(templateId, { fileIds, rightsConfirmed })`.
- Row status is `ready | duplicate_selection | duplicate_existing | unsupported_variable | unsupported_weight | missing_family | unavailable`.
- Adds `imageWidthPixels` and `imageHeightPixels` to image asset responses from file inspection metadata.

- [ ] **Step 1: Write failing service tests for inspection and atomicity**

```rust
let result = template_service::inspect_font_uploads(
    &pool,
    &actor,
    template.id,
    InspectCertificateFontUploadsRequest { file_ids },
).await.unwrap();
assert_eq!(result.files[0].status, CertificateFontUploadStatus::Ready);
```

Add duplicate-selection, duplicate-existing, variable, wrong-purpose, cross-template, no-rights, and all-or-nothing retention/row-count cases.

- [ ] **Step 2: Run RED service test**

```bash
./scripts/test_backend_school.sh font_upload_batch_is_inspected_and_attached_atomically -- --nocapture --exact --test-threads=1
```

- [ ] **Step 3: Implement DTOs, pure classification, and asset dimensions**

Enforce 1–40 unique file IDs. Classify normalized `(family, weight, style)` deterministically. Cross-template/wrong-purpose IDs return a generic authorization error. Extend asset queries with `inspection_metadata` and expose dimensions only for image metadata.

- [ ] **Step 4: Implement locked bulk attach**

Repeat authorization under campaign-owner/template locks, reject active request locks, re-read all uploads `FOR UPDATE`, bulk insert, bulk promote, update timestamp, and write one log-safe audit row in one transaction. The existing single attach endpoint derives font metadata and no longer accepts user-supplied weight/style.

- [ ] **Step 5: Register typed routes and generate contracts**

Add:

```text
POST /api/certificates/templates/{template_id}/assets/fonts/inspect
POST /api/certificates/templates/{template_id}/assets/fonts/batch
```

Register paths/schemas, add static contract expectations, then from `frontend-school` run:

```bash
npm run generate:api-contracts
```

Add typed wrappers without casts.

- [ ] **Step 6: Run GREEN service and contract checks separately**

```bash
./scripts/test_backend_school.sh font_upload_batch_is_inspected_and_attached_atomically -- --nocapture --exact --test-threads=1
```

```bash
npm run check:api-contracts
```

- [ ] **Step 7: Commit**

```bash
git add backend-school/src/modules/certificates backend-school/src/app.rs backend-school/src/api_contract.rs frontend-school/src/lib/api/certificates.ts frontend-school/src/lib/api/generated contracts/openapi/school-api.json frontend-school/tests/static/certificate-contract.test.mjs
git commit -m "feat(certificates): attach font variants in reviewed batches"
```

### Task 4: Style-aware render manifests and issued asset validation

**Files:**
- Modify: `backend-school/src/modules/certificates/services/render_service.rs`
- Modify: `backend-school/src/modules/certificates/services/template_service.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: manifest fixtures under `frontend-school/tests/e2e/certificate-*.spec.ts`
- Modify: `frontend-school/tests/e2e/certificate-lifecycle.spec.ts`

**Interfaces:**
- Consumes exact font variants from Tasks 1–3.
- Produces style on `CertificateBuiltInFont` and `CertificateRenderFontGrant`.
- Preserves private short-lived grants and public manifest minimization.

- [ ] **Step 1: Write failing manifest tests**

```rust
assert_eq!(manifest.font_grants[0].style, CertificateFontStyle::Italic);
assert!(matches!(mismatched_style, Err(AppError::Conflict(_))));
```

Cover preview and issued manifests and prove unused assets still receive no grants.

- [ ] **Step 2: Run RED manifest test**

```bash
./scripts/test_backend_school.sh preview_manifest_preserves_exact_font_style -- --nocapture --exact --test-threads=1
```

- [ ] **Step 3: Implement style-aware queries, matching, built-ins, and grants**

Select `font_style` in template/issued asset rows, compare `(family, weight, style)`, and emit built-in Sarabun 400/700 as normal. Keep current public value/grant filtering.

- [ ] **Step 4: Update typed browser fixtures and lifecycle attach flow**

Add `style: 'normal'` to manifest fonts. Change lifecycle setup to inspect then batch-attach its Sarabun file; remove manual `fontWeight`.

- [ ] **Step 5: Run GREEN backend test and Playwright discovery separately**

```bash
./scripts/test_backend_school.sh preview_manifest_preserves_exact_font_style -- --nocapture --exact --test-threads=1
```

```bash
npx playwright test --list tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

- [ ] **Step 6: Commit**

```bash
git add backend-school/src/modules/certificates frontend-school/tests/e2e
git commit -m "feat(certificates): preserve exact font styles in render manifests"
```

### Task 5: Pure font variant and image geometry helpers

**Files:**
- Create: `frontend-school/src/lib/certificates/font-variants.ts`
- Modify: `frontend-school/src/lib/certificates/editor-state.ts`
- Modify: `frontend-school/tests/static/certificate-editor.test.mjs`

**Interfaces:**
- Produces `certificateFontVariants`, `selectFontFamily`, `selectFontWeight`, `toggleBoldVariant`, and `toggleItalicVariant`.
- Produces `imageAssetAspectRatio`, `setImageAspectRatioLock`, `resetImageAspectRatio`, and ratio-aware `resizeElement`.

- [ ] **Step 1: Write failing font-selection tests**

Cover deterministic family fallback, exact 700 Bold, unavailable italic, and a valid normal/italic pair. Assert all four fields change together:

```ts
assert.deepEqual(fontVariantPatch(italic), {
  fontSource: { type: 'asset', asset_id: 'font-italic' },
  fontFamily: 'Uploaded Thai',
  fontWeight: 400,
  fontStyle: 'italic'
});
```

- [ ] **Step 2: Write failing image geometry tests**

Use a 1200×800 asset. Assert new/reset ratio 1.5. For all eight handles at rotations 0, 45, and 90, assert locked ratio and the opposite anchor. Prove unlocked width/height remain independent.

- [ ] **Step 3: Run RED static test**

```bash
node --test tests/static/certificate-editor.test.mjs --test-concurrency=1
```

- [ ] **Step 4: Implement exact variant helpers**

```ts
export type CertificateFontVariant = {
  source: TextCertificateElement['fontSource'];
  family: string;
  weight: number;
  style: TextCertificateElement['fontStyle'];
  label: string;
};
```

Return a real variant or null; never fabricate an unavailable target.

- [ ] **Step 5: Implement ratio-aware local-axis resize**

Use stored `aspectRatio`, determine uniform scale from the active handle delta, enforce minimum size, preserve the opposite local anchor, rotate the center shift into page axes, then constrain to the page.

- [ ] **Step 6: Run GREEN static test and commit**

```bash
node --test tests/static/certificate-editor.test.mjs --test-concurrency=1
```

```bash
git add frontend-school/src/lib/certificates/font-variants.ts frontend-school/src/lib/certificates/editor-state.ts frontend-school/tests/static/certificate-editor.test.mjs
git commit -m "feat(certificates): preserve image ratios and resolve font variants"
```

### Task 6: Multi-file font review UI with durable cleanup

**Files:**
- Create: `frontend-school/src/lib/components/certificates/CertificateFontBatchUpload.svelte`
- Modify: `frontend-school/src/lib/components/certificates/CertificateAssetManager.svelte`
- Modify: `frontend-school/tests/static/certificate-template-ui.test.mjs`
- Modify: `frontend-school/tests/e2e/certificate-editor.spec.ts`

**Interfaces:**
- Consumes Task 3 inspect/batch wrappers and existing upload/delete file functions.
- Produces `onpatched(template)` after atomic attach and keeps `onpendingchange(true)` while any temporary ID remains.

- [ ] **Step 1: Write failing static/E2E UI contracts**

Assert `multiple`, `.ttf,.otf`, no manual weight field, family/weight/style/status rows, 40-file guard, one rights checkbox, and row Retry/Remove/Cleanup.

- [ ] **Step 2: Run RED static UI test**

```bash
node --test tests/static/certificate-template-ui.test.mjs --test-concurrency=1
```

- [ ] **Step 3: Implement sequential batch state**

Use a keyed `$state.raw` array with `queued | uploading | uploaded | upload_failed | ready | rejected`. Reassign after every transition. Do not discard an uploaded file ID until attachment or `deleteFile(fileId, templateId)` succeeds.

- [ ] **Step 4: Replace the single-font form and group attached variants**

Keep the image form intact. Mount the batch component in the font card and group attached variants by family with weight/style badges.

- [ ] **Step 5: Run Svelte autofixer one file at a time**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificateFontBatchUpload.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificateAssetManager.svelte --svelte-version 5
```

- [ ] **Step 6: Run GREEN static test and editor E2E separately**

```bash
node --test tests/static/certificate-template-ui.test.mjs --test-concurrency=1
```

```bash
npx playwright test tests/e2e/certificate-editor.spec.ts --workers=1
```

- [ ] **Step 7: Commit**

```bash
git add frontend-school/src/lib/components/certificates/CertificateFontBatchUpload.svelte frontend-school/src/lib/components/certificates/CertificateAssetManager.svelte frontend-school/tests/static/certificate-template-ui.test.mjs frontend-school/tests/e2e/certificate-editor.spec.ts
git commit -m "feat(certificates): review multi-file font uploads"
```

### Task 7: Editor controls, ratio dragging, and exact live fonts

**Files:**
- Modify: `frontend-school/src/lib/components/certificates/editor/CertificateElementPanel.svelte`
- Modify: `frontend-school/src/lib/components/certificates/editor/CertificateCanvas.svelte`
- Modify: `frontend-school/src/lib/components/certificates/editor/CertificateEditor.svelte`
- Modify: `frontend-school/src/lib/certificates/renderer.ts`
- Modify: `frontend-school/src/lib/certificates/renderer.browser.ts`
- Modify: `frontend-school/tests/e2e/certificate-editor.spec.ts`

**Interfaces:**
- Consumes Task 5 exact font/image helpers.
- Produces `CertificateRenderer.prepareFontAliases(manifest, layout, signal): Promise<Record<string, string>>`.
- Produces family/weight/Bold/Italic and Lock/Reset controls.

- [ ] **Step 1: Extend the editor harness with real variants and image dimensions**

Add uploaded normal/italic assets and a 1200×800 image. Assert exact asset IDs on font changes; unavailable variants disabled; new image ratio 1.5; locked drag preserved; unlock distorts; Reset returns 1.5.

- [ ] **Step 2: Run RED editor E2E**

```bash
npx playwright test tests/e2e/certificate-editor.spec.ts --workers=1
```

- [ ] **Step 3: Implement controls without parsing display strings**

Use `font-variants.ts`. Weight and style controls select exact variants. When changing an image asset, locking, or resetting, preserve current width, apply source ratio, and constrain to page.

- [ ] **Step 4: Implement lazy exact live-font preparation**

Move reusable `FontFace` loading/cache behind the renderer. Include style in cache key and descriptors. `CertificateCanvas` refreshes grants if needed, awaits aliases in an abortable effect, applies alias plus `font-style`, and hides text ink until its exact face is ready rather than showing fallback glyphs.

- [ ] **Step 5: Add live Thai ink safety**

Give the live text content a small zoom-scaled vertical safety inset and avoid clipping only glyph ink, while retaining the interactive frame, selection ring, and wrapping width. Task 8 remains authoritative for exact Canvas layout.

- [ ] **Step 6: Autofix each editor component separately**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateElementPanel.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateCanvas.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateEditor.svelte --svelte-version 5
```

- [ ] **Step 7: Run GREEN static/E2E and commit**

```bash
node --test tests/static/certificate-editor.test.mjs --test-concurrency=1
```

```bash
npx playwright test tests/e2e/certificate-editor.spec.ts --workers=1
```

```bash
git add frontend-school/src/lib/components/certificates/editor frontend-school/src/lib/certificates/renderer.ts frontend-school/src/lib/certificates/renderer.browser.ts frontend-school/tests/e2e/certificate-editor.spec.ts
git commit -m "feat(certificates): add exact font and image editor controls"
```

### Task 8: Ink-bound Thai renderer and export regression

**Files:**
- Create: `frontend-school/src/lib/certificates/text-layout.browser.ts`
- Modify: `frontend-school/src/lib/certificates/renderer.browser.ts`
- Modify: `frontend-school/tests/e2e/certificate-renderer.spec.ts`
- Modify: `frontend-school/tests/static/certificate-renderer.test.mjs`

**Interfaces:**
- Produces `measureCertificateTextLayout(context, input): MeasuredTextLayout` with line baselines and ink/shadow bounds.
- Consumes exact alias, weight, style, frame, line height, auto-shrink, and shadow.

- [ ] **Step 1: Write failing Thai clipping browser test**

Render `ปั้น น้ำ ผู้เข้าร่วม กิจกรรม` with Sarabun Bold and shadow in a tight frame. Return overlay boundary pixel information and assert no ink touches the top/bottom clip boundary. Keep preview/export pixel equivalence.

- [ ] **Step 2: Run RED renderer E2E**

```bash
npx playwright test tests/e2e/certificate-renderer.spec.ts --workers=1
```

- [ ] **Step 3: Implement metric-driven layout**

Use `textBaseline='alphabetic'` and `actualBoundingBoxAscent`, `actualBoundingBoxDescent`, `actualBoundingBoxLeft`, and `actualBoundingBoxRight`. Fall back to conservative font-size ratios if absent. Include anti-alias safety and shadow extents in width/height fitting.

- [ ] **Step 4: Draw measured baselines**

Replace top baseline and `index * lineHeight` with measured baselines. Auto-shrink calls the exact fit result. Keep frame clipping only after measured ink/shadow fit.

- [ ] **Step 5: Run GREEN static/E2E and commit**

```bash
node --test tests/static/certificate-renderer.test.mjs --test-concurrency=1
```

```bash
npx playwright test tests/e2e/certificate-renderer.spec.ts --workers=1
```

```bash
git add frontend-school/src/lib/certificates/text-layout.browser.ts frontend-school/src/lib/certificates/renderer.browser.ts frontend-school/tests/e2e/certificate-renderer.spec.ts frontend-school/tests/static/certificate-renderer.test.mjs
git commit -m "fix(certificates): preserve Thai ink in PDF rendering"
```

### Task 9: Preview spinner, retry, verification, and delivery

**Files:**
- Modify: `frontend-school/src/lib/components/certificates/editor/CertificateEditor.svelte`
- Modify: `frontend-school/tests/e2e/certificate-editor.spec.ts`
- Modify: `frontend-school/tests/static/certificate-editor.test.mjs`
- Remove after implementation is recorded: `docs/superpowers/specs/2026-08-16-certificate-editor-font-image-text-rendering-design.md`
- Remove after implementation is recorded: `docs/superpowers/plans/2026-08-16-certificate-editor-font-image-text-rendering.md`

**Interfaces:**
- Produces preview state `idle | loading | ready | error`, immediate dialog, accessible spinner, Retry/Close, and stale-request abort.

- [ ] **Step 1: Write failing delayed-font and retry E2E assertions**

Delay renderer resolution. Assert the dialog and status “กำลังโหลดฟอนต์และสร้างพรีวิว…” appear first, canvas stays hidden, a forced error exposes Retry, and Retry performs a new successful renderer call.

- [ ] **Step 2: Run RED editor E2E**

```bash
npx playwright test tests/e2e/certificate-editor.spec.ts --workers=1
```

- [ ] **Step 3: Implement explicit preview state**

Open before awaiting the manifest, store the last kind, keep loading through PDF rasterization, use `aria-busy`, abort on close, ignore stale completion, and retry with a new controller.

- [ ] **Step 4: Autofix and run focused GREEN E2E**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateEditor.svelte --svelte-version 5
```

```bash
npx playwright test tests/e2e/certificate-editor.spec.ts --workers=1
```

- [ ] **Step 5: Run API contract checks separately**

```bash
npm run check:api-contracts
```

```bash
npm run test:api-contracts
```

- [ ] **Step 6: Run backend matrix one command at a time**

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
```

```bash
./scripts/test_backend_school.sh modules::certificates -- --nocapture --test-threads=1
```

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture -- --test-threads=1
```

```bash
cargo check --manifest-path backend-school/Cargo.toml
```

- [ ] **Step 7: Run frontend matrix one command at a time**

From `frontend-school`:

```bash
npm run lint
```

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

```bash
npm run test:static
```

```bash
npx playwright test tests/e2e/certificate-renderer.spec.ts --workers=1
```

```bash
npx playwright test tests/e2e/certificate-editor.spec.ts --workers=1
```

- [ ] **Step 8: Run the live lifecycle once**

Immediately before execution, tell the user that successful issuance creates immutable sandbox audit records. Load `.env.certificate-e2e.local` without printing values, then run:

```bash
npx playwright test tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

- [ ] **Step 9: Review final tree and clean workflow artifacts**

Run each separately:

```bash
git diff --check
```

```bash
git status --short
```

Review every changed file and ensure there are no secrets or generated drift. Remove the temporary spec and plan only after implementation commits preserve them in history.

- [ ] **Step 10: Commit final verification**

```bash
git add -A
git commit -m "test(certificates): verify enhanced editor rendering"
```

- [ ] **Step 11: Publish main and monitor checks**

Use the GitHub publishing workflow with the user's previously approved direct-main path. Push `main`, inspect triggered checks, and monitor one workflow at a time. Do not claim deployment success until API contract, backend, frontend deployment, and tenant summary gates are green.
