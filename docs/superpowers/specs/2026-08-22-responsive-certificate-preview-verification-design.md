# Responsive Certificate Preview and Public Verification Design

Status: approved in chat on 2026-08-22; pending written-spec review before implementation planning.

## Summary

SchoolOrbit will use one shared, read-only certificate preview surface for:

1. the real-PDF preview opened from the certificate editor;
2. the candidate preview opened while staff review an issuance request; and
3. the issued-certificate preview shown after successful public verification.

The editor's interactive canvas remains separate because it must support selection, dragging, resizing, rotation, safe-area guides, and layers. Both the interactive editor and every read-only preview continue to use the same certificate layout contract and renderer, but only the three read-only contexts share the responsive preview UI and lifecycle.

The public page will automatically render an issued certificate after either manual verification or QR verification. It will never request or display a rendered preview for a revoked certificate.

This change is frontend-only. It adds no database migration, permission, endpoint, or API-contract change.

## Problem

The staff issuance-request dialog currently calculates its render scale from `window.innerWidth` and `window.innerHeight`, while the dialog itself is capped at a much narrower width. On a desktop viewport the renderer therefore creates a canvas wider than the real dialog content area, causing horizontal scrolling and preventing the reviewer from seeing the complete certificate at once.

The certificate editor's real-PDF preview independently implements similar window-based sizing and rendering state. This duplication can make preview behavior drift between workflows.

The public verification page already verifies certificate data and can download an issued certificate through the existing short-lived public render receipt, but it displays only textual registry data. A person following a QR code cannot visually compare the verified certificate with the document in front of them.

## Goals

- Show the complete certificate inside the real available preview area without cropping, stretching, or horizontal scrolling.
- Preserve the certificate's paper aspect ratio for portrait, landscape, and supported custom page geometry.
- Share loading, rendering, fitting, retry, cancellation, and fullscreen behavior across all read-only preview contexts.
- Automatically show the actual issued certificate after successful manual or QR verification.
- Keep public verification useful even if preview rendering fails.
- Preserve the existing short-lived receipt, signed asset grants, QR-proof handling, and revoked-certificate restrictions.
- Keep the rendered preview sharp without creating unbounded canvas memory usage.
- Work on desktop, tablet, and mobile, including viewport changes after a preview opens.

## Non-goals

- Replacing the editor's interactive `CertificateCanvas` with a bitmap preview.
- Adding zoom-in, zoom-out, pan, or freeform viewer controls. The default is fit-to-area plus fullscreen.
- Generating or storing durable PNG/JPEG thumbnails.
- Embedding the browser's PDF viewer.
- Changing certificate layout, issuance, numbering, verification fields, or download semantics.
- Displaying a revoked certificate's old visual.
- Adding public access to drafts, candidates, templates, raw object keys, or permanent asset URLs.

## Considered Approaches

### 1. Shared browser-rendered preview surface — selected

Reuse the existing certificate renderer and render manifest. Measure the actual preview stage, render at a bounded device-aware resolution, and display the canvas at its logical fit size.

Advantages:

- uses the same layout, fonts, images, shadows, QR code, and PDF pipeline as downloads;
- requires no new storage or invalidation lifecycle;
- preserves the current short-lived authorization model;
- centralizes behavior that is currently duplicated.

Trade-off: the browser must lazily load the renderer and assets before displaying the image, so clear loading feedback is required.

### 2. Persisted server-generated image thumbnail — rejected

Generate and store a PNG for each issued certificate. This would make repeat views faster, but it creates another durable derivative that must be regenerated after template changes and deleted with certificate/campaign file lifecycles. It also expands backend, storage, and purge scope for no current requirement.

### 3. Embedded PDF viewer — rejected

Display a PDF in an iframe or browser viewer. Browser controls, sizing, font behavior, and mobile support vary, making it harder to guarantee a complete, branded, accessible preview. It also does not solve shared loading and lifecycle behavior cleanly.

## Architecture

### Shared read-only preview boundary

Add a focused shared component named `CertificatePreviewSurface.svelte` under the certificate component area. It owns only presentation and rendering behavior:

- a measured stage that preserves its allocated layout space;
- the canvas and certificate-paper visual treatment;
- renderer lazy loading and font/image preparation through the existing renderer;
- fit-to-area calculation from the stage's content box;
- a bounded, device-pixel-aware render resolution;
- loading, ready, error, retry, and fullscreen presentation;
- cancellation when the manifest identity changes or the component unmounts;
- accessible labels and status announcements.

The surface receives a typed `CertificateRenderManifest` and parent-owned manifest-loading state. It must not know how staff or public authorization works. Fetching a manifest, refreshing a receipt, and deciding whether a preview is allowed remain with the owning workflow.

Add a small shared dialog wrapper named `CertificatePreviewDialog.svelte` for the editor and staff request review. It provides the consistent header, description, close action, fullscreen action, and flex layout around `CertificatePreviewSurface`. The public page embeds the same surface directly and uses the same fullscreen presentation without adopting staff-dialog copy or controls.

The fit calculation lives in a pure TypeScript helper so it can be unit tested without rendering a Svelte component.

### Interactive editor boundary

`CertificateCanvas.svelte` remains the interactive design surface. It continues to render the locked background and editable DOM overlays so selection handles, drag/resize/rotate interactions, guides, and immediate layout feedback remain available.

The editor's **real PDF preview** dialog will use the shared read-only preview component. Its parent still requests a fresh preview manifest containing the unsaved layout snapshot and the chosen short/normal/long sample values. Consequently:

- the design workspace remains fast and editable;
- the real preview remains authoritative for export appearance; and
- editor, staff review, and public verification no longer duplicate fit/loading UI.

### No backend or contract changes

The design reuses:

- `createCertificateTemplatePreviewManifest` for the editor and staff candidate preview;
- `verifyCertificateManually` and `verifyCertificateByQr` for public verification;
- `createPublicCertificateRenderManifest` for an issued public certificate;
- `loadCertificateRenderer().renderPreview(...)` for the visual;
- `buildCertificatePdf(...)` for download.

No response field needs to be added because an issued public verification result already includes the short-lived render receipt required by the public manifest endpoint.

## Responsive Fitting

The preview must be sized from the preview stage, not the browser window or a presumed dialog width.

1. Observe the stage's actual content-box dimensions with `ResizeObserver`.
2. Subtract intentional stage padding from the usable dimensions.
3. Read the displayed page width and height from `manifest.pageGeometry`.
4. Compute the logical fit scale as the smaller of available-width/page-width and available-height/page-height.
5. Never alter the relationship between page width and page height.
6. Render at a bounded device-aware pixel scale and display the canvas at the logical fit dimensions. Cap the resolution multiplier to prevent excessive memory use on high-DPI or very large screens.
7. Recalculate after dialog opening, fullscreen changes, sidebar/viewport resizing, and mobile orientation changes.
8. Cancel or replace obsolete render work rather than allowing an older resize render to overwrite a newer one.

The ordinary fit mode must show the complete page with no horizontal scrollbar. Fullscreen uses the same calculation against the fullscreen stage. This scope has no 100% or zoom mode and therefore introduces no preview scrollbar.

The dialog is a flex column constrained to `96vw` and `94dvh`: header and actions do not scroll, while the preview stage receives the remaining `min-height: 0` space. The certificate is centered on a quiet blue-gray work surface with a subtle paper edge and shadow.

## Rendering State Model

Manifest acquisition and canvas rendering are distinct operations, but the user sees one coherent preview state:

- `idle`: no eligible certificate or preview has been requested;
- `loading`: manifest, renderer, fonts, images, or canvas output is being prepared;
- `ready`: the complete current certificate is visible;
- `error`: the current preview failed and can be retried.

While loading, the component reserves the preview's stage and shows a spinner with the Thai copy `กำลังโหลดฟอนต์และสร้างตัวอย่าง…` for staff/editor or `กำลังสร้างภาพเกียรติบัตร…` for public verification. The old certificate canvas must not remain visible beneath a new loading state.

Each owning workflow and the renderer use an `AbortController` plus a request identity. Opening another candidate, verifying another number, closing a dialog, navigating, or unmounting invalidates the prior identity and aborts in-flight work. A late response must never update the current surface.

Preview errors are isolated from verification results. Public users continue to see the verified status and registry details if manifest loading, asset loading, or rendering fails. Error UI uses safe Thai copy and a `ลองโหลดภาพอีกครั้ง` action without exposing raw asset URLs, receipt values, internal error bodies, or object keys.

## Staff and Editor Preview Experience

### Issuance-request review

- Opening a candidate preview immediately enters the loading state.
- The existing candidate-specific template manifest is requested.
- The dialog shows the recipient name and retains the notice that the preview is not yet an issued certificate.
- When ready, the complete page is visible without a horizontal scrollbar.
- The user can expand the same preview fullscreen and press `Escape` to return.
- Closing the dialog aborts manifest and render work and clears the candidate identity.
- A failure provides retry and close actions.

### Certificate editor real-PDF preview

- The existing preview kinds and unsaved layout snapshot are preserved.
- Opening preview immediately shows the shared loading state.
- The shared surface renders the fresh manifest using the same authoritative PDF renderer.
- The complete page fits the dialog by default, with the same fullscreen and retry behavior as staff review.
- The interactive editor canvas itself is unchanged by this refactor.

## Public Verification Experience

### Visual direction

The page remains a school certificate registry rather than a generic marketing card. Existing identity tokens continue:

- registry ink `#17324d`;
- registry blue `#2d648c`;
- registry mist `#eaf2f7`;
- registry line `#c8d8e4`;
- registry gold `#b9872e`;
- verified green `#167055`;
- revoked red `#a23d46`.

Sarabun-compatible Thai typography remains the primary reading face, while certificate numbers use the existing monospaced utility treatment. The distinguishing element is the registry rail/status treatment tied directly to the certificate number and issuer, not decorative gradients or unrelated imagery.

### Initial and verifying states

The initial page retains the manual number, first-name, and last-name form and the explanation for QR verification. Verification has its own busy state. The certificate preview area is not requested until verification succeeds with `status === 'issued'` and a non-empty receipt.

### Issued result

After successful manual or QR verification:

1. keep the verified registry result visible;
2. automatically request the public render manifest using the returned receipt;
3. immediately reserve the preview area and show its spinner;
4. render and display the complete certificate;
5. provide `ขยายเต็มจอ`, `ดาวน์โหลด PDF`, and `ตรวจสอบหมายเลขอื่น` actions.

On desktop the successful result expands to a results-oriented layout: the certificate preview is the wide primary region, while a narrower registry panel shows verified status, recipient, campaign, template, optional activity/award data, issue date, academic year, issuer, and certificate number. On mobile the order is status, preview, details, and actions.

The existing download path remains independent. A preview failure does not disable download while the receipt remains valid, and a download failure does not remove a successful preview.

### QR flow

The current QR proof continues to be read from the URL fragment and removed from the visible URL with `history.replaceState` before network work proceeds. The proof is retained only in component memory for the active verification session so a user-initiated retry can obtain a fresh receipt if the short-lived receipt expires. It must never be placed in local storage, session storage, logs, analytics, error text, or a new URL.

Both manual and QR flows converge on the same issued-result and automatic-preview path after verification. This prevents behavior or styling from diverging by entry method.

### Expiry and retry

The first preview request occurs immediately, well within the existing five-minute receipt lifetime. If the manifest request fails because the receipt expired, the UI does not enter an automatic retry loop. When the user chooses retry, the workflow repeats the original verification once using the current in-memory manual inputs or QR proof, receives a new receipt, and requests a new manifest. One user action produces at most one re-verification attempt. Generic public failure copy is retained to avoid disclosing whether a certificate, person, proof, receipt, or asset exists.

### Revoked or invalid result

- A revoked result displays a prominent red registry warning and any public replacement certificate number.
- It does not call the public render-manifest endpoint, load the renderer, display an old preview, or offer PDF download.
- Invalid verification displays the existing generic failure and no preview.
- Beginning a new verification immediately aborts and clears the prior preview before sending the new request.

## Accessibility and Interaction

- Loading regions use `role="status"`, `aria-live="polite"`, and accurate `aria-busy` values.
- Errors are announced without repeatedly stealing focus.
- Fullscreen uses the existing accessible dialog primitives, has an explicit title, traps focus, supports a visible close action, and exits with `Escape`.
- Buttons have visible keyboard focus and stable labels matching the resulting status text.
- The rendered canvas has a meaningful label; registry details remain real text rather than being available only inside the image.
- Motion is limited to the loading spinner and small existing interactions and respects reduced-motion preferences.
- Touch targets remain usable when header actions wrap on narrow screens.

## Security and Privacy

- Only `issued` public results with a valid short-lived receipt may request a public render manifest.
- The frontend gate is user experience, not authorization; the backend's receipt validation, status check, target/IP rate limits, and signed grants remain authoritative.
- Revoked results must be covered by tests proving that neither the manifest endpoint nor renderer is invoked.
- The preview component receives only the already-authorized manifest and does not fetch arbitrary file identifiers.
- Signed URLs, render receipts, QR proofs, object keys, and internal errors are never rendered into user-visible diagnostics or persisted client-side.
- New verification clears the prior manifest and canvas immediately to prevent one recipient's certificate from appearing under another recipient's result.
- Existing `Referrer-Policy`, cookie-free public requests, and QR fragment scrubbing remain unchanged.

## Expected Code Boundaries

The implementation scope is:

- add the shared responsive preview surface and staff/editor dialog wrapper under `frontend-school/src/lib/components/certificates/`;
- add or extend a pure preview-fit helper under `frontend-school/src/lib/certificates/`;
- replace duplicated real-preview canvas/dialog logic in `CertificateEditor.svelte`;
- replace duplicated candidate-preview canvas/dialog logic in `CertificateIssueRequestReview.svelte`;
- extend `PublicCertificateVerification.svelte` with automatic issued preview, responsive result layout, fullscreen, retry, and independent download state;
- retain `CertificateCanvas.svelte` as the interactive editor surface;
- extend focused unit/static/Playwright coverage.

No backend, migration, permission contract, OpenAPI artifact, or generated TypeScript DTO will change.

## Testing Strategy

Tests must be run sequentially, not concurrently.

### Pure/unit coverage

- landscape and portrait fit calculations choose the limiting dimension correctly;
- page aspect ratio is preserved;
- zero/unmeasured stages do not start a render;
- device-pixel scaling is bounded;
- resize results cannot be superseded by stale work.

### Staff/editor browser coverage

- candidate preview in a constrained desktop dialog shows the complete page with no horizontal overflow;
- opening another candidate cancels the former render and never shows the former result;
- editor real-PDF preview uses the unsaved layout snapshot and shared loading/fit behavior;
- spinner, retry, close, fullscreen, and `Escape` work;
- portrait, landscape, and narrow viewport cases are covered.

### Public verification browser coverage

- manual issued verification automatically requests a manifest and renders a preview;
- QR issued verification scrubs the fragment and automatically renders the same preview experience;
- the spinner is visible while the renderer or font/assets are pending;
- a preview failure leaves verified text visible and retry succeeds;
- starting a second verification aborts/invalidates the prior preview;
- download remains available independently from preview state;
- revoked and invalid results request no manifest, renderer, or download;
- mobile ordering and fullscreen keyboard behavior are verified.

### Project verification

After focused tests, run the frontend matrix from `.rules` one command at a time:

1. Svelte autofixer for every changed Svelte component;
2. `npm run lint`;
3. `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`;
4. `npm run test:static`;
5. focused Playwright certificate workflows using configured E2E credentials where required;
6. `git diff --check`;
7. final diff and `git status --short` review.

## Acceptance Criteria

- Staff can open a candidate preview and see the complete certificate without horizontal scrolling at the screenshot's desktop viewport.
- Editor real-PDF preview, staff request preview, and public issued preview use the same read-only preview surface and renderer behavior.
- The editor's drag-and-drop canvas continues to support all existing editing interactions.
- Manual public verification of an issued certificate automatically displays its actual certificate.
- Opening a valid issued-certificate QR link automatically verifies and displays the actual certificate.
- Loading fonts/assets/rendering always has visible progress feedback.
- Preview resizing preserves the paper ratio and does not crop or stretch the certificate.
- Fullscreen works with keyboard and touch, and exits with `Escape`.
- Preview failure does not erase a successful public verification result.
- Revoked or invalid certificates never request or display a preview or PDF.
- No API, database, permission, or durable-file lifecycle change is introduced.
