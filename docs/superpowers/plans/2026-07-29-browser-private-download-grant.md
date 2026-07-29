# Browser-Safe Private Download Grant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace browser-unsafe private-file redirects with typed, short-lived URL grants that frontend consumers fetch directly.

**Architecture:** Existing backend authorization and `FilePlatform` provider boundaries remain unchanged. The two private download handlers map an internal `DownloadGrant::Redirect` into a provider-neutral JSON DTO; shared frontend code then performs a separate credential-free CORS fetch and returns a `Blob` to existing consumers.

**Tech Stack:** Rust, Axum, serde, utoipa/OpenAPI, TypeScript, SvelteKit, Node static tests, Playwright, GitHub Actions, Cloudflare R2.

## Global Constraints

- Never edit a migration; this change requires no database work.
- Never log or persist signed URLs, credentials, national IDs, object keys, bucket names, or raw request bodies.
- Preserve existing authorization, resource policies, audit behavior, grant TTL, and private-bucket status.
- Use Rust DTOs and utoipa as the API source of truth, then regenerate OpenAPI and TypeScript.
- Do not modify backend-admin or frontend-admin.
- Keep public file delivery unchanged.
- Execute inline in the current worktree, as requested by the user.

---

### Task 1: Typed Backend Download Grant Contract

**Files:**
- Modify: `backend-school/src/modules/files/models.rs`
- Modify: `backend-school/src/modules/files/handlers.rs`
- Modify: `backend-school/src/modules/admission/handlers/portal.rs`
- Modify: `backend-school/src/api_contract.rs`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`

**Interfaces:**
- Consumes: `DownloadGrant::{Redirect, Stream}` from the existing File Platform provider boundary.
- Produces: `FileDownloadGrantResponse { url: String, expires_at: DateTime<Utc> }`, serialized as `url` and `expiresAt`.
- Produces: `POST /api/files/{id}/download -> ApiResponse<FileDownloadGrantResponse>`.
- Produces: `POST /api/admission/portal/documents/{file_id}/download -> ApiResponse<FileDownloadGrantResponse>`.

- [ ] **Step 1: Write failing DTO tests**

Add focused tests in `backend-school/src/modules/files/models.rs` that require URL grants to
serialize with camel-case expiry and stream grants to fail conversion:

```rust
#[test]
fn download_grant_response_exposes_only_temporary_delivery_fields() {
    let expires_at = Utc::now();
    let response = FileDownloadGrantResponse::try_from(DownloadGrant::Redirect {
        location: "https://provider.example/private?temporary=1".to_string(),
        expires_at,
    })
    .unwrap();
    let json = serde_json::to_value(response).unwrap();

    assert_eq!(json["url"], "https://provider.example/private?temporary=1");
    assert!(json.get("expiresAt").is_some());
    assert_eq!(json.as_object().unwrap().len(), 2);
}

#[test]
fn download_grant_response_rejects_unsupported_stream_delivery() {
    assert!(FileDownloadGrantResponse::try_from(DownloadGrant::Stream {
        content_type: "image/jpeg".to_string(),
        content_length: Some(1),
    })
    .is_err());
}
```

- [ ] **Step 2: Run the focused tests and confirm RED**

Run:

```bash
cd backend-school
cargo test modules::files::models::tests::download_grant_response --bin backend-school -- --nocapture
```

Expected: compilation fails because `FileDownloadGrantResponse` is not defined.

- [ ] **Step 3: Implement the DTO and shared conversion**

Add this provider-neutral response and conversion in `models.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileDownloadGrantResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

impl TryFrom<DownloadGrant> for FileDownloadGrantResponse {
    type Error = ();

    fn try_from(grant: DownloadGrant) -> Result<Self, Self::Error> {
        match grant {
            DownloadGrant::Redirect {
                location,
                expires_at,
            } => Ok(Self {
                url: location,
                expires_at,
            }),
            DownloadGrant::Stream { .. } => Err(()),
        }
    }
}
```

Import `chrono::{DateTime, Utc}` and `DownloadGrant` in the same module.

- [ ] **Step 4: Return the standard JSON envelope from both handlers**

Replace each redirect match with the shared conversion:

```rust
let response = FileDownloadGrantResponse::try_from(grant).map_err(|()| {
    AppError::InternalServerError("file_stream_grant_not_supported".to_string())
})?;
Ok(Json(ApiResponse::ok(response)).into_response())
```

Keep every authorization and audit call in its current order. Update both utoipa success
responses from untyped `303` to:

```rust
(status = 200, description = "Short-lived private download grant", body = ApiResponse<FileDownloadGrantResponse>)
```

- [ ] **Step 5: Register and generate the contract**

Import and register both the DTO and its envelope in `backend-school/src/api_contract.rs`:

```rust
FileDownloadGrantResponse,
ApiResponse<FileDownloadGrantResponse>,
```

Then run:

```bash
cd frontend-school
npm run generate:api-contracts
```

Expected: the tracked OpenAPI and generated TypeScript describe `200` JSON success for both
operations and contain `FileDownloadGrantResponse`.

- [ ] **Step 6: Run backend and contract-focused verification**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test modules::files::models::tests::download_grant_response --bin backend-school -- --nocapture
cargo test api_contract::tests -- --nocapture
cd ../frontend-school
npm run check:api-contracts
npm run test:api-contracts
```

Expected: all commands pass.

- [ ] **Step 7: Commit Task 1**

```bash
git add backend-school/src/modules/files/models.rs \
  backend-school/src/modules/files/handlers.rs \
  backend-school/src/modules/admission/handlers/portal.rs \
  backend-school/src/api_contract.rs \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts
git commit -m "feat: return typed private download grants"
```

---

### Task 2: Shared Browser Grant Fetch

**Files:**
- Modify: `frontend-school/tests/static/file-platform-contract.test.mjs`
- Modify: `frontend-school/src/lib/api/files.ts`
- Modify: `frontend-school/src/lib/api/admission.ts`

**Interfaces:**
- Consumes: generated `Schemas['FileDownloadGrantResponse']`.
- Produces: `downloadGrantedFile(grant, signal?) -> Promise<Blob>`.
- Preserves: `downloadFile(fileId, resourceId?, signal?) -> Promise<Blob>`.
- Preserves: `portalDownloadDocument(fileId, nationalId, dateOfBirth) -> Promise<Blob>`.

- [ ] **Step 1: Write failing frontend contract assertions**

Update `file-platform-contract.test.mjs` to require:

```javascript
assert.match(source, /export\s+type\s+FileDownloadGrantResponse\s*=\s*Schemas\['FileDownloadGrantResponse'\]/);
assert.match(source, /apiClient\s*\.\s*post<FileDownloadGrantResponse>/);
assert.match(source, /fetch\(grant\.url/);
assert.match(source, /credentials:\s*'omit'/);
assert.match(source, /referrerPolicy:\s*'no-referrer'/);
assert.doesNotMatch(source, /\.postBlob\(/);

assert.match(admission, /apiClient\.post<FileDownloadGrantResponse>/);
assert.match(admission, /downloadGrantedFile/);
assert.doesNotMatch(admission, /apiClient\.postBlobWithBody/);
```

Also assert both OpenAPI operations have a `200` response and no `303` response.

- [ ] **Step 2: Run the focused static test and confirm RED**

Run:

```bash
cd frontend-school
node --test tests/static/file-platform-contract.test.mjs
```

Expected: assertions fail because frontend code still follows redirects with blob helpers.

- [ ] **Step 3: Implement shared grant-to-blob delivery**

In `frontend-school/src/lib/api/files.ts`, export the generated type and helper:

```typescript
export type FileDownloadGrantResponse = Schemas['FileDownloadGrantResponse'];

export async function downloadGrantedFile(
    grant: FileDownloadGrantResponse,
    signal?: AbortSignal
): Promise<Blob> {
    const response = await fetch(grant.url, {
        method: 'GET',
        credentials: 'omit',
        referrerPolicy: 'no-referrer',
        signal
    });
    if (!response.ok) {
        throw new Error(`ดาวน์โหลดไฟล์ไม่สำเร็จ (${response.status})`);
    }
    return response.blob();
}
```

Change `downloadFile` to request the grant with the typed API client, unwrap it through
`requireApiData`, stop if the caller has already aborted, and call `downloadGrantedFile`.
Do not include the grant URL in any error or log.

- [ ] **Step 4: Reuse the helper for admission portal documents**

Import `downloadGrantedFile` and `FileDownloadGrantResponse` in `admission.ts`. Replace
`postBlobWithBody` with:

```typescript
const response = await apiClient.post<FileDownloadGrantResponse>(
    `/api/admission/portal/documents/${fileId}/download`,
    credentials
);
const grant = requireApiData(response, 'ไม่สามารถดาวน์โหลดเอกสารได้');
return downloadGrantedFile(grant);
```

- [ ] **Step 5: Run focused frontend checks**

Run:

```bash
cd frontend-school
node --test tests/static/file-platform-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run check:api-contracts
```

Expected: all commands pass.

- [ ] **Step 6: Commit Task 2**

```bash
git add frontend-school/tests/static/file-platform-contract.test.mjs \
  frontend-school/src/lib/api/files.ts \
  frontend-school/src/lib/api/admission.ts
git commit -m "fix: fetch private file grants directly"
```

---

### Task 3: Operations, Full Verification, and Deployment

**Files:**
- Modify: `docs/TESTING.md`
- Modify: `docs/OPERATIONS.md`

**Interfaces:**
- Documents: authorized endpoint returns a JSON grant and the browser performs the provider
  fetch without credentials.
- Does not expose: any executable example that prints the grant URL.

- [ ] **Step 1: Update the failing/stale documentation assertions**

Update the canonical File Platform smoke procedure so it no longer says the private
download endpoint returns or follows a redirect. State that tooling must parse the grant
without printing it, fetch it with `Origin`, and verify bytes. Update operations diagnosis
from “resulting redirect” to “returned short-lived grant.”

- [ ] **Step 2: Run documentation and full local verification**

Run:

```bash
cd frontend-school
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
npm run check:api-contracts
npm run test:api-contracts
cd ../backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
cargo test modules::files::models::tests::download_grant_response --bin backend-school -- --nocapture
cd ..
git diff --check
git status --short
```

Expected: all commands pass and only intended documentation files remain uncommitted.

- [ ] **Step 3: Commit Task 3**

```bash
git add docs/TESTING.md docs/OPERATIONS.md
git commit -m "docs: update private file grant operations"
```

- [ ] **Step 4: Push and monitor deployment**

```bash
git push origin main
gh run list --branch main --limit 10
backend_run_id="$(gh run list --workflow deploy-backend-school.yml --branch main --limit 1 --json databaseId --jq '.[0].databaseId')"
frontend_run_id="$(gh run list --workflow deploy-all-schools.yml --branch main --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run watch "$backend_run_id" --exit-status
gh run watch "$frontend_run_id" --exit-status
```

Expected: both deployment workflows complete successfully.

- [ ] **Step 5: Run production smoke against snwsb**

Load credentials from ignored `.env.smoke.local`, override only the old sandbox target, and
run:

```bash
set -a
source .env.smoke.local
set +a
SMOKE_ENV_FILE=/dev/null \
SMOKE_SUBDOMAIN=snwsb \
SMOKE_TENANT_URL=https://snwsb.schoolorbit.app \
SMOKE_ORIGIN=https://snwsb.schoolorbit.app \
./scripts/smoke_test.sh
```

Expected: health, readiness, CORS, login, cookie, and authenticated `/me` checks pass.

- [ ] **Step 6: Verify provider CORS without exposing the grant**

Use an in-memory script to authenticate, obtain the current profile file ID, request a fresh
typed grant, fetch it with `Origin: https://snwsb.schoolorbit.app`, and print only status,
CORS origin, content type, and byte count.

```bash
set -a
source .env.smoke.local
set +a
SMOKE_SUBDOMAIN=snwsb SMOKE_ORIGIN=https://snwsb.schoolorbit.app python3 - <<'PY'
import http.cookiejar
import json
import os
import urllib.request

api = os.environ["SMOKE_API_URL"].rstrip("/")
origin = os.environ["SMOKE_ORIGIN"]
headers = {
    "Origin": origin,
    "X-School-Subdomain": os.environ["SMOKE_SUBDOMAIN"],
    "User-Agent": "SchoolOrbit-Grant-Check/1.0",
}
cookies = http.cookiejar.CookieJar()
client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookies))
credentials = json.dumps({
    "username": os.environ["SMOKE_USERNAME"],
    "password": os.environ["SMOKE_PASSWORD"],
    "rememberMe": True,
}).encode()
request = urllib.request.Request(
    api + "/api/auth/login",
    data=credentials,
    headers={**headers, "Content-Type": "application/json"},
    method="POST",
)
with client.open(request, timeout=20) as response:
    assert response.status == 200
profile_request = urllib.request.Request(api + "/api/auth/me/profile", headers=headers)
with client.open(profile_request, timeout=20) as response:
    profile = json.load(response)["data"]
file_id = profile["profileImageFileId"]
grant_request = urllib.request.Request(
    api + f"/api/files/{file_id}/download",
    data=b"",
    headers=headers,
    method="POST",
)
with client.open(grant_request, timeout=20) as response:
    grant_status = response.status
    grant = json.load(response)["data"]
object_request = urllib.request.Request(
    grant["url"],
    headers={"Origin": origin, "User-Agent": "SchoolOrbit-Grant-Check/1.0"},
)
with urllib.request.urlopen(object_request, timeout=20) as response:
    body = response.read()
    object_status = response.status
    allowed_origin = response.headers.get("Access-Control-Allow-Origin", "missing")
    content_type = response.headers.get("Content-Type", "missing")
print(
    f"grant={grant_status} object={object_status} acao={allowed_origin} "
    f"content_type={content_type} bytes={len(body)}"
)
assert grant_status == 200
assert object_status == 200
assert allowed_origin in {origin, "*"}
assert len(body) > 0
PY
```

Expected: School API grant status `200`, object status `200`, matching
`Access-Control-Allow-Origin`, image content type, and positive bytes.

- [ ] **Step 7: Verify actual browser pixels**

Use Playwright against `/staff/profile`, capture only sanitized host-level request failures,
and assert:

```bash
cd frontend-school
set -a
source ../.env.smoke.local
set +a
node --input-type=module <<'JS'
import assert from 'node:assert/strict';
import { chromium } from 'playwright';

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext();
const page = await context.newPage();
const r2Failures = [];
page.on('requestfailed', (request) => {
    const host = new URL(request.url()).hostname;
    if (host.endsWith('r2.cloudflarestorage.com')) {
        r2Failures.push(request.failure()?.errorText ?? 'unknown');
    }
});
try {
    await page.goto('https://snwsb.schoolorbit.app/login', {
        waitUntil: 'domcontentloaded'
    });
    await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill(process.env.SMOKE_USERNAME);
    await page.getByLabel('รหัสผ่าน').fill(process.env.SMOKE_PASSWORD);
    await Promise.all([
        page.waitForURL(/\/staff\/?(?:[?#].*)?$/, { timeout: 15_000 }),
        page.getByRole('button', { name: 'เข้าสู่ระบบ' }).click()
    ]);
    await page.goto('https://snwsb.schoolorbit.app/staff/profile', {
        waitUntil: 'domcontentloaded'
    });
    const image = page.locator('main img[alt="Profile"]').first();
    await image.waitFor({ state: 'visible', timeout: 15_000 });
    await page.waitForFunction(
        (node) =>
            node.complete &&
            node.naturalWidth > 0 &&
            node.naturalHeight > 0 &&
            node.src.startsWith('blob:'),
        await image.elementHandle(),
        { timeout: 15_000 }
    );
    const dimensions = await image.evaluate((node) => ({
        width: node.naturalWidth,
        height: node.naturalHeight
    }));
    console.log(JSON.stringify({ dimensions, r2RequestFailures: r2Failures.length }));
    assert(r2Failures.length === 0);
} finally {
    await browser.close();
}
JS
```

Expected: positive dimensions and zero failed R2 requests.

- [ ] **Step 8: Record final repository and deployment state**

Run:

```bash
git diff --check
git status --short
git rev-parse --short HEAD
git rev-parse --short origin/main
gh run list --branch main --limit 5
```

Expected: clean worktree, matching local/remote commit, and successful backend/frontend
deployments.
