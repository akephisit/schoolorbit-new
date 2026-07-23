# Calendar Service Modularization Design

**Date:** 2026-07-23
**Status:** Approved
**Scope:** Structural refactor of `backend-school` calendar services

## Context

`backend-school/src/modules/calendar/services.rs` contains about 2,280 lines. It owns category and
tag management, event lifecycle, audience visibility, recipient resolution, notifications,
reminder persistence, and the cross-tenant reminder worker. Calendar handlers, parent services,
and the application background task already call a stable `calendar::services::...` boundary.

## Goals

1. Split calendar behavior by cohesive use case while preserving the public service boundary.
2. Keep event writes, targets, tags, reminders, and notification behavior transactionally correct.
3. Make visibility and reminder rules independently testable.
4. Preserve the cross-tenant worker entry point used by `main.rs`.

## Non-goals

- No API, DTO, permission, migration, notification channel, or frontend change.
- No new background queue, cache, scheduler, or external infrastructure.
- No query or index optimization in this phase.
- No change to event visibility, reminder timing, or notification wording.

## Selected architecture

`modules/calendar/services.rs` remains the public compatibility facade.

```text
backend-school/src/modules/calendar/
├── services.rs
└── services/
    ├── shared.rs
    ├── categories_and_tags.rs
    ├── events.rs
    ├── visibility.rs
    ├── notifications.rs
    └── reminders.rs
```

No `mod.rs` file is introduced.

### `services.rs`

- Declares private child modules.
- Re-exports the current public functions and helper types.
- Contains no SQL, transaction, or worker workflow.

### `shared.rs`

- Owns event date/time validation, target validation, reminder-date calculation, and stable
  deduplication helpers shared by multiple workflows.
- Owns only row types shared by more than one child.

### `categories_and_tags.rs`

- Lists, creates, updates, and deletes calendar categories and tags.
- Preserves uniqueness, active-state, and delete-conflict behavior.

### `events.rs`

- Creates, updates, and soft-deletes management events.
- Persists event targets, tags, and reminder offsets within the current transaction boundaries.
- Uses shared validation before opening a write transaction.

### `visibility.rs`

- Lists management, own, child, and public events.
- Resolves audience and parent-child visibility without exposing PII.
- Keeps `parents::services` calling the facade path for child events.

### `notifications.rs`

- Resolves event recipient user IDs.
- Sends event notifications through the existing notification service.
- Keeps recipient deduplication and user visibility rules unchanged.

### `reminders.rs`

- Processes due reminders for one tenant.
- Runs the existing cross-tenant reminder loop invoked by `main.rs`.
- Marks reminder delivery only according to current successful-delivery behavior.
- Uses narrow notification functions without duplicating recipient resolution.

## Dependency and visibility rules

External callers use only `calendar::services`. Child modules remain private. `events` and
`reminders` may call narrow notification/recipient helpers. `shared.rs` depends on no use-case
module. The cross-tenant worker resolves tenant pools using the existing application facilities
and then delegates per-tenant work through the facade.

## Behavioral compatibility

- Public paths and signatures remain unchanged.
- SQL, transaction boundaries, target/tag/reminder write order, and soft-delete semantics remain
  unchanged.
- Event visibility for staff, students, parents, and public users remains unchanged.
- Reminder dates, delivery status, notification text, and worker interval behavior remain
  unchanged.
- Errors continue to propagate through `AppError`; no partial failure is silently discarded.

## Testing strategy

Before extraction, PostgreSQL characterization tests cover:

- category and tag lifecycle and uniqueness conflicts;
- event create/update/delete with targets, tags, and reminders;
- management, own, child, and public event visibility;
- recipient resolution and deduplication;
- reminder due-date selection and delivery-state transitions;
- notification failure behavior;
- cross-tenant processing isolation.

Pure validation and reminder-date tests move beside the owning modules. Static architecture tests
protect the facade, child privacy, parent/main caller paths, structured error propagation, and
absence of SQL in the facade.

## Implementation sequence

1. Record the public surface and add missing characterization tests.
2. Extract shared validation and date helpers.
3. Extract categories and tags.
4. Extract event lifecycle and visibility reads.
5. Extract recipients and notification delivery.
6. Extract per-tenant and cross-tenant reminder processing.
7. Reduce the facade to declarations and re-exports.
8. Tighten visibility and run full verification.

## Definition of done

- `calendar/services.rs` is a thin private-module facade.
- Handlers, parent services, and `main.rs` retain their existing public call paths.
- Event, visibility, notification, and reminder workflows have characterization coverage.
- No child file becomes a replacement monolith.
- API, permissions, migrations, frontend behavior, and notification semantics are unchanged.
- Formatting, strict Clippy, static architecture tests, and the full PostgreSQL-backed backend
  suite pass.
