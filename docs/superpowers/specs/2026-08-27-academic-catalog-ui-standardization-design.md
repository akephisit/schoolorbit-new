# Academic Catalog UI and Input Standardization

**Date:** 2026-08-27

**Status:** Design approved in chat; written review pending

**Scope:** `backend-school` academic catalog read contracts, `frontend-school` academic catalog pages, the global academic-context switcher, shared select/date-picker/calendar primitives, and static UI policy tests

## Context

The subject and learner-development activity catalogs already separate stable catalog identity from effective-dated versions, but their current staff pages do not make that model easy to understand. The pages show a narrow list of stable codes and load one selected version history beside it. Teachers cannot scan the currently applicable name, classification, grade levels, credit or hours, lifecycle state, or pending drafts across the catalog.

The create-version form exposes domain values as free text. In particular, it asks users to enter comma-separated grade-level UUIDs even though grade levels are real master records. Subject type, activity type, and activity scheduling mode are also presented as unstructured text despite having known choices in current workflows.

SchoolOrbit already uses shadcn-svelte Select across application pages and a shared shadcn-based DatePicker in most date fields. The remaining inconsistencies are thirteen native `input[type="date"]` fields and the native month/year selects inside the local Calendar caption implementation. The academic-context switcher also exposes a redundant `บริบทงาน` label and active-status badges in its compact topbar trigger.

This change makes the two catalogs readable at a glance, avoids N+1 catalog requests, and establishes enforceable select and calendar conventions across the frontend.

## Goals

- Let teachers scan the currently relevant subject or activity details without opening every catalog item.
- Preserve the distinction between a stable catalog code and versioned descriptive data.
- Show the published version effective today as the primary row data, with clear handling for future, expired, unpublished, and draft states.
- Replace grade-level UUID entry with selection of human-readable grade-level master records.
- Replace catalog classification free text with deliberate shadcn-svelte selection controls.
- Keep initial catalog loading bounded to one overview request per page and avoid per-row API calls.
- Load full version history only when a user asks for it, then cache it for the page session.
- Simplify the topbar academic-context trigger while retaining complete status information inside its dropdown.
- Use shadcn-svelte primitives for application dropdowns, date pickers, and Calendar month/year selectors.
- Prevent native select and date inputs from being reintroduced into application UI.
- Preserve keyboard access, screen-reader semantics, responsive behavior, dark mode, and existing permission rules.

## Non-Goals

- Do not redesign unrelated SchoolOrbit pages or replace the established application shell.
- Do not introduce a general-purpose data-grid framework.
- Do not change the stable-identity/versioned-record academic model.
- Do not change catalog write endpoints or published-version immutability rules.
- Do not tie the global subject or activity catalog to the academic year selected in the topbar.
- Do not replace native time or datetime-local inputs in this release; the calendar standardization applies to date selection.
- Do not introduce a database migration solely for this UI change.

## Approaches Considered

### Frontend-only aggregation

The frontend could load every catalog code and request every version list to build the table. This would be quick to implement but would create an N+1 request pattern, scale poorly, and repeat the class of request-amplification issue already removed elsewhere in the application. It was rejected.

### Typed overview APIs plus shared UI primitives — selected

Add one typed overview read contract for subjects and one for activities. Each response contains all summary data needed for its page, including display-version metadata, draft count, and grade-level labels. Full history remains an on-demand, cached request. Standardize catalog inputs and all remaining native date/calendar controls on local shadcn-svelte primitives.

This keeps payloads purposeful, preserves existing writes, and establishes a clear frontend policy without introducing a larger framework.

### General catalog data-grid framework

A configurable server-driven grid could support arbitrary columns and filters, but it would add abstraction and maintenance cost before another catalog use case requires it. It was rejected as unnecessary.

## Domain Semantics

### Stable identity and display version

A subject or activity row has two visual regions:

1. **Stable identity:** catalog code and archive state from `subjects` or `activities`.
2. **Versioned details:** name, classification, grade applicability, credit/hours, effective range, and version lifecycle from the selected display version.

The overview service selects a display version relative to the tenant's current local date using this deterministic order:

1. a published version whose effective range contains today;
2. otherwise the nearest future published version;
3. otherwise the most recently ended published version;
4. otherwise no display version.

The response includes a display-state enum of `current`, `upcoming`, `expired`, or `unpublished`. A separate `draftCount` indicates whether unpublished work exists. A draft never replaces published information in the primary row.

### Grade levels

`gradeLevelIds` are foreign keys to stable grade-level master records, not codes users should type. Catalog applicability is global and effective-dated through the catalog version, so the catalog editor lists all active grade-level masters rather than filtering them by the academic year in the topbar.

The API returns grade levels ordered by level type and year with the identifiers and human labels needed by the UI. The editor uses a searchable multi-select built from shadcn-svelte Popover, Command, and Checkbox primitives. Selected values display as labels such as `ม.1`, not UUIDs.

### Catalog classifications

Known subject types, activity types, and activity scheduling modes are presented as labeled choices from one shared frontend domain-options module using the canonical strings already accepted by the API. Existing stored values that are not in the known set remain visible in read views and may be represented as an explicit current-value option while editing; the UI never silently rewrites them.

Activity scheduling mode keeps the backend-supported values `synchronized` and `independent`. Human-facing Thai labels explain whether groups share one synchronized period or may be scheduled independently.

## API Design

Add two authenticated, permission-filtered endpoints:

- `GET /api/academic/catalog/subjects/overview`
- `GET /api/academic/catalog/activities/overview`

Each endpoint uses the existing academic-catalog read permission and organization-unit visibility filter. Static routes must be registered so they are not interpreted as catalog UUID routes.

The subject response is an envelope containing `items` and `gradeLevelOptions`. Each item contains:

- the stable `CatalogSubject` identity;
- optional display `SubjectVersion`;
- display state;
- draft count;
- resolved grade-level lookup items referenced by the display version.

The activity response uses the same envelope shape and contains the equivalent stable identity, optional display `ActivityVersion`, display state, draft count, and resolved grade levels. In both responses, `gradeLevelOptions` is the complete ordered set of active global grade-level masters available to the editor, including levels not used by the current display versions.

Overview data is selected in bounded batch queries. The service must not execute a version or grade-level query once per catalog row. Full histories continue to use the existing version-list endpoints and are requested only when the user opens management details.

The OpenAPI document is the source of truth. Backend models and handlers define the contract, generated frontend types are regenerated, and frontend API wrappers consume only generated schemas and operation query types.

## Frontend Information Architecture

### Shared catalog workspace behavior

Both catalog pages use the same interaction model while preserving subject- and activity-specific columns:

- a page header with a concise explanation of stable codes and versioned details;
- a toolbar containing search, classification filter, grade-level filter, lifecycle filter, and a permission-gated add action;
- a desktop table for rapid scanning;
- stacked cards on small screens, using the same information order rather than a horizontally scrolling table;
- a large responsive Sheet for full history and create-version management;
- an explicit empty state with the next available action;
- loading skeletons that match the final row/card structure.

Opening a row fetches its full history once and stores it in an in-memory cache keyed by catalog ID. Closing and reopening the same item does not repeat the request. Creating or publishing a version invalidates that item and refreshes the page overview once.

### Subject columns

- stable code;
- Thai name and optional English name;
- subject type;
- applicable grade levels;
- credit;
- effective-state badge and effective range;
- draft indicator;
- details/manage action.

### Activity columns

- stable code;
- activity name;
- activity type;
- scheduling mode;
- applicable grade levels;
- hours per week;
- effective-state badge and effective range;
- draft indicator;
- details/manage action.

### Topbar academic context

The desktop trigger keeps only the Calendar icon, selected academic-year name, optional selected-term name, and chevrons supplied by Select. It removes the visible `บริบทงาน` label and removes status badges from the closed triggers. Year and term status badges remain beside each option inside the dropdown. The mobile compact summary remains concise; its Sheet retains descriptive context and statuses.

The dirty-form confirmation behavior and URL academic-context semantics do not change.

## Visual Direction

The subject is Thai teachers maintaining an authoritative academic catalog. The page's single job is to help a teacher verify what a code means and whether that information is in effect without opening every record.

The visual direction is a calm working register rather than a promotional dashboard:

- **Paper** `#F8FAFC`: low-contrast page ground;
- **Surface** `#FFFFFF`: tables, cards, and sheets;
- **Ink** `#0F172A`: primary reading text;
- **Orbit blue** `#315FA8`: identity and selected-state emphasis;
- **Published green** `#15803D`: currently effective published data;
- **Draft amber** `#B45309`: pending draft attention.

Implementation uses existing semantic CSS tokens so dark mode remains correct; the hex values describe the intended light-mode relationship rather than introduce page-local hard-coded colors.

Kanit remains the Thai display and body face to preserve SchoolOrbit identity. Stable catalog codes use the existing utility monospace stack and tabular spacing as a restrained secondary typographic role.

The signature element is a narrow **code spine** beside each stable code. It is structural rather than decorative: the spine visually separates immutable identity from the versioned details to its right. The same device becomes the leading edge of mobile cards.

```text
[ค้นหารหัสหรือชื่อ] [ประเภท] [ระดับชั้น] [สถานะ]          [+ เพิ่มรายการ]

┃ คณ21101  คณิตศาสตร์พื้นฐาน 1   พื้นฐาน   ม.1    1.5   ใช้งานอยู่
┃ คณ22101  คณิตศาสตร์พื้นฐาน 3   พื้นฐาน   ม.2    1.5   มีร่างใหม่
┃ ว30240   วิทยาการคำนวณเพิ่มเติม เพิ่มเติม  ม.4-6  1.0   สิ้นสุดแล้ว
```

Motion is limited to existing hover, focus, popover, and Sheet transitions. No decorative page-load animation is added. Focus rings remain visible and reduced-motion preferences are respected by the underlying primitives.

## Select and Calendar Standard

### Application selects

Selection controls use the local shadcn-svelte primitives according to purpose:

- `Select` for one choice from a bounded list;
- `Popover + Command` for searchable or multi-value selection;
- `DropdownMenu` for actions rather than form values.

The existing application already follows this rule in its current select consumers. A static policy test prevents raw HTML `select` elements outside explicitly allowed primitive implementation files.

### Date selection

The existing shared DatePicker remains the single date-selection component. It is extended only as required to cover the remaining native date-field behavior, including disabled state, optional clearing, identifiers, and accessible labels. Thirteen remaining `input[type="date"]` usages migrate to it. Native time and datetime-local fields are unchanged in this release.

### Calendar month and year selection

The local Calendar caption replaces its native MonthSelect and YearSelect rendering with shadcn-svelte Select controls. The caption updates the Calendar's bindable placeholder directly, including the month-index adjustment required when multiple months are displayed. Month and year option labels remain locale-aware and year bounds preserve the Calendar inputs.

This is an intentional local extension of the generated shadcn Calendar. Tests protect month/year synchronization, Thai/Buddhist-era labels, keyboard interaction, focus behavior, and nested Select behavior inside the DatePicker Popover.

After migration, application source and local Calendar implementation contain no raw HTML `select` and no `input[type="date"]`.

## Loading, Error, and Empty States

- Initial overview failure replaces the page body with a retryable error state.
- A history request failure is scoped to the management Sheet and does not remove the overview table.
- Save and publish errors remain inline with the action that failed.
- Busy actions disable only controls that could submit the same mutation twice.
- Empty catalogs explain that no stable code exists and show the add action only when the user has manage permission.
- A catalog item with no published version displays `ยังไม่มีรุ่นเผยแพร่`; a draft badge remains separate.
- Unknown stored classification strings display safely as text rather than disappearing.

## Permissions and Security

Existing generated academic-catalog permissions remain authoritative. Read users can search, filter, and open history. Manage users see create, publish, and editable controls according to their school or organization-unit scope.

Overview services reuse `AcademicResourceListFilter`; they must not reveal rows or organization ownership outside the caller's scope. No national ID, student-sensitive data, or plaintext identity value is introduced or logged.

## Testing and Verification

Implementation follows test-driven development and the `.rules` change-type matrix. Commands run serially.

Backend coverage proves:

- display-version selection for current, nearest future, most recently expired, and unpublished catalogs;
- drafts never replace published display data;
- draft counts and resolved grade levels are correct;
- organization-unit filtering is preserved;
- overview queries are bounded and do not issue one version query per catalog row;
- handlers and OpenAPI schemas expose the typed contract.

Frontend coverage proves:

- table and mobile-card structure exposes the approved fields;
- filters use labeled values and grade-level IDs never appear as editable text;
- version histories load on demand and are cached;
- topbar closed triggers omit `บริบทงาน` and active-status badges while dropdown options retain statuses;
- native `select` and `input[type="date"]` policy guards pass;
- Calendar month/year Select changes synchronize the visible calendar;
- DatePicker migration preserves required, optional, disabled, and clearing behavior;
- empty, loading, partial-error, and permission-restricted states are explicit.

Every edited Svelte component is checked with the Svelte autofixer. Verification includes focused tests, API contract generation/checks, backend formatting/lint/tests required by `.rules`, frontend formatting/lint, `npm run check`, `npm run test:static`, relevant Playwright coverage when available, and final diff/status review.

## Rollout and Compatibility

This release adds read-only overview endpoints and does not require a database migration. Existing write endpoints remain authoritative. The frontend switches directly to the new overview contracts; no feature flag or parallel legacy catalog UI is retained.

Deployment order follows the repository's normal single-main auto-deploy workflow. Because the frontend depends on additive endpoints, backend readiness must be verified before catalog smoke tests. Smoke tests cover both catalog URLs, topbar context selection, one existing DatePicker flow, and month/year navigation.

## Success Criteria

- A teacher can identify a catalog code's current name, type, grade applicability, amount, and lifecycle state from the overview without opening it.
- The two overview pages make one initial overview request each and never fan out per row.
- Users select grade levels by readable names and never enter UUIDs.
- No application or local Calendar source contains raw HTML `select` or `input[type="date"]` after the migration.
- The topbar trigger is compact and statuses remain discoverable inside dropdowns.
- Catalog management remains permission-correct, responsive, keyboard accessible, and contract-generated.
