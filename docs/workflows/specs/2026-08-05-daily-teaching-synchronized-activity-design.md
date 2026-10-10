# Daily Teaching Synchronized Activity Design

## Problem

The daily teaching overview renders every timetable entry in a teacher-period cell as a separate card. A synchronized activity creates one timetable entry per participating classroom, so a teacher assigned to that activity can receive many otherwise-identical cards in one cell. The tallest cell determines the height of the whole teacher row, making the schedule sparse and difficult to scan.

The data is not duplicated accidentally: each entry represents a real classroom assignment. The display needs to group those entries without discarding their classroom details.

## Outcome

For one teacher and period, entries from the same synchronized activity slot appear as one compact card. The card shows the activity title, synchronized status, and the number of participating classrooms. Opening the period detail shows the complete deduplicated classroom and room list.

Course entries, independent activities, other timetable entry types, filtering, summary counts, printing, and authorization keep their current behavior.

## API Contract

`DailyTeachingEntry` will expose two nullable fields derived from the existing timetable relationships:

- `activitySlotId`, which identifies entries generated from the same activity slot;
- `activitySchedulingMode`, which is `synchronized`, `independent`, or null.

The daily-teaching query will select the slot ID and join through the activity catalog for the scheduling mode. No schema or migration change is required.

Because the response shape changes, the endpoint, query, response envelope, and daily-teaching schemas will be registered in the school OpenAPI contract. The generated TypeScript DTO will own the wire shape. The frontend timetable API will consume that generated DTO and map it to the existing camel-case UI model where necessary; generated artifacts will not be edited by hand.

Both fields remain nullable so non-activity entries and legacy activity entries without a slot continue to render safely.

## Frontend Display Model

A pure frontend helper will convert the raw entries in one teacher-period cell into ordered display groups.

An entry is groupable only when all of these conditions hold:

1. its type is `ACTIVITY`;
2. its scheduling mode is `synchronized`;
3. it has a non-empty activity slot ID.

Groupable entries with the same activity slot ID are combined. Every other entry becomes its own display group, even when titles or classroom names match. This prevents independent or incomplete legacy data from being merged heuristically.

The helper will preserve first-occurrence ordering and retain the original entries in each group. It will also derive a deduplicated list of non-empty classroom/room labels for presentation.

## Table and Detail Presentation

The table will render one card per display group:

- a standard `กิจกรรม` badge;
- an additional `พร้อมกัน` badge for synchronized groups;
- the activity title;
- `N ห้อง` when classroom names are available, otherwise `N รายการ` for a multi-entry group.

The compact card will not list every classroom. This keeps a synchronized activity from increasing the row height in proportion to its classroom count.

The existing period dialog will use the same display groups. A synchronized group appears once and lists all deduplicated classroom/room labels. Ungrouped entries retain the current detail layout.

The raw entries remain the source for subject and classroom filters. Summary figures remain calculated from the ungrouped backend data, so this visual change does not silently redefine lesson metrics.

## Compatibility and Failure Behavior

If an activity has no slot ID, has no scheduling mode, or has an unrecognized mode, it follows the current one-card-per-entry behavior. Missing classroom names fall back to an entry count in grouped presentation. Empty cells and non-activity cells are unchanged.

No new requests, permissions, realtime events, database fields, or sensitive data are introduced.

## Testing and Verification

Backend unit tests will prove that activity slot identity and scheduling mode survive seed-to-response mapping and that existing grouping and summary behavior is unchanged.

Frontend helper tests will prove that:

- synchronized entries from the same slot merge into one display group;
- different slots do not merge;
- independent activities do not merge;
- missing metadata does not trigger heuristic grouping;
- classroom/room labels are deduplicated while original entries remain available.

The Svelte page will be checked with the Svelte autofixer. Focused tests will run before the applicable repository matrices:

- backend-school formatting, static architecture tests, focused service tests, and `cargo check`;
- API contract generation and contract tests;
- frontend lint, Svelte/TypeScript check, and static tests;
- `git diff --check`, final diff review, and `git status --short`.

## Scope Boundaries

This change does not alter timetable creation, activity assignment, lesson-count definitions, permissions, database migrations, or the general timetable planner. It only adds the identity needed for an accurate daily-overview presentation and groups synchronized activity entries in that page.
