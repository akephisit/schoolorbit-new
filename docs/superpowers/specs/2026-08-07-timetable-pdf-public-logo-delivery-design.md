# Timetable PDF Public Logo Delivery Design

## Problem

Timetable PDFs obtain the configured school logo by calling the protected school-settings API, building the public File Platform content URL, and using browser `fetch` to read the image as a `Blob`. The public content endpoint returns a temporary redirect to the configured public delivery origin. A normal `<img src>` can display that cross-origin image, which is why school logos still appear on other pages, but JavaScript cannot read the redirected response body when browser CORS validation fails. The PDF generator catches the failure and silently produces a document without the configured logo.

Production checks for the affected school confirmed that public school branding returns a valid `logoFileId`, the public content URL delivers a PNG for navigation, the browser cannot read that URL through redirect-following `fetch`, and a separate browser request to the resolved public delivery URL can read the PNG. This matches the direct-fetch transport already used for private File Platform grants.

## Outcome

Timetable PDFs load the configured school logo through a typed, browser-safe public File Platform delivery flow. A configured logo is downloaded as a `Blob`, converted to a data URL, and embedded in pdfmake output for both full landscape and portrait layouts.

Existing pages that only render public files through `<img src={publicFileUrl(...)}>` keep the redirect-based content URL. The new flow is only for JavaScript consumers that must read public file bytes.

If no logo is configured, PDF generation continues without one. If a logo is configured but its delivery or conversion fails, PDF generation fails visibly so the caller can show its existing error feedback instead of silently downloading an incomplete document.

## Considered Approaches

### Add typed public delivery and fetch it directly — selected

Add `GET /api/public/files/{id}/delivery`, returning an enveloped, generated `PublicFileDeliveryResponse` containing the public delivery URL. The handler reuses `FilePlatform::public_delivery`, so it returns a URL only for a ready public file in the current tenant. The frontend keeps the URL in memory, immediately fetches it through the centralized external-blob client with omitted credentials and referrer, and returns only the resulting `Blob` to application callers.

This mirrors the private grant correction, preserves File Platform ownership, avoids a backend data proxy, and keeps the existing redirect URL stable for image and navigation consumers.

### Proxy public bytes through backend-school

A public blob endpoint could fetch the provider object server-side and stream it to the browser. This would avoid browser CORS entirely, but every logo load would traverse backend-school, add bandwidth and failure pressure to the API service, and require a second provider-to-backend streaming path for content that is already public.

### Keep the redirect and adjust CORS only

The public storage origin already permits a direct browser request, but redirect-following `fetch` still fails in the reproduced flow. CORS-only changes would not address the request-boundary behavior and would leave the PDF coupled to a browser-unsafe redirect.

## Backend and Contract Design

Backend-school adds a public, unauthenticated `GET /api/public/files/{id}/delivery` handler next to the existing content redirect. It resolves tenant context exactly as the current public content handler does, calls `FilePlatform::public_delivery`, and returns:

```text
success: true
data:
  url: <public delivery URL>
```

The URL is transport data, not persistent identity. The response type is registered in OpenAPI, and the school API contract plus generated frontend TypeScript are regenerated. The endpoint returns the same public not-found behavior for missing, private, non-ready, deleted, or wrong-tenant files. It does not add authentication, permissions, a database migration, or a new storage-provider operation.

The existing `GET /api/public/files/{id}/content` redirect remains unchanged for `<img>`, browser navigation, and external public consumers.

## Frontend Data Flow

`frontend-school/src/lib/api/files.ts` adds `downloadPublicFile(fileId, signal?)`. It requests the typed public delivery response through `apiClient`, then passes the transient URL to the existing centralized external-blob transport. Provider credentials are omitted, referrer data is suppressed, and the URL is neither logged nor persisted.

The school API wrapper adds `getRequiredPublicSchoolInfo()` for consumers that must surface branding failures while preserving the existing tolerant `getPublicSchoolInfo()` behavior used by public admission pages. The timetable PDF utility switches from the protected `getSchoolSettings` API to this strict public-branding helper, because PDF generation should not require settings-management permission. When `logoFileId` exists, it calls `downloadPublicFile`, converts the returned `Blob` to a data URL, and passes that value into the existing full and portrait PDF table builders.

No timetable page, button, permission, or selection behavior changes. All timetable export callers receive the correction through the shared PDF generator.

## Error Handling

- No configured `logoFileId`: continue generating the PDF without a logo.
- Public branding lookup failure: fail PDF generation and let the calling page show its existing export error feedback.
- Configured logo delivery, external fetch, or data-URL conversion failure: fail PDF generation rather than silently omitting the logo.
- No provider URL, file ID, response body, or signed query value is logged.

## Testing

Backend tests cover the public delivery response mapping and confirm that the handler uses the existing public-ready File Platform boundary. API-contract checks require the new operation and generated response type.

Frontend focused tests exercise the real logo-loading orchestration across injected public-branding and Blob boundaries, proving that a configured public file becomes a data URL and that configured-logo failures propagate. The timetable browser test will mock the typed public delivery call and external PNG response, then confirm PDF download still completes through the real generator.

Verification follows the change matrix in `.rules`:

- backend formatting, focused tests, static architecture, and `cargo check`;
- API contract generation and contract tests;
- frontend lint, Svelte check, menu sync, and static tests;
- focused Playwright download coverage;
- `git diff --check`, final diff review, and `git status --short`.

## Scope Boundaries

This change fixes public school-logo byte delivery for the shared timetable PDF generator. It does not change logo upload, settings management, public `<img>` rendering, private file authorization, storage buckets, provider CORS provisioning, timetable layout, or PDF branding dimensions.
