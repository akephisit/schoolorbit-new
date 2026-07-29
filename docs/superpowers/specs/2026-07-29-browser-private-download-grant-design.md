# Browser-Safe Private Download Grant Design

## Problem

Private files are authorized through SchoolOrbit before the backend issues a short-lived
storage-provider grant. The current HTTP handlers expose that grant as a `303` redirect.
Browser `fetch` follows the redirect from `school-api.schoolorbit.app` to the R2 endpoint,
but the redirected `GET` does not carry an `Origin` header. R2 therefore omits its CORS
response header and the browser blocks the response.

Production evidence confirms that upload and ownership attachment succeed before the
failure: file upload returns `201`, profile update returns `200`, the download endpoint
returns `303`, and only the redirected object request fails CORS.

## Goals

- Make authenticated private images and downloads work in browsers.
- Keep authorization, resource scope, readiness checks, and audit decisions in the backend.
- Keep signed provider URLs short-lived, ephemeral, and absent from application logs.
- Preserve the storage-provider abstraction so frontend consumers do not depend on R2,
  bucket names, or object keys.
- Fix both private download entry points that currently expose the same redirect behavior:
  the authenticated File Platform endpoint and the admission portal document endpoint.
- Keep existing private-bucket CORS provisioning because the browser will fetch the granted
  URL directly.

## Non-Goals

- No database or migration change.
- No permission or ownership-policy change.
- No direct browser upload grant.
- No public-bucket delivery change.
- No changes to backend-admin or frontend-admin.
- No long-lived grant, URL persistence, or URL refresh protocol.

## Chosen Approach

The backend will return a typed, provider-neutral URL grant in the standard JSON envelope
instead of issuing an HTTP redirect:

```json
{
  "success": true,
  "data": {
    "url": "short-lived-provider-url",
    "expiresAt": "provider-grant-expiry"
  }
}
```

`FileDownloadGrantResponse` will be a Rust DTO registered in OpenAPI and generated for the
frontend. Its names describe delivery semantics, not R2 or S3. The existing internal
`DownloadGrant` provider enum remains the platform boundary. A redirect-style provider
grant maps to the URL DTO; an unsupported stream-style provider grant continues to fail
closed rather than silently changing transport behavior.

The authenticated endpoint `POST /api/files/{id}/download` and the portal endpoint
`POST /api/admission/portal/documents/{file_id}/download` will return
`ApiResponse<FileDownloadGrantResponse>` with status `200`.

## Backend Flow

1. Resolve the tenant and actor or validate admission portal credentials.
2. Load file metadata and enforce the existing file/resource access policy.
3. Ask `FilePlatform` for a bounded private download grant.
4. Audit the allowed download without recording the grant URL.
5. Map a URL-capable provider grant to the typed response and return it in `ApiResponse`.
6. Reject unsupported delivery modes with the existing safe internal error.

The two handlers will use one mapper so response shape and unsupported-mode behavior cannot
drift.

## Frontend Flow

The shared file API wrapper will:

1. Request the typed grant from School API with the existing authenticated request.
2. Fetch `grant.url` as a separate CORS request with `credentials: "omit"` and
   `referrerPolicy: "no-referrer"`.
3. Validate the provider response status and return a `Blob`.
4. Produce a provider-neutral user error without logging or persisting the signed URL.

`PrivateFileImage` continues to create and revoke a local `blob:` URL, so all current image
consumers benefit without page-specific changes. Admission portal document download will
reuse the same grant-to-blob helper after obtaining its grant with portal credentials.

## Security and Operations

- A grant remains a temporary bearer credential and must not be printed, persisted, placed
  in analytics, or included in application error messages.
- Backend authorization happens before every grant is issued.
- R2 credentials, bucket identifiers, and object keys remain server-owned.
- The browser fetch omits cookies and referrer data when contacting the provider.
- The private bucket remains non-public. Deployment continues to apply and verify only the
  required `GET`/`HEAD` CORS policy for SchoolOrbit origins.
- The current Admin-capable R2 key unblocks CORS provisioning. Separating deployment
  configuration authority from the narrower runtime object key is a later credential
  hardening task and is not required for this transport correction.

## Contract and Compatibility

This intentionally changes the two private download success responses from `303` to typed
`200` JSON. All in-repository browser consumers will be updated in the same change, and the
OpenAPI plus generated TypeScript artifacts will be regenerated. Error statuses and
authorization semantics remain unchanged.

The public file content endpoint keeps redirect delivery because direct image/navigation
requests do not need JavaScript to read a cross-origin response body.

## Verification

- Backend unit tests cover mapping a URL grant and rejecting an unsupported stream grant.
- API contract tests require the `200` JSON response and registered grant schema.
- Generated contract checks and frontend static tests require typed grant consumption,
  direct provider fetch, omitted credentials, and no-referrer behavior.
- Existing File Platform, Rust static architecture, formatting, and compile checks pass.
- Frontend lint, Svelte/TypeScript check, and static suites pass.
- Production smoke verifies health, readiness, CORS, login, and authenticated `/me`.
- A fresh authorized R2 grant returns `200`, a matching
  `Access-Control-Allow-Origin`, and non-empty image bytes.
- Playwright opens the production profile page and verifies the profile image is complete
  with positive `naturalWidth` and `naturalHeight`, with no failed R2 request.
- The final diff passes `git diff --check`, and the worktree state is reported explicitly.

## Rollback

Rollback reverts the backend and frontend contract change together. The private-bucket CORS
policy may remain because it does not make the bucket public and is required by any future
direct browser grant delivery.
