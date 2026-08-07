# Timetable PDF Public Logo Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the configured school logo in timetable PDFs by loading public image bytes through a typed, browser-safe File Platform delivery flow.

**Architecture:** Keep the existing redirect endpoint for `<img>` and navigation consumers, and add a public typed-delivery endpoint for JavaScript callers that must read bytes. The frontend requests that delivery URL, performs a separate credential-free external fetch through the shared API client, and gives the resulting `Blob` to a focused timetable-logo loader before pdfmake builds the document.

**Tech Stack:** Rust, Axum, Utoipa/OpenAPI, SvelteKit 5 TypeScript, generated school API types, browser `Blob`/`FileReader`, pdfmake, Node test runner, Playwright.

## Global Constraints

- Keep `GET /api/public/files/{id}/content` unchanged for existing public `<img>` and navigation consumers.
- Add `GET /api/public/files/{id}/delivery` only for ready public files in the resolved tenant; do not add authentication or permissions.
- Keep logical file IDs as persistent identity. Delivery URLs are transient in-memory transport values and must never be stored or logged.
- External provider fetches must use the centralized API client with `credentials: 'omit'` and `referrerPolicy: 'no-referrer'`.
- Use `getRequiredPublicSchoolInfo()` for PDF branding without changing the tolerant behavior of `getPublicSchoolInfo()` used by public admission pages.
- Generate a PDF without a logo only when no logo is configured. If configured branding, delivery, download, or conversion fails, reject PDF generation so the existing export error feedback is shown.
- Do not change logo upload, private file authorization, storage buckets, provider CORS, timetable layout, logo dimensions, migrations, or permission contracts.
- Regenerate the OpenAPI contract and frontend types; do not hand-edit generated artifacts.

---

### Task 1: Add the typed public File Platform delivery contract

**Files:**
- Modify: `backend-school/src/modules/files/models.rs`
- Modify: `backend-school/src/modules/files/handlers.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/tests/static/file-platform-contract.test.mjs`
- Modify: `docs/TESTING.md`

**Interfaces:**
- Consumes: `FilePlatform::public_delivery(repository, file_id) -> Result<PublicDelivery, FilePlatformError>`.
- Produces: `PublicFileDeliveryResponse { url: String }`.
- Produces: `GET /api/public/files/{id}/delivery -> ApiResponse<PublicFileDeliveryResponse>`.
- Preserves: `GET /api/public/files/{id}/content -> 307`.

- [ ] **Step 1: Write the failing backend response-model test**

In `backend-school/src/modules/files/models.rs`, import `PublicDelivery` from `platform_service` in the test module and add:

```rust
#[test]
fn public_delivery_response_exposes_only_the_delivery_url() {
    let response = PublicFileDeliveryResponse::from(PublicDelivery {
        location: url::Url::parse("https://public-files.example.test/logo.png").unwrap(),
        content_type: "image/png".to_string(),
    });
    let json = serde_json::to_value(response).unwrap();

    assert_eq!(json["url"], "https://public-files.example.test/logo.png");
    assert_eq!(json.as_object().unwrap().len(), 1);
}
```

This test catches accidental exposure of provider metadata and the absence of a typed public delivery response.

- [ ] **Step 2: Run the model test and verify the red state**

Run from `backend-school`:

```bash
cargo test public_delivery_response_exposes_only_the_delivery_url --bin backend-school
```

Expected: compilation fails because `PublicFileDeliveryResponse` does not exist.

- [ ] **Step 3: Implement the minimal response model**

In `backend-school/src/modules/files/models.rs`, import `PublicDelivery` and add:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicFileDeliveryResponse {
    pub url: String,
}

impl From<PublicDelivery> for PublicFileDeliveryResponse {
    fn from(delivery: PublicDelivery) -> Self {
        Self {
            url: delivery.location.to_string(),
        }
    }
}
```

- [ ] **Step 4: Run the focused model test and verify the green state**

Run:

```bash
cargo test public_delivery_response_exposes_only_the_delivery_url --bin backend-school
```

Expected: one passing focused test.

- [ ] **Step 5: Extend the API-contract test before registering the route**

In `backend-school/src/api_contract.rs`, extend `documents_file_platform_without_provider_locators` so its expected operations include:

```rust
(
    "/api/public/files/{id}/delivery",
    "get",
    "getPublicFileDelivery",
),
```

Add schema and response assertions:

```rust
let public_delivery = &schemas["PublicFileDeliveryResponse"];
assert!(required(public_delivery).contains(&"url"));
assert_eq!(public_delivery["properties"].as_object().unwrap().len(), 1);
assert_eq!(
    document["paths"]["/api/public/files/{id}/delivery"]["get"]["responses"]["200"]
        ["content"]["application/json"]["schema"]["$ref"],
    "#/components/schemas/ApiResponse_PublicFileDeliveryResponse",
);
```

- [ ] **Step 6: Run the API-contract test and verify the red state**

Run from `backend-school`:

```bash
cargo test documents_file_platform_without_provider_locators --bin backend-school
```

Expected: FAIL because the public delivery operation and schema are not registered.

- [ ] **Step 7: Add the public delivery handler and route**

In `backend-school/src/modules/files/handlers.rs`, import `PublicFileDeliveryResponse` and add next to the current public content handler:

```rust
#[utoipa::path(
    get,
    path = "/api/public/files/{id}/delivery",
    operation_id = "getPublicFileDelivery",
    tag = "files",
    params(("id" = Uuid, Path, description = "Logical public file ID")),
    responses(
        (status = 200, description = "Browser-safe public file delivery", body = ApiResponse<PublicFileDeliveryResponse>),
        (status = 404, description = "Public ready file not found", body = ApiErrorResponse)
    )
)]
pub async fn get_public_file_delivery(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    let repository = SqlFileRepository::new(tenant.pool);
    let delivery = state
        .file_platform
        .public_delivery(&repository, file_id)
        .await
        .map_err(map_public_platform_error)?;

    Ok(Json(ApiResponse::ok(PublicFileDeliveryResponse::from(delivery))).into_response())
}
```

In `backend-school/src/main.rs`, register the public route without authentication middleware:

```rust
.route(
    "/api/public/files/{id}/delivery",
    get(modules::files::handlers::get_public_file_delivery),
)
```

In `backend-school/src/api_contract.rs`, add the handler to `paths(...)`, add `PublicFileDeliveryResponse` and `ApiResponse<PublicFileDeliveryResponse>` to `schemas(...)`, and import the response model with the existing file model imports.

- [ ] **Step 8: Format and run the focused backend tests**

Run from `backend-school`:

```bash
cargo fmt --all
cargo test public_delivery_response_exposes_only_the_delivery_url --bin backend-school
cargo test documents_file_platform_without_provider_locators --bin backend-school
```

Expected: both focused tests pass.

- [ ] **Step 9: Add the failing generated-contract guard**

In `frontend-school/tests/static/file-platform-contract.test.mjs`, add the public delivery route to `expected`:

```js
['/api/public/files/{id}/delivery', 'get', 'getPublicFileDelivery']
```

Add literal schema assertions:

```js
const publicDelivery = contract.components?.schemas?.PublicFileDeliveryResponse;
assert.deepEqual(publicDelivery?.required, ['url']);
assert.deepEqual(Object.keys(publicDelivery?.properties ?? {}), ['url']);
assert.match(generated, /PublicFileDeliveryResponse:\s*\{/);
```

Run from `frontend-school`:

```bash
node --test tests/static/file-platform-contract.test.mjs
```

Expected: FAIL because the tracked generated contract does not contain the new operation yet.

- [ ] **Step 10: Regenerate and verify the API contract**

Run from `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/file-platform-contract.test.mjs
```

Expected: generation completes and every command passes. Inspect generated changes rather than editing them manually.

- [ ] **Step 11: Update the durable File Platform smoke procedure**

In `docs/TESTING.md`, extend the public-logo steps to state:

```text
Confirm anonymous GET /api/public/files/{id}/content still redirects and delivers the PNG. Also request GET /api/public/files/{id}/delivery, retain data.url only in memory, fetch it as a separate credential-free request with the tenant Origin and no referrer, and confirm a non-empty PNG plus matching Access-Control-Allow-Origin. Never print or persist the delivery URL.
```

Do not add a shell example that prints the response body or delivery URL.

- [ ] **Step 12: Commit the backend and generated contract**

From the repository root:

```bash
git add backend-school/src/modules/files/models.rs \
  backend-school/src/modules/files/handlers.rs \
  backend-school/src/main.rs \
  backend-school/src/api_contract.rs \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/tests/static/file-platform-contract.test.mjs \
  docs/TESTING.md
git commit -m "feat(files): expose typed public file delivery"
```

---

### Task 2: Add the centralized public-file Blob helper

**Files:**
- Modify: `frontend-school/src/lib/api/files.ts`
- Modify: `frontend-school/tests/static/file-platform-contract.test.mjs`

**Interfaces:**
- Consumes: generated `Schemas['PublicFileDeliveryResponse']`.
- Consumes: `apiClient.getExternalBlob(url, { signal })`.
- Produces: `downloadPublicFile(fileId: string, signal?: AbortSignal): Promise<Blob>`.
- Preserves: `publicFileUrl(fileId)` for `<img>` consumers and `downloadFile(...)` for authenticated private grants.

- [ ] **Step 1: Write the failing frontend contract assertions**

In the existing `typed file helper uses generated DTOs and file IDs as identity` test, add:

```js
assert.match(
    source,
    /export\s+type\s+PublicFileDeliveryResponse\s*=\s*Schemas\['PublicFileDeliveryResponse'\]/
);
assert.match(source, /export\s+async\s+function\s+downloadPublicFile\s*\(/);
assert.match(source, /apiClient\.get<PublicFileDeliveryResponse>/);
assert.match(source, /\/api\/public\/files\/\$\{fileId\}\/delivery/);
```

Keep the existing assertion that `files.ts` contains no raw `fetch(` call.

- [ ] **Step 2: Run the focused test and verify the red state**

Run from `frontend-school`:

```bash
node --test tests/static/file-platform-contract.test.mjs
```

Expected: FAIL because the generated type is not consumed and `downloadPublicFile` does not exist.

- [ ] **Step 3: Implement the public Blob helper**

In `frontend-school/src/lib/api/files.ts`, add:

```ts
export type PublicFileDeliveryResponse = Schemas['PublicFileDeliveryResponse'];

async function downloadExternalFile(url: string, signal?: AbortSignal): Promise<Blob> {
    const response = await apiClient.getExternalBlob(url, { signal });
    return requireApiData(response, 'ดาวน์โหลดไฟล์ไม่สำเร็จ');
}
```

Change `downloadGrantedFile` to delegate to `downloadExternalFile(grant.url, signal)`, then add:

```ts
export async function downloadPublicFile(fileId: string, signal?: AbortSignal): Promise<Blob> {
    const response = await apiClient.get<PublicFileDeliveryResponse>(
        `/api/public/files/${fileId}/delivery`,
        { signal }
    );
    const delivery = requireApiData(response, 'ดาวน์โหลดไฟล์สาธารณะไม่สำเร็จ');
    signal?.throwIfAborted();
    return downloadExternalFile(delivery.url, signal);
}
```

- [ ] **Step 4: Run focused frontend verification**

Run:

```bash
node --test tests/static/file-platform-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: focused contract test passes and Svelte check reports 0 errors and 0 warnings.

- [ ] **Step 5: Commit the frontend File Platform helper**

```bash
git add frontend-school/src/lib/api/files.ts \
  frontend-school/tests/static/file-platform-contract.test.mjs
git commit -m "feat(files): download public files through typed delivery"
```

---

### Task 3: Load timetable PDF branding through the public Blob flow

**Files:**
- Create: `frontend-school/src/lib/utils/timetable-pdf-logo.ts`
- Create: `frontend-school/tests/static/timetable-pdf-logo.test.mjs`
- Modify: `frontend-school/src/lib/api/school.ts`
- Modify: `frontend-school/src/lib/utils/pdf.ts`
- Modify: `frontend-school/tests/e2e/staff-own-timetable-pdf.spec.ts`

**Interfaces:**
- Produces: `getRequiredPublicSchoolInfo(): Promise<PublicSchoolInfo>`.
- Produces: `loadTimetablePdfLogoDataUrl(dependencies): Promise<string | null>`.
- Produces: `blobToDataUrl(blob: Blob): Promise<string>`.
- Consumes: `downloadPublicFile(fileId): Promise<Blob>`.
- Preserves: the existing full and portrait pdfmake builders, dimensions, and public admission fallback behavior.

- [ ] **Step 1: Write the failing pure logo-loader tests**

Create `frontend-school/tests/static/timetable-pdf-logo.test.mjs`:

```js
import assert from 'node:assert/strict';
import test from 'node:test';

test('loads a configured timetable PDF logo as a data URL', async () => {
    const module = await import('../../src/lib/utils/timetable-pdf-logo.ts').catch(() => ({}));
    assert.equal(typeof module.loadTimetablePdfLogoDataUrl, 'function');

    const result = await module.loadTimetablePdfLogoDataUrl({
        getLogoFileId: async () => 'logo-file-1',
        downloadLogo: async (fileId) => new Blob([`bytes:${fileId}`], { type: 'image/png' }),
        readLogo: async (blob) => `data:${blob.type};text,${await blob.text()}`
    });

    assert.equal(result, 'data:image/png;text,bytes:logo-file-1');
});

test('returns null only when no timetable PDF logo is configured', async () => {
    const { loadTimetablePdfLogoDataUrl } = await import(
        '../../src/lib/utils/timetable-pdf-logo.ts'
    );
    const result = await loadTimetablePdfLogoDataUrl({
        getLogoFileId: async () => undefined,
        downloadLogo: async () => {
            throw new Error('download must not run');
        },
        readLogo: async () => {
            throw new Error('conversion must not run');
        }
    });

    assert.equal(result, null);
});

test('propagates a configured timetable PDF logo delivery failure', async () => {
    const { loadTimetablePdfLogoDataUrl } = await import(
        '../../src/lib/utils/timetable-pdf-logo.ts'
    );
    const failure = new Error('public delivery failed');

    await assert.rejects(
        loadTimetablePdfLogoDataUrl({
            getLogoFileId: async () => 'logo-file-1',
            downloadLogo: async () => {
                throw failure;
            },
            readLogo: async () => 'unreachable'
        }),
        failure
    );
});
```

- [ ] **Step 2: Run the logo-loader tests and verify the red state**

Run from `frontend-school`:

```bash
node --test tests/static/timetable-pdf-logo.test.mjs
```

Expected: FAIL because the module and loader do not exist.

- [ ] **Step 3: Implement the pure logo loader and browser conversion**

Create `frontend-school/src/lib/utils/timetable-pdf-logo.ts`:

```ts
export interface TimetablePdfLogoDependencies {
    getLogoFileId: () => Promise<string | null | undefined>;
    downloadLogo: (fileId: string) => Promise<Blob>;
    readLogo: (blob: Blob) => Promise<string>;
}

export async function loadTimetablePdfLogoDataUrl(
    dependencies: TimetablePdfLogoDependencies
): Promise<string | null> {
    const logoFileId = await dependencies.getLogoFileId();
    if (!logoFileId) return null;

    const logo = await dependencies.downloadLogo(logoFileId);
    return dependencies.readLogo(logo);
}

export function blobToDataUrl(blob: Blob): Promise<string> {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => {
            if (typeof reader.result === 'string') {
                resolve(reader.result);
            } else {
                reject(new Error('แปลงโลโก้โรงเรียนไม่สำเร็จ'));
            }
        };
        reader.onerror = () => reject(reader.error ?? new Error('แปลงโลโก้โรงเรียนไม่สำเร็จ'));
        reader.readAsDataURL(blob);
    });
}
```

- [ ] **Step 4: Run the pure loader tests and verify the green state**

Run:

```bash
node --test tests/static/timetable-pdf-logo.test.mjs
```

Expected: three passing tests.

- [ ] **Step 5: Add a strict public-branding API wrapper without changing tolerant callers**

In `frontend-school/src/lib/api/school.ts`, add a shared result loader:

```ts
async function loadPublicSchoolInfo(): Promise<
    | { info: PublicSchoolInfo; error?: undefined }
    | { info: PublicSchoolInfo; error: string }
> {
    const res = await apiClient.get<PublicSchoolInfoDto>('/api/school/public');
    if (!res.success) return { info: {}, error: res.error };
    return { info: res.data ? publicSchoolInfoFromDto(res.data) : {} };
}
```

Keep the existing tolerant function:

```ts
export async function getPublicSchoolInfo(): Promise<PublicSchoolInfo> {
    return (await loadPublicSchoolInfo()).info;
}
```

Add the strict PDF-facing function:

```ts
export async function getRequiredPublicSchoolInfo(): Promise<PublicSchoolInfo> {
    const result = await loadPublicSchoolInfo();
    if (result.error) throw new Error(result.error);
    return result.info;
}
```

- [ ] **Step 6: Write the failing browser workflow expectation**

In `frontend-school/tests/e2e/staff-own-timetable-pdf.spec.ts`:

1. Define a `logoFileId` fixture and counters for typed delivery and external Blob requests.
2. Mock `/api/school/public` with `{ logoFileId, schoolName: 'ซับน้อยเหนือวิทยาคม' }`.
3. Mock `/api/public/files/${logoFileId}/delivery` with `{ url: 'https://public-files.example.test/logo.png' }` and increment the delivery counter.
4. Mock `https://public-files.example.test/logo.png` with a small valid PNG body and increment the Blob counter.
5. Return `403` from `/api/school/settings` so the test cannot pass through the legacy privileged lookup.
6. After the actual PDF download, assert both counters equal `1`.

Use this PNG fixture without adding a binary file:

```ts
const logoPng = Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z3ioAAAAASUVORK5CYII=',
    'base64'
);
```

Run against the local frontend before changing `pdf.ts`:

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run dev -- --host 127.0.0.1 --port 4173
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-own-timetable-pdf.spec.ts --project=chromium
```

Expected: FAIL because the PDF still requests `/api/school/settings` and never requests the typed public delivery or external PNG.

- [ ] **Step 7: Wire the shared PDF generator to the strict File Platform flow**

In `frontend-school/src/lib/utils/pdf.ts`:

- replace `getSchoolSettings` with `getRequiredPublicSchoolInfo`;
- replace `publicFileUrl` with `downloadPublicFile`;
- import `blobToDataUrl` and `loadTimetablePdfLogoDataUrl`;
- delete the legacy `fetchImageDataUrl(url)` function and its raw browser `fetch`;
- replace the silent logo `try/catch` with:

```ts
const logoDataUrl = await loadTimetablePdfLogoDataUrl({
    getLogoFileId: async () => (await getRequiredPublicSchoolInfo()).logoFileId,
    downloadLogo: downloadPublicFile,
    readLogo: blobToDataUrl
});
```

Do not catch this loader. A configured-logo or branding failure must reject `generateTimetablePDF`, while a missing `logoFileId` returns `null` and preserves logo-free generation.

- [ ] **Step 8: Run focused logic and browser tests**

Run from `frontend-school`:

```bash
node --test tests/static/timetable-pdf-logo.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

With the local dev server running, run:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-own-timetable-pdf.spec.ts --project=chromium
```

Expected: three focused logic tests pass, Svelte check reports 0 errors and 0 warnings, and Playwright reports one passing download workflow with one typed delivery request and one external PNG request.

- [ ] **Step 9: Commit the PDF correction**

```bash
git add frontend-school/src/lib/api/school.ts \
  frontend-school/src/lib/utils/pdf.ts \
  frontend-school/src/lib/utils/timetable-pdf-logo.ts \
  frontend-school/tests/static/timetable-pdf-logo.test.mjs \
  frontend-school/tests/e2e/staff-own-timetable-pdf.spec.ts
git commit -m "fix(pdf): load school logo through file platform"
```

---

### Task 4: Run cross-layer verification and review the final result

**Files:**
- Review all files committed in Tasks 1–3.
- No new production file is expected.

**Interfaces:**
- Verifies the typed public delivery contract, generated frontend ownership, browser-safe Blob transport, strict PDF branding behavior, and unchanged timetable layouts.

- [ ] **Step 1: Run backend formatting and checks**

From `backend-school`:

```bash
cargo fmt --all -- --check
cargo test public_delivery_response_exposes_only_the_delivery_url --bin backend-school
cargo test documents_file_platform_without_provider_locators --bin backend-school
cargo test --test static_architecture
cargo check
```

Expected: every command exits successfully.

- [ ] **Step 2: Run API-contract verification**

From `frontend-school`:

```bash
npm run check:api-contracts
npm run test:api-contracts
```

Expected: the tracked OpenAPI and generated TypeScript match the Rust contract, and generator tests pass.

- [ ] **Step 3: Run the frontend verification matrix**

From `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
```

Expected: lint exits successfully, Svelte check reports 0 errors and 0 warnings, menu sync reports 7 passing tests, and the complete static suite reports zero failures.

- [ ] **Step 4: Re-run the focused browser workflow**

Start the local frontend with the required public environment values, then run:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-own-timetable-pdf.spec.ts --project=chromium
```

Expected: one passing workflow that downloads the PDF, requests the typed public delivery once, and reads the external PNG once.

- [ ] **Step 5: Review repository integrity and the final diff**

From the repository root:

```bash
git diff --check
git status --short
git log -6 --oneline --decorate
```

Review the cumulative implementation diff against `docs/superpowers/specs/2026-08-07-timetable-pdf-public-logo-delivery-design.md`. Confirm:

- the content redirect remains `307`;
- the delivery endpoint returns only `url` for ready public files;
- no provider URL is stored or logged;
- private download authorization is unchanged;
- PDF branding uses the public strict lookup and external Blob helper;
- missing logo remains allowed, while configured-logo failures propagate;
- no migration or permission contract changed.
