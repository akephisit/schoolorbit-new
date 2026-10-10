# Daily Teaching shadcn Date Picker Design

## Problem

The Daily Teaching Today filter toolbar uses a native `<input type="date">` while the project already owns a shared shadcn-svelte date picker. The native control renders differently across browsers, does not follow the application's Thai calendar presentation, and breaks the project's convention that standard date controls use the local shadcn-svelte primitives.

## Outcome

The toolbar date control uses the shared `DatePicker` from `$lib/components/ui/date-picker`. It displays the selected ISO date using the component's Thai abbreviated month and Buddhist year formatting, and opens the shared `Popover + Calendar + Button` interaction.

The previous-day and next-day icon buttons remain on either side of the date picker. They preserve the current fast day-by-day navigation, while the calendar popover supports non-adjacent date selection.

## Considered Approaches

### Shared DatePicker with adjacent navigation — selected

Replace only the native input and keep both arrow buttons. This follows the local component system without changing the existing navigation workflow or selected-date data flow.

### DatePicker without navigation buttons

Removing the arrows would make the toolbar simpler, but moving through consecutive school days would require reopening the popover for every date.

### Page-local Popover and Calendar composition

Composing shadcn primitives directly in the page would allow page-specific behavior, but it would duplicate the existing DatePicker's ISO conversion and Thai display formatting.

## Component and Data Flow

The page imports `DatePicker` from the shared date-picker index and replaces the native date input with:

```svelte
<DatePicker
	id="teaching-date"
	bind:value={selectedDate}
	placeholder="เลือกวันที่"
	class="min-w-0 flex-1"
/>
```

`selectedDate` remains an ISO `YYYY-MM-DD` string. Existing `$effect` dependency tracking, overview request keys, API query parameters, date formatting, and previous/next navigation continue to consume that same state. No conversion layer or additional state is introduced.

Both arrow buttons receive `shrink-0`; the DatePicker trigger receives `min-w-0 flex-1`. The three controls therefore remain one compact row at narrow widths, with the selected Thai date truncating within the central control rather than forcing horizontal overflow.

## Accessibility and Interaction

The existing visible `Label` remains associated with the DatePicker trigger through `id="teaching-date"`. Previous and next buttons keep their Thai `aria-label` values. The shared DatePicker owns keyboard focus, popover positioning, single-date selection, and the localized calendar interaction.

Selecting a date immediately changes `selectedDate`; the page's existing overview load effect then requests that date. Popover visibility remains owned by the unchanged shared DatePicker behavior. The arrow buttons continue to update `selectedDate` directly using the existing `moveDate` function.

## Testing and Verification

A focused static behavior guard for the Daily Teaching page will assert that the route uses the shared DatePicker with `selectedDate` binding and no longer renders a native teaching-date input. The guard will be written and observed failing before the Svelte page change.

The page will be formatted and checked with the Svelte autofixer. Verification will run:

- focused Daily Teaching static tests;
- `npm run lint`;
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`;
- `npm run test:menu-sync`;
- `npm run test:static`;
- `git diff --check`, final diff review, and `git status --short`.

## Impact and Scope Boundaries

This is a frontend component substitution on one route. It does not change date semantics, API contracts, backend queries, permissions, migrations, realtime behavior, timetable data, activity rendering, responsive table widths, or the shared DatePicker implementation.
