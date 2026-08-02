# WordPress Calendar Embed Design

**Date:** 2026-08-02

## Goal

Allow a school staff member to copy an iframe snippet from `/staff/calendar` and paste it into a WordPress Custom HTML block. The embedded calendar must stay synchronized with SchoolOrbit and expose only events explicitly marked public.

## Success Criteria

- `/calendar/embed` renders an interactive, responsive, read-only school calendar without authentication.
- The embedded view shows only data returned by `/api/public/calendar/events`.
- The embedded layout omits the full-page heading and other chrome that would duplicate the surrounding WordPress page.
- `/staff/calendar` provides a “ฝังในเว็บไซต์” action with a live preview, WordPress instructions, a visible iframe snippet, and one-click copy.
- The generated snippet uses the current school's frontend origin, works in a WordPress Custom HTML block, and remains usable when clipboard access is unavailable.
- The existing `/calendar` page and “คัดลอกลิงก์สาธารณะ” action continue to work unchanged from a user's perspective.

## Scope

### Included

- A dedicated public embed route at `/calendar/embed`.
- A shared public-calendar view used by both `/calendar` and `/calendar/embed`.
- A compact embed presentation that adapts between desktop and mobile iframe widths.
- A staff-facing embed dialog with preview, copyable code, and brief WordPress instructions.
- Route-specific framing policy that permits the read-only embed page on HTTPS websites.
- Focused frontend tests and the repository's required frontend verification.

### Excluded

- Private or audience-targeted events.
- Authenticated calendars inside WordPress.
- A WordPress plugin or shortcode package.
- iCalendar/ICS feeds.
- Per-site color, category, tag, or date-range customization.
- Automatic cross-origin iframe height messaging; the snippet uses a stable height and the embedded view owns any necessary internal scrolling.
- Database, permission-contract, or API-contract changes.

## Architecture and Components

### Shared public calendar view

Extract the current public-calendar state and interaction logic into a focused Svelte component under the existing calendar component area. It owns:

- loading public events for the visible calendar range;
- changing months and returning to today;
- selecting a date;
- desktop event details and the mobile day dialog;
- loading, empty, and error/retry states.

The component accepts an explicit presentation mode, `page` or `embed`. The mode changes layout and chrome only; it does not change the endpoint, filters, or data model. Both modes continue to use `CalendarPublicEvent` and `listPublicCalendarEvents` from the generated-contract-backed calendar API wrapper.

### Public routes

The existing `/calendar` route remains the canonical full public page and supplies the `page` presentation. It keeps its current heading, description, full-height layout, browser title, and responsive behavior.

The new `/calendar/embed` route supplies the `embed` presentation. It has no application navigation, authentication guard, management controls, or large page heading. Its root fills the iframe viewport, uses compact spacing, and allows the calendar's content pane to scroll when the supplied iframe height is smaller than its content.

The embed response declares `Content-Security-Policy: frame-ancestors 'self' https:`. This permits same-origin development preview and embedding by HTTPS school websites while avoiding a blanket framing exception for authenticated application routes. The embed route must not receive a conflicting `X-Frame-Options` header from the application or reverse proxy.

### Staff embed dialog

Add a focused calendar embed dialog to `/staff/calendar`. The action is available to users who can read the school calendar, alongside the existing public-link action. The dialog contains:

- a concise instruction to add a WordPress Custom HTML block;
- a constrained live preview of the current school's `/calendar/embed` route;
- a read-only code field so manual selection remains possible;
- a “คัดลอกโค้ด” action with success and failure feedback.

The iframe URL is derived from `page.url.origin`; no tenant hostname is stored in source code or configuration. Snippet construction lives in a small pure helper so escaping and attributes can be tested independently of the dialog.

The generated snippet has this semantic shape:

```html
<iframe
  src="https://school-subdomain.schoolorbit.app/calendar/embed"
  title="ปฏิทินโรงเรียน"
  width="100%"
  height="760"
  loading="lazy"
  sandbox="allow-scripts allow-same-origin"
  referrerpolicy="strict-origin-when-cross-origin"
  style="border:0;border-radius:12px"
></iframe>
```

The exact school origin is generated at runtime. The fixed height avoids requiring WordPress scripts or cross-origin resize messaging; the embedded page remains responsive within the available width.

## Data Flow

1. A staff member opens `/staff/calendar` and selects “ฝังในเว็บไซต์”.
2. The frontend builds the embed URL from the current SchoolOrbit tenant origin and shows the resulting snippet and preview.
3. The staff member pastes the snippet into a WordPress Custom HTML block.
4. A website visitor's browser loads `/calendar/embed` from the school's SchoolOrbit subdomain.
5. The embedded page requests `/api/public/calendar/events` through the existing frontend API client.
6. The backend resolves the tenant from the SchoolOrbit frontend origin and applies the existing mandatory public-event filter.
7. The iframe renders the returned public DTOs. Later public calendar edits appear automatically on the next load or month refresh without updating WordPress.

WordPress never receives an authenticated SchoolOrbit token, private event DTO, or management endpoint.

## Security and Privacy

- The embed page is read-only and unauthenticated.
- It consumes only `CalendarPublicEvent`, which omits targets, reminders, creator/updater identifiers, and permission data.
- The backend's existing `is_public` enforcement remains authoritative; frontend filtering is not treated as a security boundary.
- The staff preview also uses the public route, preventing a misleading preview of private events.
- The framing policy is limited to the dedicated embed route. Authenticated application pages are not made embeddable by this feature.
- The snippet uses a sandbox that permits the Svelte application scripts and same-origin API behavior but does not grant forms, popups, top navigation, downloads, or clipboard access inside the iframe.
- No secrets, tenant identifiers, or external website domains are persisted.

## Error Handling and Accessibility

- Public API failures render the existing error state and a retry action inside the iframe.
- Empty months remain valid calendar states rather than errors.
- Clipboard failure leaves the complete read-only snippet visible and selectable and shows a Thai error toast.
- The iframe has a descriptive Thai title.
- Month navigation and retry controls retain accessible labels and keyboard behavior.
- The dialog traps focus through the existing dialog primitive and returns focus to the trigger when closed.

## Testing and Verification

Focused tests cover:

- snippet generation from a tenant origin and the required iframe attributes;
- the `/calendar/embed` route's use of the public calendar view and embed mode;
- absence of authenticated calendar APIs and management controls from the embed route;
- the staff action, preview URL, visible fallback code, and copy interaction contract;
- preservation of the existing full public-calendar route;
- route-specific framing policy;
- the compact layout's explicit mobile and desktop responsive breakpoint contract.

Run the applicable verification matrix:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/calendar/PublicCalendarView.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/calendar/CalendarEmbedDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(public)/calendar/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(public)/calendar/embed/+page.svelte' --svelte-version 5
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Then run from the repository root:

```bash
git diff --check
git status --short
```

Because the design does not change the backend, database, permissions, API contract, authentication, CORS, or proxy topology, backend and smoke-test suites are not required unless implementation reveals a necessary cross-layer change. Any such discovery requires revisiting this design before expanding scope.
