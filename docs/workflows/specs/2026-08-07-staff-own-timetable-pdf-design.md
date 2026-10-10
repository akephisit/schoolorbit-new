# Staff Own Timetable PDF Design

## Problem

The staff self-service timetable page at `/staff/timetable` lets the signed-in teacher view their timetable by academic year and semester, but it does not provide a way to download that timetable. The academic timetable-management page already exposes a consistent outlined `ดาวน์โหลด PDF` action and generates a school-branded timetable PDF.

Teachers should be able to download only the timetable already available through their own self-service view, without requiring access to the timetable-management workspace.

## Outcome

The staff timetable page shows an outlined `ดาวน์โหลด PDF` action in the shared page header, matching the timetable-management page. Clicking it immediately downloads the signed-in teacher's currently selected timetable as one A4 landscape timetable per PDF page.

The action shows a loading spinner while the document is being generated and is unavailable while the timetable is loading, when no timetable entries or periods are available, or while another export is in progress.

## Considered Approaches

### Reuse the existing timetable PDF generator with self-service data — selected

Build one instructor-mode PDF page from the entries already loaded through `getMyTimetable`, then pass it to the existing `generateTimetablePDF` utility. This preserves the visual format, school branding, lazy PDF dependency loading, and filename behavior established by the timetable-management page while keeping authorization scoped to the current user.

### Route the request through the timetable-management export flow

This would reuse the management page's selection dialog, but it would couple teacher self-service to management-only data and permissions. It also adds unnecessary navigation and selection for a download whose target is always the current teacher.

### Use browser print styles

Printing the rendered table would avoid the PDF utility, but output would vary by browser and would not match the existing branded timetable PDF. It would also require maintaining a second document layout.

## Page and Data Flow

`frontend-school/src/routes/(app)/staff/timetable/+page.svelte` will add an `actions` snippet to `PageShell`. The action uses the existing button primitive with the same outline treatment, download icon, loading icon, and Thai label as the timetable-management page.

The download handler will use the currently selected academic year and semester, the signed-in user's display name, the loaded timetable entries, and period metadata already derived from those entries. It will normalize the period data into the existing PDF input shape, preserving period order and start/end times, and create an instructor-mode page with:

- title `ตารางสอน ครู<ชื่อครู>` without duplicating an existing `ครู` prefix;
- subtitle containing the selected semester and academic year;
- the current user's timetable entries;
- a filename containing the timetable kind, teacher name, semester, and academic year.

No additional fetch is needed when the user clicks download. Room names continue to use the PDF generator's existing room-code fallback because the self-service response does not include the management page's separate room lookup collection.

## Error and Loading Behavior

The handler sets an action-specific exporting state before generating the document and clears it in `finally`. Any generation failure produces a Thai error toast and leaves the currently displayed timetable unchanged. The export does not trigger a broad reload.

## Authorization and Contracts

The feature continues to use `/api/me/timetable`, whose backend resolves the authenticated user's timetable. It does not expose another teacher's identifier, add a permission, call a management-only endpoint, or change backend authorization.

No backend, database migration, generated permission registry, OpenAPI contract, realtime event, deployment configuration, or sensitive-data handling changes are required.

## Testing

A focused frontend static test will verify that the staff timetable page:

1. renders the download action through `PageShell`;
2. uses the existing `generateTimetablePDF` utility in instructor mode;
3. builds the export from already loaded self-service entries;
4. disables the action when loading, empty, or exporting;
5. reports failures and clears exporting state.

The test will fail against the current page before implementation and pass afterward. The changed Svelte page will be checked with the Svelte autofixer. Verification will then run the frontend matrix from `.rules`: lint, Svelte check with required public environment values, static tests, `git diff --check`, final diff review, and `git status --short`.

## Scope Boundaries

This change adds only the direct, single-teacher, full-page landscape PDF download to the staff self-service timetable. It does not add XLSX output, multi-teacher selection, layout selection, browser print mode, timetable editing, or changes to the timetable-management page.
