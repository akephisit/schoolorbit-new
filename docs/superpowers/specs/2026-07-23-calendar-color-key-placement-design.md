# Calendar Color Key Placement Design

## Goal

Make the calendar category colors feel like a quiet supporting element instead of a titled section. The staff calendar and public calendar should show only the color dot and category name, while the public calendar should place this row below the calendar grid like the staff calendar.

## Scope

- Remove the visible `คำอธิบายสี` label from the shared calendar color-key component.
- Keep an accessible container label, `หมวดหมู่กิจกรรมในปฏิทิน`, for screen readers. This text is not rendered visually.
- Keep the staff calendar color key below its month grid.
- Move the public calendar color key from above the main calendar area to below the month grid in the left calendar column.
- Preserve the public calendar's selected-month filtering, category deduplication, uncategorized fallback, desktop detail panel, mobile timeline dialog, and full-viewport layout.

## Component Design

`CalendarColorKey.svelte` remains the single shared renderer for both pages. It will render only the scrollable list of colored dots and category names. Removing the heading in the shared component guarantees the two pages stay consistent without a per-page visibility option or duplicated markup.

The public page will wrap `CalendarMonthGrid` and `CalendarColorKey` in a left-column flex container:

- the month grid receives the remaining height;
- the color key stays below the grid and does not shrink;
- the desktop selected-day panel remains in the right column;
- on small screens, the color key remains horizontally scrollable and the document must not gain vertical scrolling.

## Data and Behavior

No API or data-contract changes are required. `buildCalendarColorKey()` continues to:

- include only events overlapping the selected month;
- exclude categories used only by adjacent-month grid cells;
- deduplicate categories;
- place `ไม่ระบุหมวดหมู่` last when needed.

## Accessibility

The visible heading is removed, but the color-key container keeps an `aria-label` so assistive technology can identify the purpose of the otherwise visual color list. The new label is `หมวดหมู่กิจกรรมในปฏิทิน`.

## Verification

- Update static tests to require no visible `คำอธิบายสี` label and to verify the public color key follows the month grid.
- Run the focused calendar tests.
- Run `npm run check` and `npm run build`.
- Validate the public calendar at desktop and mobile viewport sizes, including no document-level vertical scrolling and horizontal color-key scrolling on mobile.
