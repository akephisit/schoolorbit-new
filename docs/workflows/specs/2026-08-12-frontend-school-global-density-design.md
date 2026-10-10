# Frontend School Global Density Design

## Problem

`frontend-school` currently feels correctly proportioned only when the browser is set to 90% zoom. At the normal 100% browser setting, typography, controls, spacing, navigation, and fixed-height calendar rows consume more space, so desktop pages show less useful content and may require unnecessary scrolling.

The required result is a consistent 90%-style interface on every frontend-school route while the browser remains at 100%. This includes public, authentication, staff, student, parent, embedded-calendar, dialog, toast, and PWA prompt surfaces.

## Outcome

Every frontend-school route uses a global 90% presentation density. Rem-based typography, controls, navigation widths, icons, spacing, and calendar rows become proportionally smaller, while each page still occupies the full available viewport. Existing responsive breakpoints, viewport-relative sizing, fixed positioning, scrolling ownership, and user browser zoom continue to work normally.

The change establishes a single application-wide density baseline rather than adding page-specific compact styles.

## Considered Approaches

### Scale the root rem baseline — selected

Set the root `html` font size to 90% in the shared frontend stylesheet. Tailwind's standard type, spacing, size, and radius utilities are rem-based, so this scales the existing design consistently from one global owner. Viewport units and fixed positioning remain tied to the real viewport, preventing blank edges and clipped full-height workspaces.

Explicit pixel dimensions do not scale automatically. They remain unchanged unless visual verification demonstrates that a specific value prevents a page from satisfying the global density goal. Any such adjustment must be narrowly scoped and justified rather than turning this change into a component-by-component redesign.

### Apply CSS `zoom: 0.9`

This most closely resembles browser zoom for both rem and pixel values, but it also scales `100vh` and `100dvh` content to only 90% of the physical viewport. A Chromium diagnostic at 1920×1080 rendered a `100vh` child at 1728×972, leaving unused viewport space. Compensating for that behavior would require special handling for full-height layouts, fixed overlays, dialogs, and browser differences, so this approach is rejected.

### Reduce component tokens and classes individually

Changing each font size, control height, sidebar width, grid row, and spacing value would provide local control but spread one global requirement across many files. It would be difficult to keep new and existing routes consistent and would create a much larger regression surface. This approach is rejected.

## Global Styling Contract

The density setting belongs in `frontend-school/src/routes/layout.css`, which is imported once by the root SvelteKit layout and therefore reaches every route and body-level portal.

- `html` uses a 90% font-size baseline.
- The body keeps its existing background, text color, and natural document sizing.
- No CSS `zoom` or transform scaling is introduced.
- Viewport units remain unmodified so `h-screen`, `min-h-screen`, `h-dvh`, fixed sidebars, overlays, dialogs, and public calendar layouts continue filling the actual viewport.
- Tailwind responsive breakpoints retain their existing viewport thresholds; this is a density change, not a breakpoint redesign.
- Browser zoom and user font enlargement remain available on top of the application baseline.

## Responsive and Accessibility Behavior

The 90% baseline applies at every viewport size because the requirement covers the whole frontend rather than a desktop-only route. Existing responsive layouts continue deciding when columns stack, sidebars collapse, and controls wrap.

The change does not remove semantic labels, focus indicators, keyboard behavior, or accessible names. Text remains rem-based and can still be enlarged using browser zoom. Visual verification must check that compact controls remain readable and that no content is clipped at representative desktop and mobile viewport sizes.

## Testing and Verification

A focused Playwright regression test will load a real frontend-school route and prove the durable global contract through rendered browser behavior:

- the root layout applies a computed 14.4px font size at Chromium's default 16px baseline;
- a full-height route root still matches the physical viewport height;
- the document does not gain horizontal overflow.

Because the test visits the real landing route through SvelteKit, it also proves that the root layout imports the shared stylesheet without relying on a source-text assertion. The test will be observed failing before the stylesheet change and passing afterward. Additional browser verification will cover representative route families where local dependencies allow it: public/landing, login, public calendar, and the protected app shell. It will confirm the computed root font size, full viewport coverage, absence of unexpected horizontal overflow, and usable desktop/mobile layout. Authenticated checks that require unavailable credentials will be reported as unrun rather than inferred.

After implementation, run:

- the focused Playwright regression test;
- Svelte autofixer on any changed Svelte component;
- `npm run lint`;
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`;
- `npm run test:static`;
- `git diff --check`, final diff review, and `git status --short`.

## Impact and Scope Boundaries

This is a frontend presentation-only change. It does not alter data flow, route authorization, API or permission contracts, backend behavior, database migrations, realtime events, PII handling, deployment configuration, or business logic. It does not add a user-selectable density preference or redesign individual pages beyond narrowly necessary compatibility fixes discovered during verification.
