# Public Calendar Sharing And Color Key Design

## Goal

Make the school calendar easier to share and understand by:

- adding a staff-page button that copies the current school's public calendar URL; and
- adding a compact Thai “คำอธิบายสี” section to the public calendar so visitors can identify the category represented by each event color.

## Current Behavior

The staff calendar already loads active categories and displays their colored dots beneath the month grid, but it does not provide a direct public-link action.

The public calendar loads only public events for the 42-day calendar grid. Event payloads already include the category id, name, and color, but the page does not summarize those colors. The public API does not expose a category-list endpoint.

## Design

### Public-link action

Add an outlined `คัดลอกลิงก์สาธารณะ` button to the staff calendar page header.

- Show the action to users with calendar read permission; calendar management permission is not required because the target URL is public.
- Build the URL from the current page origin plus `/calendar`, preserving the active school subdomain.
- Await `navigator.clipboard.writeText`.
- Show a success toast only after the clipboard write succeeds.
- Show an error toast if the browser denies or cannot complete the clipboard write.

### Shared color-key component

Create a focused shared calendar component for the colored-dot key and use it on both staff and public calendar pages. The visible Thai label is `คำอธิบายสี`; the English design term “legend” will not appear in the UI.

The component accepts an ordered list of `{ id, name, color }` items and renders:

- a compact section label;
- one colored dot and category name per item;
- a single horizontally scrollable row on small screens; and
- a wrapping row on larger screens.

The existing staff color list will move to this component without changing which active categories staff can see.

### Public color-key data

Derive the public color key from the already-loaded public event payload. Do not add a public categories API.

For the selected month:

1. Keep events whose date range overlaps the calendar month's first through last day.
2. Convert categorized events to their category id, current category name, and current category color.
3. Deduplicate repeated categories.
4. Group events with no category under one fallback item:
   - name: `ไม่ระบุหมวดหมู่`
   - color: `#64748b`
5. Sort named categories by Thai display name and place the fallback item last.

Events shown only in adjacent-month cells do not add entries until the user navigates to their month. If the selected month has no public events, omit the color-key row.

### Public-page placement and height

Place `คำอธิบายสี` directly below the public calendar header and above the month workspace.

- Keep it `shrink-0` so the calendar continues to receive the remaining viewport height.
- On mobile, use horizontal scrolling inside the color-key row rather than wrapping to multiple rows.
- Preserve the existing `h-dvh` page and document-level no-scroll behavior.
- Keep the mobile day timeline dialog and desktop detail panel unchanged.

## Stack Impact

- Frontend calendar utility: derive, deduplicate, and sort the selected month's public color items.
- Shared Svelte calendar component: render the reusable color key.
- Staff calendar page: add the clipboard action and replace the inline color list.
- Public calendar page: derive and render the selected month's public color key.
- Frontend tests: utility behavior and static integration guards.
- No backend, API-contract, database, migration, permission, or deployment changes.

## Error Handling

- Clipboard failure is recoverable and produces a user-facing error toast.
- Public color information uses the same event request as the calendar, so it cannot fail independently or leave a mismatched loading state.
- Missing category metadata is represented by the documented fallback item rather than silently showing an unexplained gray event.

## Verification

- Write utility tests first and confirm they fail before implementation.
- Cover month overlap, adjacent-month exclusion, category deduplication, Thai-name ordering, and uncategorized fallback placement.
- Add static assertions for the public-link action, awaited clipboard call, error toast, and shared component usage on both pages.
- Run the focused calendar utility/static tests.
- Run the Svelte autofixer on every changed Svelte file until it reports no issues or suggestions.
- Run `npm run check`, the full frontend static suite, and the production build.
- Browser-check desktop and mobile layouts, including document height, horizontal color-key overflow, clipboard success/error behavior, and unchanged day-timeline interaction.
- Run `git diff --check` and inspect repository status.
