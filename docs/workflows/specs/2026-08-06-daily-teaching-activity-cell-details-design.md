# Daily Teaching Activity Cell Details Design

## Problem

The simplified daily teaching grid gives course cards enough content to grow beyond their minimum height, while centered activity, homeroom, academic, and break cards remain at the minimum. The resulting cards do not align visually. Cell and group padding also compounds, leaving more space between cards than the dense timetable needs.

Activity data is present but not consistently surfaced. Independent activity entries already carry `classroomName` and `roomCode`, yet the grid renders only the title. Synchronized activity details collect classroom labels in backend entry order, which is primarily entry-ID order rather than classroom order. Manually created timetable entries preserve newlines in `title`, but the Today page collapses those newlines even though the main timetable supports them with `whitespace-pre-line`.

## Outcome

Every occupied entry card uses the same fixed 88px height. The table becomes denser by reducing cell inset and the vertical gap between multiple cards without changing the existing responsive column widths or minimum touch target.

Entry content remains type-specific:

- courses retain code, subject, classroom, and physical room;
- independent activities show their title, classroom, and physical room;
- synchronized activities keep a compact title and classroom count in the grid;
- homeroom, academic, break, and manually created entries keep their centered title treatment;
- embedded newlines in non-course titles are preserved and clamped so they cannot expand the card.

The synchronized-activity detail dialog displays classroom assignments as compact rows with separate classroom and physical-room columns. Rows are sorted naturally by classroom and then physical room, so numeric labels such as `ม.1/2` appear before `ม.1/10`.

## Considered Approaches

### Frontend presentation helper using the existing API — selected

The daily endpoint already returns entry type, scheduling mode, classroom, room, title, and synchronized slot identity. A pure helper can derive ordered locations and activity metadata without changing data ownership. This is the smallest coherent change and keeps sorting behavior directly testable.

### Backend-sorted location DTO

The backend could return a dedicated synchronized-location array. That would centralize ordering but would require Rust DTO, query, OpenAPI, generated TypeScript, and contract changes for presentation behavior that can be derived safely from existing typed data.

### CSS-only card alignment

Fixed height and tighter spacing alone would align the cards, but independent activity locations, synchronized ordering, and manual newlines would remain incorrect.

## Presentation and Data Flow

The existing `daily-teaching-display.ts` helper remains the UI presentation boundary. It will expose ordered structured locations for a display group rather than relying only on insertion-ordered combined strings. Each location contains the optional classroom name, optional physical-room code, and a fallback display label.

A Thai natural collator with numeric comparison sorts locations by classroom first and physical room second. Duplicate classroom-room pairs are removed. The existing synchronized grouping identity remains unchanged: only `ACTIVITY` entries with `synchronized` scheduling mode and the same activity slot are combined.

The Svelte page consumes the same `DailyTeachingEntry` contract:

1. group entries for each teacher-period cell;
2. derive the semantic card presentation and ordered locations;
3. render a fixed-height, overflow-safe card;
4. show location rows in independent activity cards;
5. preserve title newlines with `whitespace-pre-line` while clamping visible lines;
6. open the existing dialog with the original raw entries.

No mutation, reload, or additional API call is introduced.

## Grid Layout

All entry blocks use `h-[5.5rem]` (88px) and `overflow-hidden`. Course content stays left-aligned. Centered activity-style content remains vertically centered unless it has classroom metadata, in which case the title and metadata use the same compact internal rhythm as a course.

Independent activity cards show classroom and physical room as two compact icon rows when values exist. Missing values are omitted rather than replaced with dashes. Their title uses at most two visible lines so both location rows fit within the fixed height.

Entries without location metadata may use up to three visible title lines. `whitespace-pre-line` respects the line breaks entered in the timetable batch textarea. The fixed height and clamp prevent long manual titles from changing row alignment.

Table-cell horizontal and vertical padding, the cell button inset, and the gap between multiple display groups are reduced to a 2–4px rhythm. The cell button remains full-width and keyboard focusable.

## Synchronized Detail Dialog

The dialog retains activity type, team-teaching, and synchronized badges. For synchronized groups, the current unordered classroom badges are replaced by a compact bordered list:

- first column: classroom, such as `ม.1/3`;
- second column: physical room, such as `115`;
- rows sorted naturally by classroom, then physical room;
- a missing physical room is displayed as `-` in the detail view so the assignment is explicit.

If no structured location exists, the dialog keeps the current entry-count fallback.

## Accessibility and Responsive Behavior

The cell remains one button with the existing focus ring and click behavior. Icons are decorative because adjacent text provides the same meaning. Visually empty cells retain their existing accessible label.

The 128px teacher column, 132px minimum period width, sticky headers, wide-screen expansion, and narrow-screen horizontal scrolling remain unchanged. Light and dark semantic color treatments remain unchanged.

## Testing and Verification

Test-driven changes in the existing daily-teaching display test file will cover:

- natural classroom and physical-room ordering;
- duplicate location removal;
- independent activity location presentation;
- preservation of the existing synchronized grouping/count behavior.

The focused test must fail for the missing ordering/presentation behavior before implementation. The changed Svelte page will be analyzed with the Svelte autofixer. Verification will run:

- focused daily-teaching display tests;
- `npm run lint`;
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`;
- `npm run test:menu-sync`;
- `npm run test:static`;
- `git diff --check`, final diff review, and `git status --short`.

## Impact and Scope Boundaries

This refinement changes frontend presentation and pure display helpers only. It does not change backend queries, OpenAPI or generated API contracts, permissions, migrations, realtime behavior, filters, summaries, responsive column calculations, or the timetable editing workflow. The main timetable page remains the source pattern for multiline manual titles and independent activity metadata.
