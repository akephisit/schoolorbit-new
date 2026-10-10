# Daily Teaching Responsive Density Design

## Problem

The daily teaching overview uses a dynamic 136–188px teacher column and a fixed 168px width for every period. A ten-period day therefore needs roughly 1,816–1,868px before borders and scrollbars. Even on a wide desktop this can hide the last periods, while the subject-group line below every teacher name adds vertical noise without helping the primary scan task.

The page's main job is to let staff scan which teacher teaches what in each period. It should use the available desktop width before asking the user to scroll, while retaining readable cells on smaller devices.

## Outcome

On wide screens, the teacher column and all period columns fit within the schedule card without horizontal scrolling. On narrower screens, the table keeps a readable minimum width and exposes horizontal scrolling instead of compressing content beyond legibility.

Teacher rows show the teacher name only. Subject-group filtering remains available in the filter controls, and subject-group information remains available in period details where it describes an entry.

## Considered Approaches

### Responsive fluid columns — selected

The table fills the available width. Period columns share the remaining desktop width, while a calculated minimum table width preserves readable period cells on smaller viewports. This meets both the wide-screen and small-device goals with one layout.

### Fixed compact columns

Every period would use a narrower fixed width. This is predictable but still causes unnecessary scrolling when the viewport has spare room and does not adapt to varying period counts.

### User-selectable density

A compact/comfortable toggle would support personal preference but adds state, controls, and testing without solving a distinct workflow requirement. It is outside this change.

## Table Layout

The schedule table will use a fixed layout so its declared column widths are stable and content cannot expand a column unexpectedly.

- The sticky teacher column will use a compact fixed width of 128px.
- The table will fill the schedule viewport when the available width is at least the readable minimum.
- Each period will receive an equal share of the remaining width on wide screens.
- The minimum table width will be calculated from the 128px teacher column plus 132px per period.
- When the viewport is narrower than that minimum, the existing schedule scroll container will provide horizontal scrolling.
- The sticky teacher and period headers will keep their current behavior during horizontal and vertical scrolling.

This design supports different numbers of periods without hard-coding a desktop breakpoint: available width and the calculated minimum determine whether the table fits or scrolls.

## Information Density

The teacher cell will render only the teacher's display name. Long names will remain truncated visually and expose the full display name through a title tooltip. Subject-group names continue to participate in search and subject-group filtering; removing the subtitle is a presentation change only.

Header, table-cell, cell-button, and inner-card padding will be reduced by one spacing step. Period cells will use a slightly smaller minimum height. Existing type sizes, badge meanings, synchronized-activity grouping, empty-cell labels, and period detail behavior will remain unchanged. The result should feel like the existing SchoolOrbit interface at a denser operational-table setting, not a new visual theme.

## Responsive and Accessibility Behavior

The table will never make period content narrower than 132px. Small devices retain a visible horizontal scrollbar and normal touch/trackpad scrolling. Sticky positioning keeps teacher identity visible while moving across periods.

Interactive cells remain buttons with the current keyboard focus ring and dialog behavior. Truncation will not remove the accessible teacher name, because the full text remains in the DOM and is also exposed as a title. No information required to understand a period is moved to hover-only UI.

## Testing and Verification

A focused static regression test will inspect the Svelte source and prove the layout contract that would fail if the production change were reverted:

- the table uses the compact teacher and minimum period widths;
- the table fills available width while retaining the calculated minimum width;
- teacher rows no longer render subject-group names;
- the subject-group filter remains present;
- period buttons and cards use the compact spacing contract.

The test will be written and observed failing before the Svelte implementation changes. After implementation, run:

- the focused static test;
- the Svelte autofixer on the changed page;
- `npm run lint`;
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`;
- `npm run test:static`;
- `git diff --check`, final diff review, and `git status --short`.

## Impact and Scope Boundaries

This is a frontend presentation-only change. It does not change API contracts, backend queries, database migrations, permissions, realtime events, summary calculations, filters, printing data, or timetable creation. It does not add a density preference or redesign the filter and summary sections.
