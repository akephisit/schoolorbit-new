# Daily Teaching Simple Cells Design

## Problem

The responsive daily teaching table now fits more periods on screen, but each occupied cell still contains a neutral nested card and up to three badges before the lesson title. The repeated chrome competes with the actual schedule data and makes a full day slower to scan.

The main timetable already uses a simpler visual language: a flat pastel block, a thin semantic border, strong primary text, and small supporting metadata. The read-only daily overview should use the same recognizable pattern while keeping its teacher-oriented grouping and synchronized-activity compaction.

## Outcome

Each displayed entry group becomes one flat color-coded block without type, team-teaching, or synchronized badges in the grid. Color and content structure communicate the entry type:

- courses use a pale blue block with left-aligned code, subject name, classroom, and room;
- academic entries use a pale blue block with a centered title;
- activities and homeroom entries use a pale green block with a centered title;
- breaks use a pale amber block with a centered title.

Synchronized activities show the activity title and a small classroom count. Team-teaching and synchronized status remain available in the existing period detail dialog. Empty cells are visually blank.

## Considered Approaches

### Semantic pastel blocks — selected

Three restrained semantic color families make courses, activities, and breaks recognizable at a glance. This matches the reference timetable and removes the need for repeated type badges.

### White blocks with colored edge

A white surface with one colored edge would be quieter, but entry types would be less distinguishable across a dense full-school grid.

### Subject-derived pastel colors

Generating a color from each subject code could distinguish individual subjects, but it would introduce many unrelated colors and weaken the simple type-level scan requested for this page.

## Grid Presentation Model

The existing pure daily-teaching display helper will own a small presentation model for entry types. Given `DailyTeachingEntry.entryType`, it returns one of three tones (`course`, `activity`, or `break`) and one of two content layouts (`details` or `centered`).

- `COURSE` maps to `course` + `details`.
- `ACADEMIC` maps to `course` + `centered`.
- `ACTIVITY` and `HOMEROOM` map to `activity` + `centered`.
- `BREAK` maps to `break` + `centered`.

The Svelte page maps those stable presentation values to Tailwind classes with light and dark variants. This keeps entry-type decisions testable without putting framework-specific class strings into business tests.

## Cell Content

The period cell remains one keyboard-focusable button that opens the existing detail dialog. It no longer draws its own dashed border; the entry blocks provide the visible surface. Hover and focus states apply to the whole cell without adding persistent decoration.

For a course block:

- subject code is the strongest line;
- subject name is a compact secondary line with a two-line clamp;
- classroom and room appear as separate compact metadata rows with the existing SchoolOrbit icon style;
- missing metadata rows are omitted rather than replaced with punctuation.

For a centered block:

- the title is centered vertically and horizontally;
- synchronized activities add a muted classroom-count line below the title;
- other activity, homeroom, academic, and break entries show only the title.

Multiple groups in one teacher-period cell remain vertically stacked with a small gap. The complete raw entries remain available to the dialog and filters.

## Empty and Detail Behavior

An empty cell displays no visible “ว่าง” label. Its button receives an accessible label containing the teacher, period, and empty state so keyboard and assistive-technology users retain context. Clicking or pressing Enter/Space continues to open the empty-period dialog.

The detail dialog keeps the current type, team-teaching, and synchronized badges and all classroom, room, subject-group, subject-code, and note fields. No status is removed from the workflow; the grid only reduces repeated presentation.

## Responsive, Theme, and Interaction Behavior

The existing responsive contract remains unchanged: 128px teacher column, 132px minimum period columns, full-width expansion on wide screens, sticky headers, and horizontal scrolling on narrow devices.

Pastel surfaces use restrained blue, emerald, and amber Tailwind tokens with matching borders and explicit dark-mode variants. Text contrast remains readable in both themes. The existing focus ring, click target, sticky behavior, and reduced table padding remain intact.

## Testing and Verification

Behavior tests in the existing daily-teaching display test file will cover the entry-type-to-presentation mapping for every supported entry type and the accessible empty-cell label. The tests will be written and observed failing before the helper and Svelte markup change.

The changed Svelte page will be checked with the Svelte autofixer. Verification will run:

- focused daily-teaching display tests;
- `npm run lint`;
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`;
- `npm run test:static`;
- `git diff --check`, final diff review, and `git status --short`.

## Impact and Scope Boundaries

This is a frontend presentation-only refinement. It does not change the responsive column calculation, grouping rules, filters, summaries, dialog data, API contracts, backend queries, permissions, database migrations, realtime behavior, or timetable creation and editing. It does not change the main timetable page that supplied the visual reference.
